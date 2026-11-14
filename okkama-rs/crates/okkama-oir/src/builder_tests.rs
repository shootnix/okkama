use crate::*;
use crate::cfg::{self, CfgEvent, EdgeKind};
use crate::visit::{Rewrite, VisitorMut};
use std::collections::HashSet;

fn declaration(params: Vec<TypeId>, results: Vec<TypeId>) -> Function {
    Function { symbol: "test".into(), signature: Signature { params, results, ownership_abi: OwnershipAbi::Unelaborated },
        attrs: vec![], type_params: vec![], summary: None, async_creation_summary: None,
        async_body_summary: None, body: None, loc: None }
}
fn setup(profile: Profile) -> Result<(ModuleBuilder, FuncId, ScopeId), BuildError> {
    let mut module = ModuleBuilder::new("builder_tests", profile);
    let function = module.declare_function(declaration(vec![], vec![]))?;
    let scope = module.declare_scope(ScopeDecl { function, parent: None, kind: ScopeKind::Action, loc: None })?;
    Ok((module, function, scope))
}
fn int(value: &str) -> Operation { Operation::Const(Literal::Int(value.into())) }
fn events(body: &Body, module: &Module) -> Result<Vec<CfgEvent>, BuildError> {
    let mut result = vec![]; cfg::walk_body(body, ContractRegistry::new(module), |event| result.push(event))?; Ok(result)
}
fn assert_definitions(body: &Body, module: &Module) -> Result<(), BuildError> {
    for event in events(body, module)? {
        if let CfgEvent::Definition { value, definition } = event { assert_eq!(body.values.get(value)?.def, definition); }
    }
    Ok(())
}

#[test]
fn branches_and_loop_parameters_use_the_public_builder() -> Result<(), BuildError> {
    let (mut module, function, scope) = setup(Profile::High)?;
    let int_ty = module.intern_type(Type::Int)?;
    let bool_ty = module.intern_type(Type::Bool)?;
    module.raw_mut().functions.get_mut(function)?.signature = Signature {
        params: vec![bool_ty, int_ty], results: vec![int_ty], ownership_abi: OwnershipAbi::Unelaborated,
    };
    let mut b = module.function_builder(function, scope)?;
    let entry = b.entry();
    let args = b.block_params(entry)?.to_vec();
    let header = b.create_block(scope, &[ParamSpec::new(int_ty)])?;
    let body = b.create_block(scope, &[ParamSpec::new(int_ty)])?;
    let exit = b.create_block(scope, &[ParamSpec::new(int_ty)])?;
    b.terminate(TermKind::Br(Target { block: header, args: vec![args[1]] }))?;
    b.switch_to(header)?;
    let current = b.block_params(header)?[0];
    b.terminate(TermKind::Cond { condition: args[0], yes: Target { block: body, args: vec![current] }, no: Target { block: exit, args: vec![current] } })?;
    b.switch_to(body)?;
    let loop_value = b.block_params(body)?[0];
    let one = b.emit(int("1"), &[ResultSpec::new(int_ty)])?[0];
    let next = b.emit_schema(Operation::Core(CoreOp::IntAdd { left: loop_value, right: one }))?[0];
    b.terminate(TermKind::Br(Target { block: header, args: vec![next] }))?;
    b.switch_to(exit)?;
    b.terminate(TermKind::Ret { values: vec![b.block_params(exit)?[0]] })?;
    let body_ir = b.finish()?;
    assert_eq!(body_ir.values.len(), 7);
    assert_eq!(cfg::successors(header, &body_ir, ContractRegistry::new(module.module()))?, HashSet::from([body, exit]));
    assert_eq!(cfg::successors(body, &body_ir, ContractRegistry::new(module.module()))?, HashSet::from([header]));
    assert_definitions(&body_ir, module.module())?;
    module.define_function(function, body_ir)?;
    assert!(module.finish()?.functions.get(function)?.body.is_some());
    Ok(())
}

