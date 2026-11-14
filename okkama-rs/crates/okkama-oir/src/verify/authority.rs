use super::{EntityLocation as L, RuleId as R, State};
use crate::*;

pub(super) fn owner(module: &Module, function: FuncId) -> Option<DeclId> {
    let data = module.functions.get(function).ok()?;
    data.attrs.iter().find_map(|attr| match attr {
        FunctionAttr::Method(id) | FunctionAttr::Ctor(id) | FunctionAttr::Successor(id)
        | FunctionAttr::Destroy(id) | FunctionAttr::Default { object: id, .. } => Some(*id),
        FunctionAttr::Lambda { owner, .. } => *owner,
        _ => None,
    }).or_else(|| module.declarations.iter().find_map(|(_, declaration)| {
        if let DeclarationKind::Cap { owner, fields } = &declaration.kind
            && fields.iter().any(|field| field.function == function)
            && let Ok(Type::Object { decl, .. }) = module.types.get(*owner) {
            Some(*decl)
        } else {
            None
        }
    }))
}
pub(super) fn field(module: &Module, field: FieldRef) -> Option<&Field> {
    let DeclarationKind::Object { fields, .. } = &module.declarations.get(field.declaration).ok()?.kind else {
        return None;
    };
    fields.get(field.index as usize)
}
pub(super) fn pending(module: &Module, reference: FieldRef) -> bool {
    field(module, reference).is_some_and(|f| f.name == "$pending"
        && module.types.get(f.ty) == Ok(&Type::Bool))
}
pub(super) fn check(module: &Module, selected: Option<FuncId>, state: &mut State<'_>) {
    for (function, data) in module.functions.iter() {
        if selected.is_some_and(|id| id != function) { continue; }
        let Some(body) = &data.body else { continue; };
        if !super::graph::eligible(function, body, state) { continue; }
        for (block_id, block) in body.blocks.iter() {
            for &instruction in &block.insts {
                let Ok(inst) = body.insts.get(instruction) else { continue; };
                state.at(L::Instruction { function, instruction }, inst.loc);
                operation(module, function, body, &inst.op, state);
            }
            if let TermKind::InvokeOwn { op, .. } = &block.term.kind {
                state.at(L::Terminator { function, block: block_id }, block.term.loc);
                operation(module, function, body, op, state);
            }
        }
        super::construction::check(module, function, body, state);
    }
}
fn callable(module: &Module, caller: FuncId, callee: FuncId, state: &mut State<'_>) {
    let Ok(data) = module.functions.get(callee) else { return; };
    if data.attrs.contains(&FunctionAttr::Entry) {
        state.error(R::A1, "entry cannot be called or bound as an ordinary action");
    }
    if data.attrs.contains(&FunctionAttr::Hidden)
        && (owner(module, caller).is_none() || owner(module, caller) != owner(module, callee)) {
        state.error(R::A1, "private action requires the same explicit owner authority");
    }
}
fn operation(module: &Module, function: FuncId, body: &Body, op: &Operation, state: &mut State<'_>) {
    let Ok(data) = module.functions.get(function) else { return; };
    let authority = owner(module, function);
    match op {
        Operation::Core(CoreOp::ObjNew { declaration, .. }) => {
            if !data.attrs.contains(&FunctionAttr::Ctor(*declaration)) {
                state.error(R::A1, "obj.new requires its object's constructor authority");
            }
        }
        Operation::Core(CoreOp::ObjInit { field, .. }) => {
            if !data.attrs.contains(&FunctionAttr::Ctor(field.declaration)) {
                state.error(R::A2, "obj.init requires its object's constructor authority");
            }
        }
        Operation::Core(CoreOp::ObjGet { field, .. } | CoreOp::OnceLoad { field, .. }
            | CoreOp::OnceStore { field, .. }) => {
            if authority != Some(field.declaration) {
                state.error(R::A1, "field access requires its owner's explicit authority");
            }
        }
        Operation::Core(CoreOp::ObjPending { field, .. }) => {
            if !pending(module, *field) { state.error(R::A2, "obj.pending may read only $pending: bool"); }
        }
        Operation::Core(CoreOp::ObjNext { object, updates, .. }) => {
            let receiver = body.values.get(*object).ok().and_then(|v| module.types.get(v.ty).ok());
            let same_owner = matches!(receiver, Some(Type::Object { decl, .. }) if Some(*decl) == authority);
            if !same_owner || !data.attrs.iter().any(|attr| matches!(attr, FunctionAttr::Successor(id)
                if Some(*id) == authority && updates.iter().all(|u| u.field.declaration == *id))) {
                state.error(R::A1, "obj.next requires the successor authority of its owner");
            }
        }
        Operation::Lexical(LexicalOp::Stage { field, .. }) => {
            if authority != Some(field.declaration)
                || !data.attrs.iter().any(|a| matches!(a, FunctionAttr::Ctor(_) | FunctionAttr::Successor(_))) {
                state.error(R::A1, "staging requires the constructor/successor of the field owner");
            }
        }
        Operation::Core(CoreOp::FnBind { function: callee, .. } | CoreOp::FnClosure { function: callee, .. }) => {
            callable(module, function, *callee, state);
        }
        Operation::Call(CallOp::Direct { function: callee, .. } | CallOp::Native { function: callee, .. }
            | CallOp::Intrinsic { function: callee, .. } | CallOp::Resume { function: callee, .. }) => {
            callable(module, function, *callee, state);
        }
        Operation::Lexical(LexicalOp::DeferRegister { action, .. }) => {
            callable(module, function, action.function, state);
        }
        _ => {}
    }
    let suspends = ContractRegistry::new(module).lookup(op).is_ok_and(|c| c.effects.suspends);
    let static_close = matches!(op, Operation::Ownership(OwnershipOp::ScopeEnd { .. }
        | OwnershipOp::ScopeUnwind { .. } | OwnershipOp::End { .. }));
    let lowered_cleanup = matches!(op, Operation::Cleanup(_));
    if suspends && !static_close && !lowered_cleanup && !data.attrs.contains(&FunctionAttr::Async) {
        state.error(R::A3, "suspending operation requires an async action");
    }
    if matches!(op, Operation::Core(CoreOp::TestOk { .. } | CoreOp::TestIs { .. }
        | CoreOp::TestIsnt { .. } | CoreOp::TestFail { .. } | CoreOp::FakeNew { .. }))
        && (!module.requires.contains(&Feature::Test) || state.options.execution != super::ExecutionMode::Tests) {
        state.error(R::A3, "test operations require requires test and test execution mode");
    }
}
