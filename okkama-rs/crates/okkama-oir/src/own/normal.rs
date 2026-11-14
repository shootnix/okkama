use super::*;
use std::collections::BTreeSet;

// Ordered, flat atoms avoid recursive comparisons of input selection trees.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Atom {
    parameter: u16,
    kind: u8,
    fields: Vec<(DeclId, u32)>,
}

pub(super) fn selections(
    roots: &[SummarySelection],
    budget: &mut Budget<'_>,
) -> Result<Vec<SummarySelection>, OwnError> {
    let mut atoms = BTreeSet::new();
    let mut pending: Vec<_> = roots.iter().collect();
    while let Some(selection) = pending.pop() {
        budget.work(1)?;
        let (parameter, kind, fields) = match selection {
            SummarySelection::Parameter(p) => (*p, 0, vec![]),
            SummarySelection::Field { parameter, field } => {
                (*parameter, 1, vec![(field.declaration, field.index)])
            }
            SummarySelection::Path { parameter, fields } => {
                if fields.len() > 4 {
                    (*parameter, 2, vec![])
                } else {
                    let fields = fields.iter().map(|field| (field.declaration, field.index)).collect();
                    (*parameter, 1, fields)
                }
            }
            SummarySelection::OwnerGraph(p) => (*p, 2, vec![]),
            SummarySelection::Descriptor(p) => (*p, 3, vec![]),
            SummarySelection::Union(items) => {
                budget.memory(items.len().saturating_mul(8))?;
                pending.extend(items);
                continue;
            }
        };
        let kind = if kind == 1 && fields.is_empty() { 0 } else { kind };
        budget.memory(64 + fields.len() * 16)?;
        atoms.insert(Atom { parameter, kind, fields });
    }
    let owners: BTreeSet<_> = atoms.iter().filter(|atom| atom.kind == 2).map(|atom| atom.parameter).collect();
    atoms.retain(|atom| atom.kind == 2 || !owners.contains(&atom.parameter));
    Ok(atoms.into_iter().map(|atom| match atom.kind {
        0 => SummarySelection::Parameter(atom.parameter),
        2 => SummarySelection::OwnerGraph(atom.parameter),
        3 => SummarySelection::Descriptor(atom.parameter),
        _ if atom.fields.len() == 1 => SummarySelection::Field {
            parameter: atom.parameter,
            field: FieldRef { declaration: atom.fields[0].0, index: atom.fields[0].1 },
        },
        _ => SummarySelection::Path {
            parameter: atom.parameter,
            fields: atom.fields.into_iter().map(|(declaration, index)| FieldRef { declaration, index }).collect(),
        },
    }).collect())
}

pub(super) fn parameter(selection: &SummarySelection) -> u16 {
    match selection {
        SummarySelection::Parameter(p) | SummarySelection::OwnerGraph(p) | SummarySelection::Descriptor(p)
        | SummarySelection::Field { parameter: p, .. } | SummarySelection::Path { parameter: p, .. } => *p,
        SummarySelection::Union(_) => unreachable!("normal form contains only atoms"),
    }
}

pub(super) fn failure_key(kind: FailureKind) -> u8 {
    match kind {
        FailureKind::Thrown => 0,
        FailureKind::Program => 1,
        FailureKind::Panic => 2,
        FailureKind::Native => 3,
        FailureKind::Cancelled => 4,
    }
}

pub(super) fn failures(target: &mut Vec<FailureKind>, source: &[FailureKind]) {
    target.extend_from_slice(source);
    target.sort_by_key(|kind| failure_key(*kind));
    target.dedup();
}

pub(super) fn relation(
    target: &mut ResultRelation,
    source: &ResultRelation,
    budget: &mut Budget<'_>,
) -> Result<(), OwnError> {
    let empty = |relation: &ResultRelation| {
        relation.fresh.is_empty() && relation.aliases.is_empty() && relation.external_deps.is_empty()
    };
    if !empty(source) {
        if empty(target) {
            target.provenance = source.provenance;
        } else if target.provenance != source.provenance {
            return Err(budget.fail(OwnDiagnosticKind::IncompatibleProvenanceJoin, format!(
                "result provenance {:?} conflicts with {:?}", target.provenance, source.provenance,
            )));
        }
    }
    target.fresh.extend_from_slice(&source.fresh);
    target.fresh.sort();
    target.fresh.dedup();
    target.aliases.extend_from_slice(&source.aliases);
    target.aliases = selections(&target.aliases, budget)?;
    target.external_deps.extend_from_slice(&source.external_deps);
    target.external_deps.sort();
    target.external_deps.dedup();
    Ok(())
}

