use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultDemand { Want, Discard }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeferAction { pub function: FuncId, pub captures: Vec<ValueId>, pub bindings: Vec<BindingId> }
impl OperandSource for DeferAction {
    fn append(&self, mode: OperandMode, out: &mut Vec<ReferencedOperand>) { self.captures.append(mode, out); }
}

operation_group! { LexicalOp {
    Bind { binding: BindingId, value: ValueId }
        => ("bind", [value: Borrow], Rule::Intent);
    Rebind { binding: BindingId, value: ValueId, target: ScopeId }
        => ("rebind", [value: Borrow], Rule::Intent);
    Stage { action: ScopeId, field: FieldRef, value: ValueId }
        => ("stage", [value: Borrow], Rule::Intent);
    Temp { value: ValueId, scope: ScopeId }
        => ("temp", [value: Borrow], Rule::Intent);
    DeferRegister { scope: ScopeId, action: DeferAction }
        => ("defer.register", [action: Borrow], Rule::Intent);
    Want { value: ValueId }
        => ("want", [value: Observe], Rule::Intent);
    Discard { value: ValueId }
        => ("discard", [value: Observe], Rule::Intent);
} }