#[test]
fn ordinary_unwind_has_explicit_arguments_and_implicit_failure() -> Result<(), BuildError> {
    let (mut module, function, scope) = setup(Profile::High)?;
    let int_ty = module.intern_type(Type::Int)?;
    let failure_ty = module.intern_type(Type::Service(ServiceType::Failure))?;
    module.raw_mut().functions.get_mut(function)?.signature.params = vec![int_ty];
    let mut b = module.function_builder(function, scope)?;
    let entry = b.entry(); let value = b.block_params(entry)?[0];
    let failed = b.create_block(scope, &[ParamSpec::new(int_ty), ParamSpec::new(failure_ty)])?;
    b.set_unwind(Some(BlockUnwind { target: Target { block: failed, args: vec![value] } }))?;
    let result = b.emit_schema(Operation::Core(CoreOp::IntDiv { left: value, right: value }))?[0];
    assert_ne!(result, value);
    b.terminate(TermKind::Ret { values: vec![] })?;
    b.switch_to(failed)?; b.terminate(TermKind::Raise(b.block_params(failed)?[1]))?;
    let body = b.finish()?;
    assert_eq!(cfg::successors(entry, &body, ContractRegistry::new(module.module()))?, HashSet::from([failed]));
    let events = events(&body, module.module())?;
    assert!(events.iter().any(|event| matches!(event, CfgEvent::EdgeDefinition { kind: EdgeKind::BlockUnwind, result: EdgeResult { outcome: Outcome::Unwind, slot: 0 }, .. })));
    Ok(())
}

#[test]
fn invoke_successors_match_split_close_buffer_call_and_await_schemas() -> Result<(), BuildError> {
    for case in 0..5 {
        let (mut module, function, scope) = setup(Profile::Own)?;
        let int_ty = module.intern_type(Type::Int)?;
        let future_ty = module.intern_type(Type::Future(Some(int_ty)))?;
        module.raw_mut().functions.get_mut(function)?.signature.params = vec![future_ty];
        let summary = module.add_summary(Summary { entry_kind: EntryKind::CallBoundary, parameters: vec![], results: vec![],
            contract: CallContract { results: vec![int_ty], failures: vec![FailureKind::Program, FailureKind::Cancelled],
                suspends: false, async_destroy: false, publication: Publication::Atomic, protocol: None } })?;
        let mut b = module.function_builder(function, scope)?;
        let packet = LoweringBuilder::new(&mut b).own_empty()?;
        let value = b.emit_schema(int("3"))?[0];
        let selection = b.declare_selection(SelectionExpr::Value(value))?;
        let op = match case {
            0 => Operation::Ownership(OwnershipOp::SplitChecked { packet, selection }),
            1 => { let state = LoweringBuilder::new(&mut b).scope_begin(scope)?; Operation::Ownership(OwnershipOp::ScopeEnd { state }) }
            2 => { let buffer = b.emit_schema(Operation::Collection(CollectionOp::BuilderNew { element: int_ty }))?[0];
                Operation::Collection(CollectionOp::BuilderPush { buffer, value, packet: Some(packet) }) }
            3 => Operation::Call(CallOp::Direct { function, call: CallSite { args: vec![], packet: Some(packet), deps: vec![], descriptors: vec![], type_args: vec![], summary } }),
            _ => Operation::Async(AsyncOp::Await { future: b.block_params(b.entry())?[0], result: Some(int_ty) }),
        };
        let contract = ContractRegistry::new(b.module()).lookup(&op)?;
        let normal_specs = b.result_schema(&op, Outcome::Normal)?;
        let unwind_specs = b.result_schema(&op, Outcome::Unwind)?;
        let edge = b.invoke_own_successors(op, scope, scope)?;
        let expected_names: (&[SlotName], &[SlotName]) = match case {
            0 => (&[SlotName::Rest, SlotName::Selected, SlotName::ExternalDeps], &[SlotName::Packet, SlotName::Failure]),
            1 => (&[SlotName::Closed], &[SlotName::Closed, SlotName::Failure]),
            2 => (&[SlotName::Buffer, SlotName::Packet], &[SlotName::Buffer, SlotName::Packet, SlotName::Failure]),
            3 => (&[SlotName::Value, SlotName::Packet, SlotName::Deps], &[SlotName::Failure]),
            _ => (&[SlotName::Value, SlotName::Deps], &[SlotName::Failure, SlotName::Task, SlotName::Receipt]),
        };
        assert_eq!(contract.normal.results.iter().map(|slot| slot.name).collect::<Vec<_>>(), expected_names.0);
        assert_eq!(contract.unwind.as_ref().unwrap().results.iter().map(|slot| slot.name).collect::<Vec<_>>(), expected_names.1);
        assert_eq!(edge.normal_params.len(), expected_names.0.len());
        assert_eq!(edge.unwind_params.len(), expected_names.1.len());
        for (params, specs) in [(&edge.normal_params, &normal_specs), (&edge.unwind_params, &unwind_specs)] {
            for (&value, spec) in params.iter().zip(specs) {
                assert_eq!(b.body().values.get(value)?.ty, spec.ty);
                assert!(matches!(b.body().values.get(value)?.def, Definition::BlockParam { .. }));
            }
        }
        b.switch_to(edge.normal)?; b.terminate(TermKind::Ret { values: vec![] })?;
        b.switch_to(edge.unwind)?;
        let failure_slot = contract.unwind.as_ref().unwrap().results.iter().position(|slot| slot.name == SlotName::Failure).unwrap();
        b.terminate(TermKind::Raise(edge.unwind_params[failure_slot]))?;
        let body = b.finish()?;
        assert_eq!(cfg::successors(body.entry, &body, ContractRegistry::new(module.module()))?, HashSet::from([edge.normal, edge.unwind]));
        assert_definitions(&body, module.module())?;
    }
    Ok(())
}

