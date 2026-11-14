//! Commit-0 structural checks. Cleanup protocol proofs belong to #564 commit 4.
use super::*;
use std::collections::BTreeSet;

#[allow(clippy::too_many_arguments)]
pub(super) fn defer_edges(
    checker: &mut Checker<'_, '_>, callee: FuncId, call: &CallSite, continue_edge: &InvokeEdge,
    replacements: &[DeferReplaceEdge], unwind: Option<&InvokeEdge>, caller: FuncId,
    block: BlockId, next_edge: &mut u32,
) -> Checked<()> {
    let module = checker.module;
    let body = checker.body;
    let declaration = module.functions.get(callee)?;
    let labels = declaration.attrs.iter().find_map(|attr| {
        if let FunctionAttr::DeferEntry { replacements } = attr { Some(replacements) } else { None }
    });
    let Some(labels) = labels else {
        checker.report(R::T1, "invoke.defer target must declare the defer ABI");
        return Ok(());
    };
    checker.require(module.profile != Profile::High, "invoke.defer requires elaborated ownership");
    checker.require(call.summary.is_some(), "invoke.defer requires its call summary");
    checker.require(call.packet.is_some(), "invoke.defer requires an explicit offered input packet");
    checker.require(call.cleanup == CallCleanupMode::Ordinary, "invoke.defer shielding is intrinsic");
    checker.require(labels.len() == replacements.len(), "one replacement edge is required per declared label");
    let registry = ContractRegistry::new(module);
    let mut shielded = call.clone();
    shielded.cleanup = CallCleanupMode::Shielded;
    if registry.lookup(&Operation::Call(CallOp::Direct { function: callee, call: shielded })).is_err() {
        checker.report(R::T1, "defer operands do not match the declared call contract");
    }
    at_edge(caller, block, next_edge, checker.state);
    invoke_target(module, body, continue_edge, Outcome::DeferContinue, &[],
        &checker.substitution, checker.state);
    for (index, (label, replacement)) in labels.iter().zip(replacements).enumerate() {
        checker.require(label.label == replacement.label, "replacement edges must follow declared label order");
        let index = u16::try_from(index).map_err(|_| TypeCheckError)?;
        let outcome = Outcome::DeferReplace(index);
        let slots = registry.defer_results(callee, outcome).map_err(|_| TypeCheckError)?;
        at_edge(caller, block, next_edge, checker.state);
        invoke_target(module, body, &replacement.edge, outcome, &slots, &checker.substitution, checker.state);
        linear_slots(&replacement.edge, outcome, &slots, checker);
    }
    let may_fail = call.summary.and_then(|id| module.summaries.get(id).ok())
        .is_none_or(|summary| summary.contract.failures.iter().any(|kind| *kind != FailureKind::Cancelled));
    checker.require(may_fail == unwind.is_some(), "defer unwind edge must agree with application failures");
    if let Some(unwind) = unwind {
        let slots = registry.defer_results(callee, Outcome::Unwind).map_err(|_| TypeCheckError)?;
        at_edge(caller, block, next_edge, checker.state);
        invoke_target(module, body, unwind, Outcome::Unwind, &slots, &checker.substitution, checker.state);
    }
    Ok(())
}

fn linear_slots(edge: &InvokeEdge, outcome: Outcome, slots: &[ResultSlot], checker: &mut Checker<'_, '_>) {
    for (index, slot) in slots.iter().enumerate() {
        if slot.ty != ResultType::Service(ServiceType::Own) { continue; }
        let uses = edge.args.iter().filter(|arg| matches!(arg, EdgeArg::Produced(result)
            if result.outcome == outcome && usize::from(result.slot) == index)).count();
        if uses != 1 {
            checker.report(R::T3, "replacement packet must be forwarded exactly once");
        }
    }
}

