use super::{EntityLocation as L, RuleId as R, State};
use crate::*;

fn language(module: &Module, types: &[TypeId]) -> Vec<TypeId> {
    types.iter().copied().filter(|ty| module.types.get(*ty)
        .is_ok_and(|ty| !matches!(ty, Type::Service(_) | Type::Descriptor))).collect()
}
pub(super) fn check(module: &Module, selected: Option<FuncId>, state: &mut State<'_>) {
    if selected.is_none() {
        for (id, summary) in module.summaries.iter() {
            state.at(L::Summary(id), None);
            shape(summary, state);
        }
    }
    for (function, data) in module.functions.iter() {
        if selected.is_some_and(|id| id != function) { continue; }
        state.at(L::Function(function), data.loc);
        if let Some(id) = data.summary {
            if let Ok(summary) = module.summaries.get(id) {
                let observation = high_external_observation(module, data, summary, state);
                signature(module, &data.signature, summary, None, observation, state);
            }
        } else if module.profile != Profile::High {
            state.error(R::B51, "elaborated function requires its declared summary");
        }
        let Some(body) = &data.body else {
            if module.profile != Profile::High {
                for attr in &data.attrs {
                    let trusted = match attr {
                        FunctionAttr::Extern(name) => state.context.external_contracts.native_summary(name),
                        FunctionAttr::Intrinsic { name, .. } => {
                            state.context.external_contracts.intrinsic_summary(name)
                        }
                        _ => continue,
                    };
                    let declared = data.summary.and_then(|id| module.summaries.get(id).ok());
                    if trusted.is_none() || trusted != declared {
                        state.error(R::B51, "external ownership summary differs from the trusted backend contract");
                    }
                }
            }
            continue;
        };
        if !super::graph::eligible(function, body, state) { continue; }
        for (block_id, block) in body.blocks.iter() {
            for &instruction in &block.insts {
                let Ok(inst) = body.insts.get(instruction) else { continue; };
                state.at(L::Instruction { function, instruction }, inst.loc);
                operation(module, body, &inst.op, state);
            }
            if let TermKind::InvokeOwn { op, .. } = &block.term.kind {
                state.at(L::Terminator { function, block: block_id }, block.term.loc);
                operation(module, body, op, state);
            }
        }
    }
}
// H2 keeps external High signatures unelaborated too. Only a registered
// boundary may declare an observation before the rewrite chooses its ABI.
fn high_external_observation(module: &Module, function: &Function, summary: &Summary, state: &State<'_>) -> bool {
    module.profile == Profile::High && function.body.is_none() && function.attrs.iter().any(|attr| {
        let trusted = match attr {
            FunctionAttr::Extern(name) => state.context.external_contracts.native_summary(name),
            FunctionAttr::Intrinsic { name, .. } => state.context.external_contracts.intrinsic_summary(name),
            _ => None,
        };
        trusted == Some(summary)
    })
}

fn signature(
    module: &Module,
    signature: &Signature,
    summary: &Summary,
    type_args: Option<(GenericOwner, &[TypeId])>,
    high_observation: bool,
    state: &mut State<'_>,
) {
    if !state.array(signature.params.len(), 16) || !state.array(signature.results.len(), 16)
        || !state.step(signature.params.len().saturating_add(signature.results.len())) {
        return;
    }
    let params = language(module, &signature.params);
    let results = language(module, &signature.results);
    if summary.parameters.len() != params.len() || summary.results.len() != results.len()
        || summary.contract.results.iter().zip(&results).any(|(actual, expected)| {
            type_args.map_or(actual != expected, |(owner, args)| {
                !super::types::instantiated_matches(module, *expected, *actual, owner, args, state)
            })
        }) || summary.contract.results.len() != results.len() {
        state.error(R::B51, "summary arity or result types differ from the signature");
    }
    // High deliberately has an unelaborated ABI (H2), including registered
    // observation intrinsics. Its final summary already knows the entry kind;
    // the synchronous rewrite will materialize Observation in the signature.
    let unelaborated_high = high_observation && module.profile == Profile::High
        && signature.ownership_abi == OwnershipAbi::Unelaborated;
    if !unelaborated_high && matches!(signature.ownership_abi, OwnershipAbi::Observation)
        != (summary.entry_kind == EntryKind::ObservationAdmissionStaysCaller) {
        state.error(R::B51, "summary entry kind differs from the signature ownership ABI");
    }
    shape(summary, state);
}
fn shape(summary: &Summary, state: &mut State<'_>) {
    let params = summary.parameters.len();
    if !state.step(params.saturating_add(summary.results.len())) { return; }
    if summary.results.len() != summary.contract.results.len() {
        state.error(R::B51, "summary result relations and contract result slots differ in arity");
    }
    for effect in &summary.parameters {
        if !state.step(effect.deps.len()) { return; }
        if effect.deps.iter().any(|i| usize::from(*i) >= params) {
            state.error(R::B51, "summary dependency parameter is out of range");
        }
        if effect.convention == FreshConvention::Borrow
            && (effect.retention.is_some() || effect.destination != TransferDestination::None) {
            state.error(R::B51, "borrow convention cannot retain offered fresh inputs");
        }
        if let Some(selection) = &effect.retention { selection_indices(selection, params, state); }
    }
    for result in &summary.results {
        if !state.step(result.external_deps.len().saturating_add(result.fresh.len()))
            || !state.array(result.fresh.len(), 32) { return; }
        if result.external_deps.iter().any(|i| usize::from(*i) >= params) {
            state.error(R::B51, "summary result dependency parameter is out of range");
        }
        for selection in &result.aliases { selection_indices(selection, params, state); }
    }
}

