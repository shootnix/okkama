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
            if let Ok(summary) = module.summaries.get(id) { signature(module, &data.signature, summary, None, state); }
        } else if module.profile != Profile::High {
            state.error(R::B51, "elaborated function requires its declared summary");
        }
        let Some(body) = &data.body else { continue; };
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
fn signature(
    module: &Module,
    signature: &Signature,
    summary: &Summary,
    type_args: Option<(GenericOwner, &[TypeId])>,
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
    if matches!(signature.ownership_abi, OwnershipAbi::Observation)
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
        if !state.step(result.external_deps.len()) { return; }
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
        let index = match selection {
            SummarySelection::Parameter(i) | SummarySelection::OwnerGraph(i) | SummarySelection::Descriptor(i)
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
    let Ok(summary) = module.summaries.get(call.summary) else { return; };
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
    if let Some(target) = target.and_then(|id| module.functions.get(id).ok()) {
        if target.summary != Some(call.summary)
            && (target.summary.is_some() || module.profile != Profile::High) {
            state.error(R::B51, "call summary differs from the callee declaration");
        }
        signature(module, &target.signature, summary, None, state);
    }
}

fn dispatch(module: &Module, body: &Body, op: &Operation, summary: &Summary, state: &mut State<'_>) {
    let Operation::Call(call) = op else { return; };
    match call {
        CallOp::Role { receiver, role, slot, .. } => {
            let Ok(decl) = module.declarations.get(*role) else { return; };
            let DeclarationKind::Role { methods, .. } = &decl.kind else { return; };
            let Some(method) = methods.get(*slot as usize) else { return; };
            let Ok(value) = body.values.get(receiver.value) else { return; };
            let Ok(Type::Role { args, .. }) = module.types.get(value.ty) else { return; };
            if !state.array(method.signature.params.len() + 1, 16) { return; }
            let mut sig = method.signature.clone();
            sig.params.insert(0, value.ty);
            signature(module, &sig, summary, Some((GenericOwner::Declaration(*role), args)), state);
        }
        CallOp::Cap { cap, field, .. } => {
            let Ok(value) = body.values.get(cap.value) else { return; };
            let Ok(Type::Cap { decl, args }) = module.types.get(value.ty) else { return; };
            let Ok(declaration) = module.declarations.get(field.declaration) else { return; };
            let DeclarationKind::Cap { fields, .. } = &declaration.kind else { return; };
            let Some(field) = fields.get(field.index as usize) else { return; };
            let Ok(target) = module.functions.get(field.function) else { return; };
            signature(module, &target.signature, summary, Some((GenericOwner::Declaration(*decl), args)), state);
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
            self::signature(module, &signature, summary, None, state);
        }
        _ => {}
    }
}