pub(super) fn metadata<'a>(module: &'a Module, body: &'a Body, function: FuncId, state: &mut State<'a>) {
    if module.profile != Profile::High && !body.high_results.is_empty() {
        state.error(R::T1, "result.exit metadata belongs to High");
    }
    let mut dictionaries = BTreeSet::new();
    for binding in &body.descriptor_bindings {
        let is_parameter = body.values.get(binding.dictionary).is_ok_and(|value| {
            module.types.get(value.ty) == Ok(&Type::Descriptor)
                && matches!(value.def, Definition::BlockParam { block, .. } if block == body.entry)
        });
        if !is_parameter || !dictionaries.insert(binding.dictionary) {
            state.error(R::T1, "descriptor.binding requires a unique dictionary entry parameter");
        }
        if module.functions.get(binding.contract.cleanup).is_err()
            || module.summaries.get(binding.contract.summary).is_err() {
            state.error(R::T4, "descriptor binding references an unknown cleanup contract");
        }
    }
    let mut names = BTreeSet::new();
    for schedule in &body.service_schedules {
        if schedule.name.is_empty() || !names.insert(&schedule.name)
            || module.scopes.get(schedule.owner).map_or(true, |scope| scope.function != function)
            || module.protocols.get(schedule.protocol).is_err() {
            state.error(R::S1, "invalid service schedule declaration");
        }
    }
    let mut sites = BTreeSet::new();
    for record in &body.high_effects {
        let key = match record.site {
            HighOperationSite::Instruction(inst) => (0, inst.as_u32()),
            HighOperationSite::Terminator(block) => (1, block.as_u32()),
        };
        if module.profile != Profile::High || !sites.insert(key) {
            state.error(R::T1, "high effect metadata requires a unique High operation site");
        }
        failure_set(&record.effects.failures, true, state);
        let operation = match record.site {
            HighOperationSite::Instruction(inst) => body.insts.get(inst).ok().map(|inst| &inst.op),
            HighOperationSite::Terminator(block) => body.blocks.get(block).ok().and_then(|block| {
                if let TermKind::InvokeOwn { op, .. } = &block.term.kind { Some(op.as_ref()) } else { None }
            }),
        };
        if let Some(operation) = operation
            && let Ok(contract) = ContractRegistry::new(module).construction_schema(operation, body)
            && (contract.effects.failures.iter().any(|kind| !record.effects.failures.contains(kind))
                || contract.effects.suspends && !record.effects.may_suspend) {
            state.error(R::H5, "High effect envelope understates the operation's conservative contract");
        }
    }
}

pub(super) fn failure_set(failures: &[FailureKind], cancellation: bool, state: &mut State<'_>) {
    let mut seen = BTreeSet::new();
    for failure in failures {
        if !state.step(1) { return; }
        if !seen.insert(*failure as u8) || !cancellation && *failure == FailureKind::Cancelled {
            state.error(R::T1, "effect failure inventory has a duplicate or forbidden cancellation kind");
        }
    }
}

/// Type a summary selection in its parameter context, carrying nominal substitutions along the path.
pub(super) fn summary_paths(module: &Module, summary: &Summary, params: &[TypeId], state: &mut State<'_>) {
    let mut pending: Vec<_> = summary.parameters.iter().filter_map(|effect| effect.retention.as_ref())
        .chain(summary.results.iter().flat_map(|result| result.aliases.iter())).collect();
    while let Some(selection) = pending.pop() {
        if !state.step(1) { return; }
        match selection {
            SummarySelection::Union(items) => pending.extend(items),
            SummarySelection::Path { parameter, fields } => {
                if fields.len() > 4 {
                    state.error(R::T4, "summary path cannot exceed four fields");
                    continue;
                }
                let valid = params.get(usize::from(*parameter))
                    .is_some_and(|ty| path_type(module, *ty, fields, state));
                if !valid { state.error(R::T4, "summary path does not follow substituted object fields"); }
            }
            SummarySelection::Field { parameter, field } => {
                let valid = params.get(usize::from(*parameter))
                    .is_some_and(|ty| path_type(module, *ty, &[*field], state));
                if !valid { state.error(R::T4, "summary field does not belong to its parameter type"); }
            }
            _ => {}
        }
    }
}

fn path_type(module: &Module, mut ty: TypeId, fields: &[FieldRef], state: &mut State<'_>) -> bool {
    let mut substitution = Substitution::default();
    for field in fields {
        loop {
            if !state.step(1) { return false; }
            let Ok(Type::Parameter(parameter)) = module.types.get(ty) else { break;
                };
            let Some((_, replacement)) = substitution.entries.iter().rev().find(|(key, _)| key == parameter)
                else { return false;
                    };
            if ty == *replacement { return false; }
            ty = *replacement;
        }
        let Ok(Type::Object { decl, args }) = module.types.get(ty) else { return false;
            };
        if *decl != field.declaration { return false; }
        let Ok(declaration) = module.declarations.get(*decl) else { return false;
            };
        if args.len() != declaration.type_params.len() { return false; }
        for (index, argument) in args.iter().enumerate() {
            let mut argument = *argument;
            if let Ok(Type::Parameter(parameter)) = module.types.get(argument)
                && let Some((_, replacement)) = substitution.entries.iter().rev().find(|(key, _)| key == parameter) {
                argument = *replacement;
            }
            substitution.entries.push((TypeParameterRef {
                owner: GenericOwner::Declaration(*decl), index: index as u32,
            }, argument));
        }
        let Ok(next) = field_type(module, *field) else { return false;
            };
        ty = next;
    }
    true
}