fn selection_indices(root: &SummarySelection, params: usize, state: &mut State<'_>) {
    let mut stack = vec![root];
    while let Some(selection) = stack.pop() {
        if !state.step(1) { return; }
        if let SummarySelection::Path { fields, .. } = selection && fields.len() > 4 {
            state.error(R::T4, "summary path cannot exceed four fields");
        }
        let index = match selection {
            SummarySelection::Parameter(i) | SummarySelection::OwnerGraph(i) | SummarySelection::Descriptor(i)
            | SummarySelection::Path { parameter: i, .. }
            | SummarySelection::Field { parameter: i, .. } => *i,
            SummarySelection::Union(items) => {
                if !state.array(items.len(), 8) { return; }
                stack.extend(items);
                continue;
            }
        };
        if usize::from(index) >= params { state.error(R::B51, "summary selection parameter is out of range"); }
    }
}
pub(super) fn call(op: &Operation) -> Option<(&CallSite, Option<CallArgument>, Option<FuncId>)> {
    let Operation::Call(op) = op else { return None; };
    Some(match op {
        CallOp::Direct { function, call } | CallOp::Native { function, call }
        | CallOp::Intrinsic { function, call } | CallOp::Resume { function, call, .. } => {
            (call, None, Some(*function))
        }
        CallOp::Role { receiver, call, .. } => (call, Some(*receiver), None),
        CallOp::Closure { callee, call } => (call, Some(*callee), None),
        CallOp::Cap { cap, call, .. } => (call, Some(*cap), None),
    })
}
fn operation(module: &Module, body: &Body, op: &Operation, state: &mut State<'_>) {
    let Some((call, receiver, target)) = call(op) else { return; };
    let Some(id) = call.summary else {
        if module.profile != Profile::High { state.error(R::B51, "elaborated call requires a summary"); }
        return;
    };
    let Ok(summary) = module.summaries.get(id) else { return; };
    let args = receiver.iter().chain(&call.args);
    if args.clone().count() != summary.parameters.len() {
        state.error(R::B51, "call arguments and summary parameters differ");
    }
    for (arg, effect) in args.zip(&summary.parameters) {
        let convention = match effect.convention {
            FreshConvention::Borrow => OperandConvention::Borrow,
            FreshConvention::Take => OperandConvention::Consume,
        };
        if arg.convention != convention { state.error(R::B51, "call convention differs from its summary"); }
    }
    if summary.entry_kind == EntryKind::ObservationAdmissionStaysCaller && call.packet.is_some() {
        state.error(R::B51, "observation cannot accept an input packet");
    }
    dispatch(module, body, op, summary, state);
    let target = target.or_else(|| cap_target(module, op));
    if let Some(target) = target.and_then(|id| module.functions.get(id).ok()) {
        if summary.entry_kind == EntryKind::ObservationAdmissionStaysCaller
            && target.summary.and_then(|id| module.summaries.get(id).ok())
                .is_some_and(|s| s.entry_kind == EntryKind::CallBoundary) {
            state.error(R::B52, "a call-boundary method cannot be relabeled as an observation");
        }
        if target.summary != call.summary
            && (target.summary.is_some() || module.profile != Profile::High) {
            state.error(R::B51, "call summary differs from the callee declaration");
        }
        let observation = high_external_observation(module, target, summary, state);
        signature(module, &target.signature, summary, None, observation, state);
    }
}