#[test]
fn emit_rejects_wrong_schema_and_requires_complete_blocks() -> Result<(), BuildError> {
    let (mut module, function, scope) = setup(Profile::Own)?;
    let int_ty = module.intern_type(Type::Int)?;
    let bool_ty = module.intern_type(Type::Bool)?;
    let own_ty = module.intern_type(Type::Service(ServiceType::Own))?;
    let mut b = module.function_builder(function, scope)?;
    let before = b.body().clone();
    assert!(matches!(b.emit(int("1"), &[]), Err(BuildError::ResultArity { .. })));
    assert!(matches!(b.emit(int("1"), &[ResultSpec::new(bool_ty)]), Err(BuildError::ResultType { .. })));
    assert!(matches!(b.emit(Operation::Ownership(OwnershipOp::Empty {}), &[ResultSpec::new(int_ty)]), Err(BuildError::ResultType { .. })));
    assert!(matches!(b.emit(int("1"), &[ResultSpec::new(int_ty), ResultSpec::new(own_ty)]), Err(BuildError::ResultArity { .. })));
    assert_eq!(b.body(), &before);
    let block = b.entry();
    assert_eq!(b.finish(), Err(BuildError::Unterminated(vec![block])));
    Ok(())
}

#[test]
fn high_wrappers_record_intent_and_own_wrappers_follow_registry() -> Result<(), BuildError> {
    let (mut module, function, scope) = setup(Profile::High)?;
    let int_ty = module.intern_type(Type::Int)?;
    let object = module.declare(Declaration { name: "Holder".into(), type_params: vec![], loc: None,
        kind: DeclarationKind::Object { fields: vec![Field { name: "value".into(), ty: int_ty, default: None, once: false }], witnesses: vec![], destroy: None, builtin: false } })?;
    let mut b = module.function_builder(function, scope)?;
    let value = b.emit_schema(int("42"))?[0];
    let selection = b.declare_selection(SelectionExpr::Value(value))?;
    let binding = b.declare_binding(BindingDecl { name: "x".into(), scope, ty: int_ty, loc: None })?;
    let mut lower = LoweringBuilder::new(&mut b);
    lower.bind_high(binding, value)?; lower.rebind_high(binding, value, scope)?;
    lower.stage_high(scope, FieldRef { declaration: object, index: 0 }, value)?;
    lower.demand(value, ResultDemand::Want)?; lower.demand(value, ResultDemand::Discard)?;
    let state = lower.scope_begin(scope)?; let packet = lower.own_empty()?;
    let split = lower.own_split(packet, selection)?;
    let state = lower.own_admit(state, split.rest)?;
    let state = lower.own_admit(state, split.selected)?;
    let next = b.create_block(scope, &[])?;
    b.terminate(TermKind::Br(Target { block: next, args: vec![] }))?;
    b.switch_to(next)?;
    let edges = b.invoke_own_successors(Operation::Ownership(OwnershipOp::ScopeEnd { state }), scope, scope)?;
    b.switch_to(edges.normal)?; b.terminate(TermKind::Ret { values: vec![] })?;
    b.switch_to(edges.unwind)?; b.terminate(TermKind::Raise(edges.unwind_params[1]))?;
    let body = b.finish()?;
    assert_eq!(body.insts.iter().map(|(_, inst)| inst.op.opcode()).collect::<Vec<_>>(),
        ["const", "bind", "rebind", "stage", "want", "discard", "scope.begin", "own.empty", "own.split", "own.admit", "own.admit"]);
    assert_eq!(body.values.get(split.external_deps)?.ty, module.intern_type(Type::Service(ServiceType::Deps))?);
    Ok(())
}

