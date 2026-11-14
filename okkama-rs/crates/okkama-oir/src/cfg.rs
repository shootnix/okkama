//! Structural enumeration. Dominance, linearity and protocol validation are #564.
use crate::*;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseSite { Instruction(InstId), Terminator(BlockId), BlockUnwind(BlockId), ValueFacts(ValueId), InstructionFacts(InstId), Declaration(OperandRef) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind { Dispatch(u16), DeferContinue, DeferReplace(u16), DeferUnwind, Branch, Yes, No, SwitchCase(u32),
    Default,
    BlockUnwind, InvokeNormal, InvokeUnwind, CatchHandler, CatchNext, LookupFound, LookupMissing }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CfgEvent {
    Use { site: UseSite, operand: OperandUse },
    Definition { value: ValueId, definition: Definition },
    Edge { source: BlockId, target: BlockId, kind: EdgeKind },
    EdgeDefinition { source: BlockId, kind: EdgeKind, result: EdgeResult, schema: ResultSlot },
    EdgeResultUse { source: BlockId, kind: EdgeKind, result: EdgeResult },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraversalError { Arena(ArenaError), Contract(ContractError), CyclicExpression(OperandRef), TooManySlots }
impl From<ArenaError> for TraversalError { fn from(error: ArenaError) -> Self { Self::Arena(error) } }
impl From<ContractError> for TraversalError { fn from(error: ContractError) -> Self { Self::Contract(error) } }
impl std::fmt::Display for TraversalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for TraversalError {}

/// Resolve SSA leaves of typed expressions, preserving operand occurrences.
/// Path-local cycle detection also terminates on deliberately malformed IR.
pub fn operation_uses(op: &Operation, body: &Body) -> Result<Vec<OperandUse>, TraversalError> {
    let mut uses = vec![];
    for operand in op.operand_refs() { expand(operand, body, &mut uses)?; }
    Ok(uses)
}

/// Immediate references, without recursively expanding a shared expression DAG.
pub(crate) fn expression_children(reference: OperandRef, body: &Body) -> Result<Vec<OperandRef>, TraversalError> {
    let mut refs = vec![];
    let mut value = |value| refs.push(OperandRef::Value(value));
    match reference {
        OperandRef::Value(v) => value(v),
        OperandRef::Selection(id) => match body.selections.get(id)? {
            SelectionExpr::Value(v) | SelectionExpr::OwnerGraph(v) | SelectionExpr::ViewOwner(v) => value(*v),
            SelectionExpr::Projection(projection) => match projection {
                Projection::Field { value: v, .. } => value(*v),
                Projection::Index { value: v, index } => {
                    value(*v);
                    value(*index);
                }
                Projection::Slice { value: v, start, end } => {
                    value(*v);
                    value(*start);
                    value(*end);
                }
                Projection::Map { value: v, keys } => {
                    value(*v);
                    value(*keys);
                }
            },
            SelectionExpr::Admissible(id) => refs.push(OperandRef::Selection(*id)),
            SelectionExpr::Union(items) => refs.extend(items.iter().copied().map(OperandRef::Selection)),
            SelectionExpr::Descriptor { descriptor, value: v } => {
                value(*descriptor);
                value(*v);
            }
        },
        OperandRef::Dependency(id) => match body.deps.get(id)? {
            DepExpr::Empty => {}
            DepExpr::Key(id) => refs.push(OperandRef::Key(*id)),
            DepExpr::Union(items) => refs.extend(items.iter().copied().map(OperandRef::Dependency)),
            DepExpr::FromSsa(v) => value(*v),
            DepExpr::Retention { value: v, selection } => {
                value(*v);
                refs.push(OperandRef::Selection(*selection));
            }
        },
        OperandRef::Key(id) => match body.keys.get(id)? {
            KeyExpr::Created { instance, .. } => value(*instance),
            KeyExpr::ServiceOf { value: v, .. } | KeyExpr::Runtime(v) => value(*v),
        },
    }
    Ok(refs)
}

fn expand(
    operand: ReferencedOperand,
    body: &Body,
    uses: &mut Vec<OperandUse>,
) -> Result<(), TraversalError> {
    // Leave markers give path-local cycle detection. A shared node is revisited
    // on each occurrence, preserving the previous walk's order and multiplicity.
    let mut pending = vec![(operand.reference, false)];
    let mut active = HashSet::new();
    while let Some((reference, leaving)) = pending.pop() {
        if leaving {
            active.remove(&reference);
            continue;
        }
        if let OperandRef::Value(value) = reference {
            uses.push(OperandUse { value, mode: operand.mode });
            continue;
        }
        if !active.insert(reference) { return Err(TraversalError::CyclicExpression(reference)); }
        pending.push((reference, true));
        let children = expression_children(reference, body)?;
        pending.extend(children.into_iter().rev().map(|child| (child, false)));
    }
    Ok(())
}
fn emit_refs(
    refs: impl IntoIterator<Item = ReferencedOperand>,
    site: UseSite,
    body: &Body,
    visit: &mut impl FnMut(CfgEvent),
) -> Result<(), TraversalError> {
    for operand in refs {
        let mut uses = vec![];
        expand(operand, body, &mut uses)?;
        for operand in uses { visit(CfgEvent::Use { site, operand }); }
    }
    Ok(())
}
fn ssa(value: ValueId, mode: OperandMode) -> ReferencedOperand { ReferencedOperand { reference: OperandRef::Value(value), mode } }
fn edge(source: BlockId, target: &Target, kind: EdgeKind, site: UseSite, visit: &mut impl FnMut(CfgEvent)) {
    visit(CfgEvent::Edge { source, target: target.block, kind });
    for &value in &target.args { visit(CfgEvent::Use { site, operand: OperandUse { value, mode: OperandMode::Forward } }); }
}
fn invoke_edge(source: BlockId, target: &InvokeEdge, kind: EdgeKind, site: UseSite, visit: &mut impl FnMut(CfgEvent)) {
    visit(CfgEvent::Edge { source, target: target.block, kind });
    for arg in &target.args {
        match arg {
            EdgeArg::Value(value) => visit(CfgEvent::Use { site, operand: OperandUse { value: *value, mode: OperandMode::Forward } }),
            EdgeArg::Produced(result) => visit(CfgEvent::EdgeResultUse { source, kind, result: *result }),
        }
    }
}
fn edge_definitions(source: BlockId, kind: EdgeKind, outcome: Outcome, slots: &[ResultSlot], visit: &mut impl FnMut(CfgEvent)) -> Result<(), TraversalError> {
    for (index, schema) in slots.iter().enumerate() {
        let slot = u16::try_from(index).map_err(|_| TraversalError::TooManySlots)?;
        visit(CfgEvent::EdgeDefinition { source, kind, result: EdgeResult { outcome, slot }, schema: schema.clone() });
    }
    Ok(())
}
pub fn walk_terminator(block: BlockId, term: &Terminator, body: &Body, registry: ContractRegistry<'_>, visit: &mut impl FnMut(CfgEvent)) -> Result<(), TraversalError> {
    let site = UseSite::Terminator(block);
    let mut operands = vec![];
    match &term.kind {
        TermKind::Br(target) => edge(block, target, EdgeKind::Branch, site, visit),
        TermKind::Cond { condition, yes, no } => {
            operands.push(ssa(*condition, OperandMode::Observe));
            edge(block, yes, EdgeKind::Yes, site, visit); edge(block, no, EdgeKind::No, site, visit);
        }
        TermKind::Switch { value, cases, default } => {
            operands.push(ssa(*value, OperandMode::Observe));
            for (index, case) in cases.iter().enumerate() { edge(block, &case.target, EdgeKind::SwitchCase(u32::try_from(index).map_err(|_| TraversalError::TooManySlots)?), site, visit); }
            edge(block, default, EdgeKind::Default, site, visit);
        }
        TermKind::InvokeOwn { op, normal, unwind } => {
            operands.extend(op.operand_refs());
            let contract = registry.construction_schema(op, body)?;
            edge_definitions(block, EdgeKind::InvokeNormal, Outcome::Normal, &contract.normal.results, visit)?;
            if let Some(outcome) = &contract.unwind { edge_definitions(block, EdgeKind::InvokeUnwind, Outcome::Unwind, &outcome.results, visit)?; }
            invoke_edge(block, normal, EdgeKind::InvokeNormal, site, visit); invoke_edge(block, unwind, EdgeKind::InvokeUnwind, site, visit);
        }
        TermKind::InvokeDefer { function, call, continue_edge, replacements, unwind } => {
            call.append(OperandMode::Borrow, &mut operands);
            edge_definitions(block, EdgeKind::DeferContinue, Outcome::DeferContinue,
                &registry.defer_results(*function, Outcome::DeferContinue)?, visit)?;
            invoke_edge(block, continue_edge, EdgeKind::DeferContinue, site, visit);
            for (index, replacement) in replacements.iter().enumerate() {
                let index = u16::try_from(index).map_err(|_| TraversalError::TooManySlots)?;
                let outcome = Outcome::DeferReplace(index);
                edge_definitions(block, EdgeKind::DeferReplace(index), outcome,
                    &registry.defer_results(*function, outcome)?, visit)?;
                invoke_edge(block, &replacement.edge, EdgeKind::DeferReplace(index), site, visit);
            }
            if let Some(unwind) = unwind {
                edge_definitions(block, EdgeKind::DeferUnwind, Outcome::Unwind,
                    &registry.defer_results(*function, Outcome::Unwind)?, visit)?;
                invoke_edge(block, unwind, EdgeKind::DeferUnwind, site, visit);
            }
        }
        TermKind::StateDispatch { closed, failure, exit, arms } => {
            if let Some(closed) = closed { operands.push(ssa(*closed, OperandMode::Observe)); }
            operands.push(ssa(*failure, OperandMode::Observe));
            if let Some(exit) = exit { operands.push(ssa(*exit, OperandMode::Consume)); }
            for arm in arms {
                let kind = EdgeKind::Dispatch(arm.kind as u16);
                edge_definitions(block, kind, arm.kind.outcome(), &arm.kind.results(), visit)?;
                invoke_edge(block, &arm.edge, kind, site, visit);
            }
        }
        TermKind::RetDefer { values, .. } |
        TermKind::Ret { values } => operands.extend(values.iter().map(|&value| ssa(value, OperandMode::Forward))),
        TermKind::Throw(value) | TermKind::Raise(value) | TermKind::Panic(value) => operands.push(ssa(*value, OperandMode::Observe)),
        TermKind::Catch { failure, ty, handler, next } => {
            operands.push(ssa(*failure, OperandMode::Observe));
            edge_definitions(block, EdgeKind::CatchHandler, Outcome::Normal, &[ResultSlot::new(SlotName::Value, ResultType::Exact(*ty))], visit)?;
            invoke_edge(block, handler, EdgeKind::CatchHandler, site, visit); edge(block, next, EdgeKind::CatchNext, site, visit);
        }
        TermKind::Lookup { kind, collection, key, found, missing } => {
            operands.extend([ssa(*collection, OperandMode::Borrow), ssa(*key, OperandMode::Observe)]);
            let ty = match kind { LookupKind::Array => ResultType::Element(*collection), LookupKind::Map => ResultType::MapValue(*collection) };
            edge_definitions(block, EdgeKind::LookupFound, Outcome::Normal, &[ResultSlot::new(SlotName::Value, ty)], visit)?;
            invoke_edge(block, found, EdgeKind::LookupFound, site, visit); edge(block, missing, EdgeKind::LookupMissing, site, visit);
        }
        TermKind::Trap { .. } | TermKind::Unreachable => {},
    }
    emit_refs(operands, site, body, visit)
}
/// Includes unused declarations and annotations. Expression leaves can occur at
/// several sites; no use is deduplicated. Edge results have no source ValueId.
pub fn walk_body(body: &Body, registry: ContractRegistry<'_>, mut visit: impl FnMut(CfgEvent)) -> Result<(), TraversalError> {
    for binding in &body.descriptor_bindings {
        emit_refs([ssa(binding.dictionary, OperandMode::Observe)],
            UseSite::Terminator(body.entry), body, &mut visit)?;
    }
    for (id, _) in body.keys.iter() { let reference = OperandRef::Key(id); emit_refs([ReferencedOperand { reference, mode: OperandMode::Observe }], UseSite::Declaration(reference), body, &mut visit)?; }
    for (id, _) in body.deps.iter() { let reference = OperandRef::Dependency(id); emit_refs([ReferencedOperand { reference, mode: OperandMode::Observe }], UseSite::Declaration(reference), body, &mut visit)?; }
    for (id, _) in body.selections.iter() { let reference = OperandRef::Selection(id); emit_refs([ReferencedOperand { reference, mode: OperandMode::Observe }], UseSite::Declaration(reference), body, &mut visit)?; }
    for (value, data) in body.values.iter() {
        let operand = match &data.facts {
            ValueFacts::Managed { deps } => Some(ReferencedOperand { reference: OperandRef::Dependency(*deps), mode: OperandMode::Observe }),
            ValueFacts::Plain { certificate: PlainCertificate::Descriptor(value) } => Some(ssa(*value, OperandMode::Observe)),
            ValueFacts::Unelaborated | ValueFacts::Plain { .. } | ValueFacts::Service => None,
        };
        emit_refs(operand, UseSite::ValueFacts(value), body, &mut visit)?;
    }
    for (block_id, block) in body.blocks.iter() {
        for (index, &value) in block.params.iter().enumerate() {
            visit(CfgEvent::Definition { value, definition: Definition::BlockParam { block: block_id, index: u32::try_from(index).map_err(|_| TraversalError::TooManySlots)? } });
        }
        if let Some(unwind) = &block.unwind {
            edge(block_id, &unwind.target, EdgeKind::BlockUnwind, UseSite::BlockUnwind(block_id), &mut visit);
            edge_definitions(block_id, EdgeKind::BlockUnwind, Outcome::Unwind, &[ResultSlot::service(SlotName::Failure, ServiceType::Failure)], &mut visit)?;
        }
        for &inst_id in &block.insts {
            let inst = body.insts.get(inst_id)?;
            emit_refs(inst.op.operand_refs(), UseSite::Instruction(inst_id), body, &mut visit)?;
            let facts = inst.facts.result_deps.iter().map(|(_, id)| ReferencedOperand { reference: OperandRef::Dependency(*id), mode: OperandMode::Observe })
                .chain(inst.facts.retention.iter().chain(inst.facts.provenance.iter().map(|fact| &fact.obligations)).map(|id| ReferencedOperand { reference: OperandRef::Selection(*id), mode: OperandMode::Observe }));
            emit_refs(facts, UseSite::InstructionFacts(inst_id), body, &mut visit)?;
            for (index, &value) in inst.results.iter().enumerate() {
                visit(CfgEvent::Definition { value, definition: Definition::InstResult { inst: inst_id, index: u16::try_from(index).map_err(|_| TraversalError::TooManySlots)? } });
            }
        }
        for record in body.high_results.iter().filter(|record| record.block == block_id) {
            emit_refs([ssa(record.value, OperandMode::Observe)], UseSite::Terminator(block_id), body, &mut visit)?;
        }
        walk_terminator(block_id, &block.term, body, registry, &mut visit)?;
    }
    Ok(())
}
pub fn successors(block: BlockId, body: &Body, registry: ContractRegistry<'_>) -> Result<HashSet<BlockId>, TraversalError> {
    let mut targets = HashSet::new();
    let data = body.blocks.get(block)?;
    if let Some(unwind) = &data.unwind { targets.insert(unwind.target.block); }
    walk_terminator(block, &data.term, body, registry, &mut |event| { if let CfgEvent::Edge { target, .. } = event { targets.insert(target); } })?;
    Ok(targets)
}

/// Declarations without a body have no CFG or SSA definitions.
pub fn walk_function(function: &Function, registry: ContractRegistry<'_>, visit: impl FnMut(CfgEvent)) -> Result<(), TraversalError> {
    if let Some(body) = &function.body { walk_body(body, registry, visit)?; }
    Ok(())
}
