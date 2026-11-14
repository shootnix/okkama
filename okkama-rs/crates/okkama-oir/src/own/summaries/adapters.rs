use super::*;

pub(super) fn adapt(source: &Summary, captures: Option<usize>, budget: &mut Budget<'_>) -> Result<Summary, OwnError> {
    let Some(captures) = captures else {
        return Ok(source.clone());
    };
    let map = |parameter: u16| -> Result<u16, OwnError> {
        if usize::from(parameter) < captures {
            Ok(0)
        } else {
            u16::try_from(usize::from(parameter) - captures + 1).map_err(|_| {
                budget.fail(OwnDiagnosticKind::IncompatibleSignature, "closure parameter limit")
            })
        }
    };
    let map_selection = |selection: &SummarySelection| -> Result<SummarySelection, OwnError> {
        let parameter = normal::parameter(selection);
        if usize::from(parameter) < captures {
            return Ok(SummarySelection::OwnerGraph(0));
        }
        let parameter = map(parameter)?;
        Ok(match selection {
            SummarySelection::Parameter(_) => SummarySelection::Parameter(parameter),
            SummarySelection::OwnerGraph(_) => SummarySelection::OwnerGraph(parameter),
            SummarySelection::Descriptor(_) => SummarySelection::Descriptor(parameter),
            SummarySelection::Field { field, .. } => SummarySelection::Field { parameter, field: *field },
            SummarySelection::Path { fields, .. } => SummarySelection::Path { parameter, fields: fields.clone() },
            SummarySelection::Union(_) => unreachable!(),
        })
    };
    let mut result = source.clone();
    if captures > source.parameters.len() {
        return Err(budget.fail(OwnDiagnosticKind::IncompatibleSignature, "closure capture arity"));
    }
    let mut environment = ParameterEffect {
        convention: FreshConvention::Borrow, retention: None, deps: vec![], destination: TransferDestination::None,
    };
    for effect in &source.parameters[..captures] {
        if effect.convention == FreshConvention::Take {
            environment.convention = FreshConvention::Take;
            environment.retention = Some(SummarySelection::OwnerGraph(0));
            if environment.destination != TransferDestination::None && environment.destination != effect.destination {
                return Err(budget.fail(OwnDiagnosticKind::IncompatibleDestinationJoin, "closure capture destinations"));
            }
            environment.destination = effect.destination.clone();
        }
        for dependency in &effect.deps {
            environment.deps.push(map(*dependency)?);
        }
    }
    result.parameters = vec![environment];
    for effect in &source.parameters[captures..] {
        let mut effect = effect.clone();
        effect.deps = effect.deps.iter().map(|dep| map(*dep)).collect::<Result<_, _>>()?;
        if let Some(selection) = &effect.retention {
            let mut pending = vec![selection];
            let mut selections = vec![];
            while let Some(selection) = pending.pop() {
                match selection {
                    SummarySelection::Union(items) => pending.extend(items),
                    _ => selections.push(map_selection(selection)?),
                }
            }
            effect.retention = Some(SummarySelection::Union(selections));
        }
        result.parameters.push(effect);
    }
    for relation in &mut result.results {
        relation.aliases = relation.aliases.iter().map(map_selection).collect::<Result<_, _>>()?;
        relation.external_deps = relation.external_deps.iter().map(|dep| map(*dep)).collect::<Result<_, _>>()?;
    }
    normal::normalize(&mut result, budget)?;
    Ok(result)
}