#[test]
fn checked_scope_split_wrapper_uses_entry_unwind_and_blocks_stale_state() -> Result<(), BuildError> {
    let (mut module, function, scope) = setup(Profile::Own)?;
    let state_ty = module.intern_type(Type::Service(ServiceType::Scope(scope)))?;
    let failure_ty = module.intern_type(Type::Service(ServiceType::Failure))?;
    let mut b = module.function_builder(function, scope)?;
    let state = LoweringBuilder::new(&mut b).scope_begin(scope)?;
    let selection = b.declare_selection(SelectionExpr::Union(vec![]))?;
    let split_block = b.create_block(scope, &[ParamSpec::new(state_ty)])?;
    let failed = b.create_block(scope, &[ParamSpec::new(state_ty), ParamSpec::new(failure_ty)])?;
    b.terminate(TermKind::Br(Target { block: split_block, args: vec![state] }))?;
    b.switch_to(split_block)?;
    let state = b.block_params(split_block)?[0];
    b.set_unwind(Some(BlockUnwind { target: Target { block: failed, args: vec![state] } }))?;
    let split = LoweringBuilder::new(&mut b).scope_split(state, selection)?;
    assert_eq!(b.body().values.get(split.state)?.ty, state_ty);
    let value = b.emit_schema(int("2"))?[0];
    assert_eq!(b.emit_schema(Operation::Core(CoreOp::IntDiv { left: value, right: value })), Err(BuildError::NeedsNewBlock(split_block)));
    let next = b.create_block(scope, &[])?;
    b.terminate(TermKind::Br(Target { block: next, args: vec![] }))?;
    b.switch_to(next)?;
    assert_eq!(b.emit_schema(Operation::Ownership(OwnershipOp::ScopeEnd { state: split.state })), Err(BuildError::NeedsInvoke));
    b.terminate(TermKind::Unreachable)?;
    b.switch_to(failed)?; b.terminate(TermKind::Raise(b.block_params(failed)?[1]))?;
    b.finish()?;
    Ok(())
}

#[test]
fn malformed_edges_are_rejected_without_terminating_the_block() -> Result<(), BuildError> {
    let (mut module, function, scope) = setup(Profile::High)?;
    let int_ty = module.intern_type(Type::Int)?;
    let mut b = module.function_builder(function, scope)?;
    let target = b.create_block(scope, &[ParamSpec::new(int_ty)])?;
    let before = b.body().clone();
    assert!(matches!(b.terminate(TermKind::Br(Target { block: target, args: vec![] })), Err(BuildError::EdgeArity { .. })));
    assert_eq!(b.body(), &before);
    let value = b.emit_schema(int("1"))?[0];
    b.terminate(TermKind::Br(Target { block: target, args: vec![value] }))?;
    assert_eq!(b.emit_schema(int("2")), Err(BuildError::Terminated(b.entry())));
    b.switch_to(target)?; b.terminate(TermKind::Ret { values: vec![] })?;
    b.finish()?;
    Ok(())
}