pub(super) fn empty_relation() -> ResultRelation {
    ResultRelation {
        fresh: vec![],
        aliases: vec![],
        external_deps: vec![],
        provenance: AdmissionProvenance::UnboundResult,
    }
}

pub(super) fn normalize(summary: &mut Summary, budget: &mut Budget<'_>) -> Result<(), OwnError> {
    let count = summary.parameters.len();
    for effect in &mut summary.parameters {
        budget.work(1 + effect.deps.len())?;
        effect.deps.sort();
        effect.deps.dedup();
        let items = effect.retention.take().into_iter().collect::<Vec<_>>();
        let mut items = selections(&items, budget)?;
        effect.retention = match items.len() {
            0 => None,
            1 => items.pop(),
            _ => Some(SummarySelection::Union(items)),
        };
        if effect.deps.iter().any(|index| usize::from(*index) >= count) {
            return Err(budget.fail(OwnDiagnosticKind::IncompatibleSignature, "invalid dependency index"));
        }
    }
    for result in &mut summary.results {
        result.fresh.sort();
        result.fresh.dedup();
        result.external_deps.sort();
        result.external_deps.dedup();
        result.aliases = selections(&result.aliases, budget)?;
    }
    failures(&mut summary.contract.failures, &[]);
    Ok(())
}

/// Every component is monotone. Boundary representations are fixed, not tops.
/// Incompatible publication/protocol contracts are D15 diagnostics in 0.6.0.
pub(super) fn join(
    target: &mut Summary,
    source: &Summary,
    budget: &mut Budget<'_>,
) -> Result<(), OwnError> {
    budget.work(1)?;
    if target.entry_kind != source.entry_kind {
        return Err(budget.fail(OwnDiagnosticKind::IncompatibleEntryJoin, "different entry kinds"));
    }
    if target.contract.results != source.contract.results || target.parameters.len() != source.parameters.len()
        || target.results.len() != source.results.len() {
        return Err(budget.fail(OwnDiagnosticKind::IncompatibleSignature, "different summary signatures"));
    }
    let mut diagnostics = vec![];
    if target.contract.publication != source.contract.publication {
        budget.stats.incompatible_publication_joins += 1;
        diagnostics.push(OwnDiagnostic {
            kind: OwnDiagnosticKind::IncompatiblePublicationJoin,
            function: None,
            site: None,
            message: format!("publication {:?} conflicts with {:?}",
                target.contract.publication, source.contract.publication),
        });
    }
    if target.contract.protocol != source.contract.protocol {
        budget.stats.incompatible_protocol_joins += 1;
        diagnostics.push(OwnDiagnostic {
            kind: OwnDiagnosticKind::IncompatibleProtocolJoin,
            function: None,
            site: None,
            message: format!("protocol {:?} conflicts with {:?}", target.contract.protocol, source.contract.protocol),
        });
    }
    if !diagnostics.is_empty() {
        return Err(OwnError::Diagnostics { diagnostics, stats: Box::new(budget.stats.clone()) });
    }
    for (target, source) in target.parameters.iter_mut().zip(&source.parameters) {
        if source.convention == FreshConvention::Take {
            target.convention = FreshConvention::Take;
        }
        let mut selections: Vec<_> = target.retention.take().into_iter().collect();
        selections.extend(source.retention.clone());
        let mut selections = self::selections(&selections, budget)?;
        target.retention = match selections.len() {
            0 => None,
            1 => selections.pop(),
            _ => Some(SummarySelection::Union(selections)),
        };
        target.deps.extend_from_slice(&source.deps);
        target.deps.sort();
        target.deps.dedup();
        if target.destination == TransferDestination::None {
            target.destination = source.destination.clone();
        } else if source.destination != TransferDestination::None && target.destination != source.destination {
            return Err(budget.fail(OwnDiagnosticKind::IncompatibleDestinationJoin, format!(
                "destination {:?} conflicts with {:?}", target.destination, source.destination,
            )));
        }
    }
    for (target, source) in target.results.iter_mut().zip(&source.results) {
        relation(target, source, budget)?;
    }
    failures(&mut target.contract.failures, &source.contract.failures);
    target.contract.suspends |= source.contract.suspends;
    target.contract.async_destroy |= source.contract.async_destroy;
    Ok(())
}

pub(super) fn size(summary: &Summary) -> usize {
    let mut bytes = 256 + summary.parameters.len() * 128 + summary.results.len() * 128;
    for result in &summary.results {
        bytes += result.fresh.len() * 4 + result.aliases.len() * 128 + result.external_deps.len() * 2;
    }
    for parameter in &summary.parameters {
        bytes += parameter.deps.len() * 2;
        if let Some(SummarySelection::Union(items)) = &parameter.retention {
            bytes += items.len() * 128;
        }
    }
    bytes
}