pub(super) fn cap_target(module: &Module, op: &Operation) -> Option<FuncId> {
    let Operation::Call(CallOp::Cap { field, .. }) = op else { return None; };
    let DeclarationKind::Cap { fields, .. } = &module.declarations.get(field.declaration).ok()?.kind else {
        return None;
    };
    fields.get(field.index as usize).map(|field| field.function)
}

fn witnesses(module: &Module, role: DeclId, slot: u32, summary: &Summary, state: &mut State<'_>) {
    for (_, declaration) in module.declarations.iter() {
        if !state.step(1) { return; }
        let DeclarationKind::Object { witnesses, .. } = &declaration.kind else { continue; };
        for witness in witnesses {
            if !state.step(1) { return; }
            if !matches!(module.types.get(witness.role), Ok(Type::Role { decl, .. }) if *decl == role) { continue; }
            let actual = witness.slots.get(slot as usize).and_then(|id| module.functions.get(*id).ok())
                .and_then(|function| function.summary).and_then(|id| module.summaries.get(id).ok());
            let Some(actual) = actual else {
                state.error(R::B51, "role witness adapter has no declared summary");
                continue;
            };
            if actual.entry_kind != summary.entry_kind || actual.parameters != summary.parameters
                || actual.results != summary.results || actual.contract.publication != summary.contract.publication
                || actual.contract.protocol != summary.contract.protocol
                || (actual.contract.suspends && !summary.contract.suspends)
                || (actual.contract.async_destroy && !summary.contract.async_destroy)
                || actual.contract.failures.iter().any(|failure| !summary.contract.failures.contains(failure)) {
                state.error(R::B51, "role slot summary does not cover its witness adapter");
            }
        }
    }
}

fn dispatch(module: &Module, body: &Body, op: &Operation, summary: &Summary, state: &mut State<'_>) {
    let Operation::Call(call) = op else { return; };
    match call {
        CallOp::Role { receiver, role, slot, .. } => {
            witnesses(module, *role, *slot, summary, state);
            let Ok(decl) = module.declarations.get(*role) else { return; };
            let DeclarationKind::Role { methods, .. } = &decl.kind else { return; };
            let Some(method) = methods.get(*slot as usize) else { return; };
            let Ok(value) = body.values.get(receiver.value) else { return; };
            let Ok(Type::Role { args, .. }) = module.types.get(value.ty) else { return; };
            if !state.array(method.signature.params.len() + 1, 16) { return; }
            let mut sig = method.signature.clone();
            sig.params.insert(0, value.ty);
            signature(module, &sig, summary, Some((GenericOwner::Declaration(*role), args)), false, state);
        }
        CallOp::Cap { cap, field, .. } => {
            let Ok(value) = body.values.get(cap.value) else { return; };
            let Ok(Type::Cap { decl, args }) = module.types.get(value.ty) else { return; };
            let Ok(declaration) = module.declarations.get(field.declaration) else { return; };
            let DeclarationKind::Cap { fields, .. } = &declaration.kind else { return; };
            let Some(field) = fields.get(field.index as usize) else { return; };
            let Ok(target) = module.functions.get(field.function) else { return; };
            let observation = high_external_observation(module, target, summary, state);
            signature(module, &target.signature, summary, Some((GenericOwner::Declaration(*decl), args)),
                observation, state);
        }
        CallOp::Closure { callee, .. } => {
            let Ok(value) = body.values.get(callee.value) else { return; };
            let Ok(Type::Fn(function)) = module.types.get(value.ty) else { return; };
            if !state.array(function.params.len() + 1, 16) { return; }
            let mut params = function.params.clone();
            params.insert(0, value.ty);
            let signature = Signature {
                params, results: function.result.into_iter().collect(),
                ownership_abi: if summary.entry_kind == EntryKind::ObservationAdmissionStaysCaller {
                    OwnershipAbi::Observation
                } else {
                    OwnershipAbi::Unelaborated
                },
            };
            self::signature(module, &signature, summary, None, false, state);
        }
        _ => {}
    }
}