#[test]
fn invalid_ir_is_constructible_by_explicit_structure_editing() -> Result<(), BuildError> {
    let (mut module, function, scope) = setup(Profile::High)?;
    let mut b = module.function_builder(function, scope)?;
    b.emit_schema(int("1"))?;
    b.terminate(TermKind::Ret { values: vec![] })?;
    let mut body = b.finish()?;
    let mut editor = FunctionEditor::new(module.raw_mut(), &mut body);
    // Deliberately violate const's schema, then put a dangling target in the CFG.
    editor.raw_mut().insts.get_mut(InstId::from_u32(0))?.results.clear();
    editor.raw_mut().blocks.get_mut(BlockId::from_u32(0))?.term.kind = TermKind::Br(Target { block: BlockId::from_u32(999), args: vec![] });
    module.define_function(function, body)?;
    let module = module.finish()?;
    let body = module.functions.get(function)?.body.as_ref().unwrap();
    assert!(body.insts.get(InstId::from_u32(0))?.results.is_empty());
    assert!(body.blocks.get(BlockId::from_u32(999)).is_err());
    // finish() deliberately has not certified this malformed IR as verified.
    Ok(())
}

#[test]
fn editor_insert_replace_split_uses_edges_and_renumber_keep_cfg_consistent() -> Result<(), BuildError> {
    let (mut module, function, scope) = setup(Profile::High)?;
    let int_ty = module.intern_type(Type::Int)?;
    let file = module.add_file(SourceFileDecl { path: "test.ok".into() })?;
    let loc = Some(DebugLoc { file, line: 4, column: 2 });
    let mut b = module.function_builder(function, scope)?;
    let entry = b.entry();
    let target = b.create_block(scope, &[ParamSpec::new(int_ty)])?;
    b.set_location(loc);
    let old = b.emit_schema(int("1"))?[0];
    let key = b.declare_key(KeyExpr::Created { site: CreationSiteId::from_u32(7), instance: old })?;
    let dep = b.intern_dependency(DepExpr::Key(key))?;
    assert_eq!(b.intern_dependency(DepExpr::Key(key))?, dep);
    let selection = b.declare_selection(SelectionExpr::Projection(Projection::Index { value: old, index: old }))?;
    let ret_dep = b.intern_dependency(DepExpr::Retention { value: old, selection })?;
    let facts = InstructionFacts { result_deps: vec![(0, dep)], retention: vec![selection],
        provenance: vec![ProvenanceFact { obligations: selection, state: AdmissionProvenance::Bound }] };
    let value = b.emit_with_facts(Operation::Core(CoreOp::IntAdd { left: old, right: old }),
        &[ResultSpec { ty: int_ty, facts: ValueFacts::Managed { deps: ret_dep } }], facts, )?[0];
    let add = match b.body().values.get(value)?.def { Definition::InstResult { inst, .. } => inst, _ => unreachable!() };
    b.terminate(TermKind::Br(Target { block: target, args: vec![value] }))?;
    b.switch_to(target)?; b.terminate(TermKind::Ret { values: vec![] })?;
    let mut body = b.finish()?;
    let mut editor = FunctionEditor::new(module.raw_mut(), &mut body);
    let (inserted, values) = editor.insert_instruction(entry, 0, int("9"), &[ResultSpec::new(int_ty)], InstructionFacts::default(), loc)?;
    let replacement = values[0];
    assert!(inserted.as_u32() > add.as_u32());
    editor.replace_instruction(add, Operation::Core(CoreOp::IntSub { left: old, right: old }))?;
    assert!(matches!(editor.replace_instruction(add, Operation::Const(Literal::Bool(true))), Err(BuildError::ResultType { .. })));
    editor.replace_uses(old, replacement)?;
    let suffix = editor.split_block(entry, 2)?;
    assert_eq!(editor.body().insts.get(add)?.op, Operation::Core(CoreOp::IntSub { left: replacement, right: replacement }));
    editor.replace_edge_arguments(suffix, EdgeKind::Branch, vec![EdgeArg::Value(replacement)])?;
    assert!(matches!(editor.replace_edge_arguments(suffix, EdgeKind::Branch, vec![]), Err(BuildError::EdgeArity { .. })));
    struct Relocate;
    impl VisitorMut for Relocate {
        fn location(&mut self, loc: &mut DebugLoc) { loc.line += 10; }
    }
    editor.rewrite(&mut Relocate);
    let remap = editor.renumber()?;
    assert_eq!(remap.instructions[&inserted], InstId::from_u32(0));
    assert_ne!(remap.values[&replacement], replacement);
    let mapped = remap.values[&replacement];
    assert_eq!(editor.body().keys.get(key)?, &KeyExpr::Created { site: CreationSiteId::from_u32(7), instance: mapped });
    assert_eq!(editor.body().selections.get(selection)?, &SelectionExpr::Projection(Projection::Index { value: mapped, index: mapped }));
    assert_eq!(editor.body().deps.get(ret_dep)?, &DepExpr::Retention { value: mapped, selection });
    assert_eq!(editor.body().insts.get(remap.instructions[&add])?.loc.unwrap().line, 14);
    let module = module.module();
    let uses = events(&body, module)?;
    assert!(!uses.iter().any(|event| matches!(event, CfgEvent::Use { operand, .. } if operand.value == remap.values[&old])));
    assert_eq!(cfg::successors(entry, &body, ContractRegistry::new(module))?, HashSet::from([suffix]));
    assert_eq!(cfg::successors(suffix, &body, ContractRegistry::new(module))?, HashSet::from([target]));
    assert_definitions(&body, module)?;
    Ok(())
}

#[test]
fn mutable_operation_visitor_covers_every_registered_payload() {
    let mut operations = vec![Operation::Const(Literal::Int("1".into()))];
    operations.extend(CoreOp::representatives().into_iter().map(Operation::Core));
    operations.extend(LexicalOp::representatives().into_iter().map(Operation::Lexical));
    operations.extend(OwnershipOp::representatives().into_iter().map(Operation::Ownership));
    operations.extend(CallOp::representatives().into_iter().map(Operation::Call));
    operations.extend(CollectionOp::representatives().into_iter().map(Operation::Collection));
    operations.extend(AsyncOp::representatives().into_iter().map(Operation::Async));
    operations.extend(CarrierOp::representatives().into_iter().map(Operation::Carrier));
    operations.extend(ExitOp::representatives().into_iter().map(Operation::Exit));
    operations.extend(CleanupOp::representatives().into_iter().map(Operation::Cleanup));
    struct Shift;
    impl VisitorMut for Shift {
        fn value(&mut self, id: &mut ValueId) { *id = ValueId::from_u32(id.as_u32() + 100); }
        fn selection(&mut self, id: &mut SelectionId) { *id = SelectionId::from_u32(id.as_u32() + 100); }
        fn dependency(&mut self, id: &mut DepExprId) { *id = DepExprId::from_u32(id.as_u32() + 100); }
        fn key(&mut self, id: &mut KeyExprId) { *id = KeyExprId::from_u32(id.as_u32() + 100); }
    }
    assert_eq!(operations.len(), Operation::opcode_inventory().len());
    for mut op in operations {
        let mut expected = op.operand_refs();
        for operand in &mut expected { match &mut operand.reference {
            OperandRef::Value(id) => Shift.value(id), OperandRef::Selection(id) => Shift.selection(id),
            OperandRef::Dependency(id) => Shift.dependency(id), OperandRef::Key(id) => Shift.key(id),
        } }
        op.rewrite(&mut Shift);
        assert_eq!(op.operand_refs(), expected, "{}", op.opcode());
    }
}

#[test]
fn editor_preserves_unwind_and_checks_invoke_edge_results() -> Result<(), BuildError> {
    let (mut module, function, scope) = setup(Profile::Own)?;
    let int_ty = module.intern_type(Type::Int)?;
    let failure_ty = module.intern_type(Type::Service(ServiceType::Failure))?;
    module.raw_mut().functions.get_mut(function)?.signature.params = vec![int_ty, int_ty];
    let mut b = module.function_builder(function, scope)?;
    let entry = b.entry(); let inputs = b.block_params(entry)?.to_vec();
    let failed = b.create_block(scope, &[ParamSpec::new(int_ty), ParamSpec::new(failure_ty)])?;
    b.set_unwind(Some(BlockUnwind { target: Target { block: failed, args: vec![inputs[0]] } }))?;
    b.emit_schema(Operation::Core(CoreOp::IntDiv { left: inputs[0], right: inputs[1] }))?;
    let state = LoweringBuilder::new(&mut b).scope_begin(scope)?;
    let edges = b.invoke_own_successors(Operation::Ownership(OwnershipOp::ScopeEnd { state }), scope, scope)?;
    b.switch_to(failed)?; b.terminate(TermKind::Raise(b.block_params(failed)?[1]))?;
    b.switch_to(edges.normal)?; b.terminate(TermKind::Ret { values: vec![] })?;
    b.switch_to(edges.unwind)?; b.terminate(TermKind::Raise(edges.unwind_params[1]))?;
    let mut body = b.finish()?;
    let mut editor = FunctionEditor::new(module.raw_mut(), &mut body);
    let suffix = editor.split_block(entry, 1)?;
    editor.replace_edge_arguments(suffix, EdgeKind::BlockUnwind, vec![EdgeArg::Value(inputs[1])])?;
    let before = editor.body().clone();
    let wrong = EdgeResult { outcome: Outcome::Unwind, slot: 0 };
    assert_eq!(editor.replace_edge_arguments(suffix, EdgeKind::InvokeNormal, vec![EdgeArg::Produced(wrong)]), Err(BuildError::InvalidEdgeResult(wrong)));
    assert_eq!(editor.body(), &before);
    let nonexistent = EdgeResult { outcome: Outcome::Normal, slot: 1 };
    assert_eq!(editor.replace_edge_arguments(suffix, EdgeKind::InvokeNormal, vec![EdgeArg::Produced(nonexistent)]), Err(BuildError::InvalidEdgeResult(nonexistent)));
    editor.replace_edge_arguments(suffix, EdgeKind::InvokeNormal, vec![EdgeArg::Produced(EdgeResult { outcome: Outcome::Normal, slot: 0 })])?;
    let registry = ContractRegistry::new(module.module());
    assert_eq!(cfg::successors(entry, &body, registry)?, HashSet::from([suffix, failed]));
    assert_eq!(cfg::successors(suffix, &body, registry)?, HashSet::from([edges.normal, edges.unwind, failed]));
    assert_definitions(&body, module.module())?;
    Ok(())
}

#[test]
fn renumber_rewrites_proof_creation_aliases_and_frame_slot_payloads() -> Result<(), BuildError> {
    let (mut module, function, scope) = setup(Profile::Coro)?;
    let int_ty = module.intern_type(Type::Int)?;
    let future_ty = module.intern_type(Type::Future(Some(int_ty)))?;
    let mut b = module.function_builder(function, scope)?;
    let entry = b.entry();
    let future = b.emit_schema(Operation::Core(CoreOp::Undef { ty: future_ty }))?[0];
    let creation = match b.body().values.get(future)?.def { Definition::InstResult { inst, .. } => inst, _ => unreachable!() };
    let frame = b.emit_schema(Operation::Async(AsyncOp::FrameAlloc { function }))?[0];
    let proof = UniqueCacheProof { future, scope, region: vec![entry], evidence: UniqueCacheEvidence {
        creation, aliases: vec![future], frame_slots: vec![4], no_external_storage: true,
        no_other_waiters: true, no_live_views: true, no_collector_retention: true, scheduler_cannot_publish: true } };
    let unique = b.emit_schema(Operation::Async(AsyncOp::UniqueCacheProof { proof }))?[0];
    b.emit_schema(Operation::Async(AsyncOp::FrameSave { frame, slots: vec![FrameSlot { slot: 4, value: unique, mode: OperandMode::Consume }], continuation: 3 }))?;
    b.terminate(TermKind::Ret { values: vec![] })?;
    let mut body = b.finish()?;
    let mut editor = FunctionEditor::new(module.raw_mut(), &mut body);
    editor.insert_instruction(entry, 0, int("7"), &[ResultSpec::new(int_ty)], InstructionFacts::default(), None)?;
    let map = editor.renumber()?;
    let proof = editor.body().insts.iter().find_map(|(_, inst)| match &inst.op {
        Operation::Async(AsyncOp::UniqueCacheProof { proof }) => Some(proof), _ => None,
    }).unwrap();
    assert_eq!(proof.future, map.values[&future]);
    assert_eq!(proof.evidence.aliases, [map.values[&future]]);
    assert_eq!(proof.evidence.creation, map.instructions[&creation]);
    assert_eq!(proof.evidence.frame_slots, [4]);
    assert_eq!(proof.region, [entry]);
    let saved = editor.body().insts.iter().find_map(|(_, inst)| match &inst.op {
        Operation::Async(AsyncOp::FrameSave { slots, .. }) => Some(slots), _ => None,
    }).unwrap();
    assert_eq!(saved[0].value, map.values[&unique]);
    assert_definitions(&body, module.module())?;
    Ok(())
}

#[test]
fn result_type_formulas_resolve_types_without_ownership_inference() -> Result<(), BuildError> {
    let (mut module, function, scope) = setup(Profile::High)?;
    let int_ty = module.intern_type(Type::Int)?;
    let bool_ty = module.intern_type(Type::Bool)?;
    let array_ty = module.intern_type(Type::Array(int_ty))?;
    let map_ty = module.intern_type(Type::Map(int_ty, bool_ty))?;
    let buffer_ty = module.intern_type(Type::Builder(int_ty))?;
    let mbuffer_ty = module.intern_type(Type::MapBuilder(int_ty, bool_ty))?;
    let future_ty = module.intern_type(Type::Future(Some(int_ty)))?;
    let cell_ty = module.intern_type(Type::LowCell(bool_ty))?;
    let object = module.declare(Declaration { name: "Object".into(), type_params: vec![], loc: None,
        kind: DeclarationKind::Object { fields: vec![Field { name: "field".into(), ty: bool_ty, default: None, once: false }], witnesses: vec![], destroy: None, builtin: false } })?;
    module.raw_mut().functions.get_mut(function)?.signature.params = vec![array_ty, map_ty, buffer_ty, mbuffer_ty, future_ty, cell_ty];
    let mut b = module.function_builder(function, scope)?;
    let p = b.block_params(b.entry())?.to_vec();
    b.set_value_facts(p[0], ValueFacts::Plain { certificate: PlainCertificate::ClosedGraph { ty: array_ty } })?;
    b.terminate(TermKind::Ret { values: vec![] })?;
    let body = b.finish()?;
    let field = FieldRef { declaration: object, index: 0 };
    for (formula, expected) in [
        (ResultType::Exact(int_ty), int_ty), (ResultType::SameAs(p[0]), array_ty),
        (ResultType::Element(p[0]), int_ty), (ResultType::MapKey(p[1]), int_ty),
        (ResultType::MapValue(p[1]), bool_ty), (ResultType::Field(field), bool_ty),
        (ResultType::ArrayOf(Box::new(ResultType::MapKey(p[1]))), array_ty),
        (ResultType::FutureOf(Some(int_ty)), future_ty), (ResultType::Awaited(p[4]), int_ty),
        (ResultType::BuilderOf(int_ty), buffer_ty), (ResultType::MapBuilderOf(int_ty, bool_ty), mbuffer_ty),
        (ResultType::Finished(p[2]), array_ty), (ResultType::Finished(p[3]), map_ty),
        (ResultType::CellOf(bool_ty), cell_ty), (ResultType::CellValue(p[5]), bool_ty),
    ] {
        assert_eq!(resolve_result_type(module.raw_mut(), &body, &formula)?, expected);
    }
    assert!(matches!(resolve_result_type(module.raw_mut(), &body, &ResultType::MapKey(p[0])), Err(BuildError::UnresolvableType(_))));
    assert_eq!(resolve_result_type(module.raw_mut(), &body, &ResultType::Field(FieldRef { declaration: object, index: 99 })), Err(BuildError::InvalidField(FieldRef { declaration: object, index: 99 })));
    Ok(())
}
