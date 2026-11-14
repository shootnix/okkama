use super::*;

fn function(symbol: &str, params: Vec<TypeId>, results: Vec<TypeId>) -> Function {
    Function {
        symbol: symbol.into(), signature: Signature { params, results, ownership_abi: OwnershipAbi::Unelaborated },
        attrs: vec![], type_params: vec![], summary: None, async_creation_summary: None,
        async_body_summary: None, body: None, loc: None,
    }
}
fn arithmetic() -> Module {
    let mut module = ModuleBuilder::new("verify", Profile::High);
    let int = module.intern_type(Type::Int).unwrap();
    let function = module.declare_function(function("sum", vec![int, int], vec![int])).unwrap();
    let scope = module.declare_scope(ScopeDecl {
        function, parent: None, kind: ScopeKind::Action, loc: None,
    }).unwrap();
    let mut builder = module.function_builder(function, scope).unwrap();
    let params = builder.block_params(builder.entry()).unwrap().to_vec();
    let result = builder.emit_schema(Operation::Core(CoreOp::IntAdd { left: params[0], right: params[1] })).unwrap();
    builder.terminate(TermKind::Ret { values: result }).unwrap();
    let body = builder.finish().unwrap();
    module.define_function(function, body).unwrap();
    module.finish().unwrap()
}
fn fid() -> FuncId { FuncId::from_u32(0) }
fn body(module: &Module) -> &Body { module.functions.get(fid()).unwrap().body.as_ref().unwrap() }
fn body_mut(module: &mut Module) -> &mut Body { module.functions.get_mut(fid()).unwrap().body.as_mut().unwrap() }
fn rejected(module: &Module, rule: RuleId) -> VerifyReport {
    let report = verify(module).unwrap_err();
    assert_eq!(report.failure, VerifyFailure::Rejected, "{report:?}");
    assert!(report.diagnostics.iter().any(|diagnostic| diagnostic.rule == rule), "{report:?}");
    report
}
fn own_summary(results: Vec<TypeId>) -> Summary {
    Summary {
        entry_kind: EntryKind::CallBoundary, parameters: vec![], results: vec![],
        contract: CallContract {
            results, failures: vec![], suspends: false, async_destroy: false,
            publication: Publication::Atomic, protocol: None,
        },
    }
}

#[test]
fn public_apis_are_read_only_and_require_exact_profiles() {
    let mut module = arithmetic();
    let before = module.clone();
    let verified = verify_high(&module).unwrap();
    assert_eq!(verified.module(), &before);
    assert_eq!(verified.profile(), Profile::High);
    assert_eq!(verified.stats().functions, 1);
    assert_eq!(module, before);
    assert_eq!(verify_own(&module).unwrap_err().diagnostics[0].rule, RuleId::S1);
    for profile in [Profile::Own, Profile::Coro, Profile::Low] {
        module.profile = profile;
        match profile {
            Profile::Own => { verify_own(&module).unwrap(); }
            Profile::Coro => { verify_coro(&module).unwrap(); }
            Profile::Low => { verify_low(&module).unwrap(); }
            Profile::High => unreachable!(),
        }
    }
    let report = verify_function(&module, fid(), VerifyContext::default(), &VerifyOptions::default());
    assert_eq!(report.failure, None);
    let report = verify_function(&module, FuncId::from_u32(99), VerifyContext::default(), &VerifyOptions::default());
    assert_eq!(report.failure, Some(VerifyFailure::Rejected));
}

#[test]
fn s1_entry_forms_symbols_locations_and_unused_declarations() {
    let mut module = arithmetic();
    module.functions.get_mut(fid()).unwrap().attrs.push(FunctionAttr::Entry);
    rejected(&module, RuleId::S1);
    let mut module = arithmetic();
    module.functions.get_mut(fid()).unwrap().attrs.push(FunctionAttr::Extern("native.sum".into()));
    rejected(&module, RuleId::S1);
    let mut module = arithmetic();
    let duplicate = module.functions.get(fid()).unwrap().clone();
    module.functions.alloc(duplicate).unwrap();
    rejected(&module, RuleId::S1);
    let mut module = arithmetic();
    module.declarations.alloc(Declaration {
        name: "E".into(), type_params: vec![], loc: None,
        kind: DeclarationKind::Enum { cases: vec!["a".into(), "a".into()], is_error: false },
    }).unwrap();
    rejected(&module, RuleId::S1);
    let mut module = arithmetic();
    module.functions.get_mut(fid()).unwrap().loc = Some(DebugLoc { file: FileId::from_u32(9), line: 0, column: 0 });
    rejected(&module, RuleId::S1);
    let mut module = arithmetic();
    let future = TypeId::from_u32(module.types.len() as u32);
    module.types.intern(Type::Array(future)).unwrap();
    rejected(&module, RuleId::S1);
}

#[test]
fn s2_scope_ownership_parents_boundaries_and_expression_cycles() {
    let mut module = arithmetic();
    let root = ScopeId::from_u32(0);
    let nested = module.scopes.alloc(ScopeDecl {
        function: fid(), parent: Some(root), kind: ScopeKind::Block, loc: None,
    }).unwrap();
    body_mut(&mut module).boundaries.alloc(Boundary { scope: nested, kind: BoundaryKind::Lexical }).unwrap();
    verify(&module).unwrap();
    module.scopes.get_mut(root).unwrap().parent = Some(nested);
    rejected(&module, RuleId::S2);
    let mut module = arithmetic();
    module.scopes.get_mut(root).unwrap().function = FuncId::from_u32(99);
    rejected(&module, RuleId::S2);
    let mut module = arithmetic();
    let selection = SelectionId::from_u32(0);
    body_mut(&mut module).selections.alloc(SelectionExpr::Union(vec![selection])).unwrap();
    rejected(&module, RuleId::S2);
    let mut module = arithmetic();
    let dependency = DepExprId::from_u32(0);
    body_mut(&mut module).deps.alloc(DepExpr::Union(vec![dependency])).unwrap();
    rejected(&module, RuleId::S2);
}

#[test]
fn s3_definition_membership_entry_and_unreachable_shape() {
    let mut module = arithmetic();
    let entry = body(&module).blocks.get(body(&module).entry).unwrap().clone();
    let mut tombstone = entry;
    tombstone.params.clear();
    tombstone.insts.clear();
    tombstone.term.kind = TermKind::Unreachable;
    let dead = body_mut(&mut module).blocks.alloc(tombstone).unwrap();
    verify(&module).unwrap();
    body_mut(&mut module).blocks.get_mut(dead).unwrap().term.kind = TermKind::Ret { values: vec![] };
    rejected(&module, RuleId::S3);
    let mut module = arithmetic();
    body_mut(&mut module).blocks.get_mut(BlockId::from_u32(0)).unwrap().insts.push(InstId::from_u32(0));
    rejected(&module, RuleId::S3);
    let mut module = arithmetic();
    body_mut(&mut module).values.get_mut(ValueId::from_u32(2)).unwrap().def = Definition::InstResult {
        inst: InstId::from_u32(0), index: 7,
    };
    rejected(&module, RuleId::S3);
    let mut module = arithmetic();
    module.functions.get_mut(fid()).unwrap().signature.params.pop();
    rejected(&module, RuleId::S3);
}

#[test]
fn malformed_arenas_never_panic_and_other_functions_are_checked() {
    for index in [3, 50, u32::MAX] {
        let mut module = arithmetic();
        body_mut(&mut module).insts.get_mut(InstId::from_u32(0)).unwrap().op = Operation::Core(CoreOp::IntAdd {
            left: ValueId::from_u32(index), right: ValueId::from_u32(0),
        });
        rejected(&module, RuleId::S3);
        let mut module = arithmetic();
        body_mut(&mut module).entry = BlockId::from_u32(index);
        rejected(&module, RuleId::S3);
        let mut module = arithmetic();
        body_mut(&mut module).values.get_mut(ValueId::from_u32(0)).unwrap().ty = TypeId::from_u32(index);
        rejected(&module, RuleId::S1);
    }
    let mut module = arithmetic();
    let mut other = module.functions.get(fid()).unwrap().clone();
    other.symbol = "second".into();
    other.body.as_mut().unwrap().insts.get_mut(InstId::from_u32(0)).unwrap().results.clear();
    let second = module.functions.alloc(other).unwrap();
    let scope = module.scopes.alloc(ScopeDecl {
        function: second, parent: None, kind: ScopeKind::Action, loc: None,
    }).unwrap();
    module.functions.get_mut(second).unwrap().body.as_mut().unwrap().blocks
        .get_mut(BlockId::from_u32(0)).unwrap().lexical_scope = scope;
    body_mut(&mut module).entry = BlockId::from_u32(u32::MAX);
    let report = rejected(&module, RuleId::T2);
    assert!(report.diagnostics.iter().any(|diag| matches!(diag.primary.entity,
        EntityLocation::Operand { site: OperationSite::Instruction { function, .. }, .. } if function == second)));
}

#[test]
fn t1_operands_literals_await_and_internal_values() {
    let mut module = arithmetic();
    let boolean = module.types.intern(Type::Bool).unwrap();
    body_mut(&mut module).values.get_mut(ValueId::from_u32(0)).unwrap().ty = boolean;
    module.functions.get_mut(fid()).unwrap().signature.params[0] = boolean;
    let report = rejected(&module, RuleId::T1);
    assert!(report.diagnostics.iter().any(|diag| matches!(diag.primary.entity, EntityLocation::Operand { .. })));
    let mut module = arithmetic();
    body_mut(&mut module).insts.get_mut(InstId::from_u32(0)).unwrap().op = Operation::Const(Literal::Int("01".into()));
    rejected(&module, RuleId::T1);
    let mut module = arithmetic();
    let int = module.types.intern(Type::Int).unwrap();
    let future = module.types.intern(Type::Future(Some(int))).unwrap();
    body_mut(&mut module).values.get_mut(ValueId::from_u32(0)).unwrap().ty = future;
    module.functions.get_mut(fid()).unwrap().signature.params[0] = future;
    body_mut(&mut module).insts.get_mut(InstId::from_u32(0)).unwrap().op = Operation::Async(AsyncOp::Await {
        future: ValueId::from_u32(0), result: None,
    });
    rejected(&module, RuleId::T1);
    let mut module = arithmetic();
    let own = module.types.intern(Type::Service(ServiceType::Own)).unwrap();
    module.types.intern(Type::Array(own)).unwrap();
    rejected(&module, RuleId::T1);
}

#[test]
fn t2_tuple_arity_and_read_only_result_formulas() {
    let mut module = arithmetic();
    body_mut(&mut module).insts.get_mut(InstId::from_u32(0)).unwrap().results.pop();
    rejected(&module, RuleId::T2);
    let mut module = arithmetic();
    let boolean = module.types.intern(Type::Bool).unwrap();
    body_mut(&mut module).values.get_mut(ValueId::from_u32(2)).unwrap().ty = boolean;
    rejected(&module, RuleId::T2);
    let mut module = ModuleBuilder::new("void_call", Profile::Own);
    let own = module.intern_type(Type::Service(ServiceType::Own)).unwrap();
    let deps = module.intern_type(Type::Service(ServiceType::Deps)).unwrap();
    let summary = module.add_summary(own_summary(vec![])).unwrap();
    let mut callee = function("callee", vec![own, deps], vec![own, deps]);
    callee.signature.ownership_abi = OwnershipAbi::Explicit { input_packet: 0, output_packet: 0 };
    callee.summary = Some(summary);
    let callee = module.declare_function(callee).unwrap();
    let callee_scope = module.declare_scope(ScopeDecl {
        function: callee, parent: None, kind: ScopeKind::Action, loc: None,
    }).unwrap();
    let mut callee_builder = module.function_builder(callee, callee_scope).unwrap();
    let returned = callee_builder.block_params(callee_builder.entry()).unwrap().to_vec();
    callee_builder.terminate(TermKind::Ret { values: returned }).unwrap();
    let callee_body = callee_builder.finish().unwrap();
    module.define_function(callee, callee_body).unwrap();
    let caller = module.declare_function(function("caller", vec![own, deps], vec![])).unwrap();
    let scope = module.declare_scope(ScopeDecl {
        function: caller, parent: None, kind: ScopeKind::Action, loc: None,
    }).unwrap();
    let mut builder = module.function_builder(caller, scope).unwrap();
    let packet = builder.block_params(builder.entry()).unwrap()[0];
    let results = builder.emit_schema(Operation::Call(CallOp::Direct { function: callee, call: CallSite {
        args: vec![], packet: Some(packet), deps: vec![ValueId::from_u32(1)],
        descriptors: vec![], type_args: vec![], summary,
    } })).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(builder.body().values.get(results[0]).unwrap().ty, own);
    assert_eq!(builder.body().values.get(results[1]).unwrap().ty, deps);
    builder.terminate(TermKind::Ret { values: vec![] }).unwrap();
    let body = builder.finish().unwrap();
    module.define_function(caller, body).unwrap();
    verify(&module.finish().unwrap()).unwrap();
}

#[test]
fn deterministic_first_all_locations_and_limits() {
    struct Locations;
    impl TextLocations for Locations {
        fn span(&self, _: &EntityLocation) -> Option<ByteSpan> { Some(ByteSpan { start: 4, end: 8 }) }
    }
    let mut module = arithmetic();
    let boolean = module.types.intern(Type::Bool).unwrap();
    body_mut(&mut module).values.get_mut(ValueId::from_u32(0)).unwrap().ty = boolean;
    module.functions.get_mut(fid()).unwrap().signature.params[0] = boolean;
    body_mut(&mut module).insts.get_mut(InstId::from_u32(0)).unwrap().results.clear();
    let all = rejected(&module, RuleId::T1);
    for _ in 0..10 { assert_eq!(verify(&module).unwrap_err(), all); }
    let first = VerifyOptions { errors: ErrorPolicy::First, ..VerifyOptions::default() };
    let first = verify_with(&module, &first, VerifyContext::default()).unwrap_err();
    assert_eq!(first.diagnostics, all.diagnostics[..1]);
    assert!(first.suppressed > 0);
    let context = VerifyContext { text_locations: Some(&Locations), ..VerifyContext::default() };
    let located = verify_with(&module, &VerifyOptions::default(), context).unwrap_err();
    assert_eq!(located.diagnostics[0].primary.text_span, Some(ByteSpan { start: 4, end: 8 }));
    let valid = arithmetic();
    for limits in [
        VerifyLimits { steps: 0, ..VerifyLimits::default() },
        VerifyLimits { entities: 0, ..VerifyLimits::default() },
        VerifyLimits { memory_bytes: 0, ..VerifyLimits::default() },
        VerifyLimits { blocks_per_function: 0, ..VerifyLimits::default() },
        VerifyLimits { values_per_function: 0, ..VerifyLimits::default() },
        VerifyLimits { instructions_per_function: 0, ..VerifyLimits::default() },
    ] {
        let options = VerifyOptions { limits, ..VerifyOptions::default() };
        assert_eq!(verify_with(&valid, &options, VerifyContext::default()).unwrap_err().failure,
            VerifyFailure::Incomplete);
    }
    let options = VerifyOptions {
        limits: VerifyLimits { diagnostics: 0, ..VerifyLimits::default() }, ..VerifyOptions::default()
    };
    assert_eq!(verify_with(&module, &options, VerifyContext::default()).unwrap_err().failure,
        VerifyFailure::Incomplete);
    let mut count = usize::MAX;
    assert!(!super::limits::add(&mut count, 1, usize::MAX));
    assert_eq!(count, usize::MAX);
}

#[test]
fn every_opcode_has_an_explicit_type_rule() {
    #[path = "type_inventory.rs"]
    mod inventory;
    let mut actual = Operation::opcode_inventory();
    let mut rules = inventory::TYPE_RULES.to_vec();
    actual.sort_unstable();
    rules.sort_unstable();
    assert_eq!(actual, rules);
    assert_eq!(rules.len(), rules.iter().collect::<std::collections::BTreeSet<_>>().len());
    // Exhaustive matches in operands.rs also make a new enum variant a compile error.
    // Exercise every existing payload on deliberately malformed arenas.
    for op in crate::contract_tests::representatives() {
        let mut module = arithmetic();
        body_mut(&mut module).insts.get_mut(InstId::from_u32(0)).unwrap().op = op;
        let _ = verify(&module);
    }
}

#[test]
fn t3_typed_branches_unwind_and_edge_slots() {
    let mut module = ModuleBuilder::new("edges", Profile::High);
    let int = module.intern_type(Type::Int).unwrap();
    let failure = module.intern_type(Type::Service(ServiceType::Failure)).unwrap();
    let function = module.declare_function(function("division", vec![int], vec![])).unwrap();
    let scope = module.declare_scope(ScopeDecl {
        function, parent: None, kind: ScopeKind::Action, loc: None,
    }).unwrap();
    let mut builder = module.function_builder(function, scope).unwrap();
    let entry = builder.entry();
    let value = builder.block_params(entry).unwrap()[0];
    let handler = builder.create_block(scope, &[ParamSpec::new(int), ParamSpec::new(failure)]).unwrap();
    builder.set_unwind(Some(BlockUnwind { target: Target { block: handler, args: vec![value] } })).unwrap();
    builder.emit_schema(Operation::Core(CoreOp::IntDiv { left: value, right: value })).unwrap();
    let finish = builder.create_block(scope, &[ParamSpec::new(int)]).unwrap();
    builder.terminate(TermKind::Br(Target { block: finish, args: vec![value] })).unwrap();
    builder.switch_to(handler).unwrap();
    let failure_value = builder.block_params(handler).unwrap()[1];
    builder.terminate(TermKind::Raise(failure_value)).unwrap();
    builder.switch_to(finish).unwrap();
    builder.terminate(TermKind::Ret { values: vec![] }).unwrap();
    let finished = builder.finish().unwrap();
    module.define_function(function, finished).unwrap();
    let positive = module.finish().unwrap();
    verify(&positive).unwrap();
    let mut negative = positive.clone();
    body_mut(&mut negative).values.get_mut(failure_value).unwrap().ty = int;
    let report = rejected(&negative, RuleId::T3);
    assert!(report.diagnostics.iter().any(|diag| diag.rule == RuleId::T3
        && matches!(diag.primary.entity, EntityLocation::Edge { .. })));
    let mut negative = positive;
    body_mut(&mut negative).blocks.get_mut(entry).unwrap().term.kind = TermKind::Br(Target {
        block: finish, args: vec![],
    });
    rejected(&negative, RuleId::T3);
}

fn generic_object() -> (Module, DeclId, TypeId) {
    let mut module = ModuleBuilder::new("generic", Profile::High);
    let int = module.intern_type(Type::Int).unwrap();
    let decl = module.declare(Declaration {
        name: "Box".into(), loc: None,
        type_params: vec![TypeParameter { name: "T".into(), bounds: vec![] }],
        kind: DeclarationKind::Object { fields: vec![], witnesses: vec![], destroy: None, builtin: false },
    }).unwrap();
    let parameter = module.intern_type(Type::Parameter(TypeParameterRef {
        owner: GenericOwner::Declaration(decl), index: 0,
    })).unwrap();
    if let DeclarationKind::Object { fields, .. } = &mut module.raw_mut().declarations.get_mut(decl).unwrap().kind {
        fields.push(Field { name: "value".into(), ty: parameter, default: None, once: false });
    }
    let object = module.intern_type(Type::Object { decl, args: vec![int] }).unwrap();
    let function = module.declare_function(function("project", vec![object], vec![])).unwrap();
    let scope = module.declare_scope(ScopeDecl {
        function, parent: None, kind: ScopeKind::Action, loc: None,
    }).unwrap();
    let mut builder = module.function_builder(function, scope).unwrap();
    let value = builder.block_params(builder.entry()).unwrap()[0];
    let result = builder.emit_schema(Operation::Core(CoreOp::ObjGet {
        object: value, field: FieldRef { declaration: decl, index: 0 },
    })).unwrap()[0];
    builder.terminate(TermKind::Ret { values: vec![] }).unwrap();
    let mut body = builder.finish().unwrap();
    // The #563 builder resolves a Field formula to its declaration TypeId.
    // A producer that substitutes generics writes the concrete result instead.
    body.values.get_mut(result).unwrap().ty = int;
    module.define_function(function, body).unwrap();
    (module.finish().unwrap(), decl, int)
}

#[test]
fn t4_generics_substitute_fields_and_check_arity_bounds() {
    let (positive, decl, int) = generic_object();
    let before = positive.clone();
    verify(&positive).unwrap();
    assert_eq!(positive, before);
    let mut negative = positive.clone();
    negative.types.intern(Type::Object { decl, args: vec![] }).unwrap();
    rejected(&negative, RuleId::T4);
    let mut negative = positive.clone();
    let role = negative.declarations.alloc(Declaration {
        name: "Bound".into(), loc: None, type_params: vec![],
        kind: DeclarationKind::Role { methods: vec![], builtin: false },
    }).unwrap();
    let bound = negative.types.intern(Type::Role { decl: role, args: vec![] }).unwrap();
    negative.declarations.get_mut(decl).unwrap().type_params[0].bounds.push(bound);
    rejected(&negative, RuleId::T4);
    let mut negative = positive;
    let boolean = negative.types.intern(Type::Bool).unwrap();
    body_mut(&mut negative).values.get_mut(ValueId::from_u32(1)).unwrap().ty = boolean;
    rejected(&negative, RuleId::T2);
    assert_ne!(int, boolean);
}

#[test]
fn t5_layouts_witnesses_defaults_and_destroy_signatures() {
    let mut module = arithmetic();
    let int = module.types.intern(Type::Int).unwrap();
    let role = module.declarations.alloc(Declaration {
        name: "Value".into(), type_params: vec![], loc: None,
        kind: DeclarationKind::Role { methods: vec![RoleMethod {
            name: "get".into(), signature: module.functions.get(fid()).unwrap().signature.clone(),
        }], builtin: false },
    }).unwrap();
    let role_ty = module.types.intern(Type::Role { decl: role, args: vec![] }).unwrap();
    let object = module.declarations.alloc(Declaration {
        name: "Holder".into(), type_params: vec![], loc: None,
        kind: DeclarationKind::Object {
            fields: vec![Field { name: "item".into(), ty: int, default: None, once: false }],
            witnesses: vec![Witness { role: role_ty, slots: vec![fid()] }], destroy: None, builtin: false,
        },
    }).unwrap();
    let object_ty = module.types.intern(Type::Object { decl: object, args: vec![] }).unwrap();
    let mut builder = ModuleBuilder::new("witnesses", Profile::High);
    *builder.raw_mut() = module;
    let mut method = function("get", vec![object_ty, int], vec![int]);
    method.attrs.push(FunctionAttr::Method(object));
    let method = builder.declare_function(method).unwrap();
    let scope = builder.declare_scope(ScopeDecl {
        function: method, parent: None, kind: ScopeKind::Action, loc: None,
    }).unwrap();
    let mut body_builder = builder.function_builder(method, scope).unwrap();
    let value = body_builder.block_params(body_builder.entry()).unwrap()[1];
    body_builder.terminate(TermKind::Ret { values: vec![value] }).unwrap();
    let method_body = body_builder.finish().unwrap();
    builder.define_function(method, method_body).unwrap();
    let mut module = builder.finish().unwrap();
    if let DeclarationKind::Role { methods, .. } = &mut module.declarations.get_mut(role).unwrap().kind {
        methods[0].signature.params = vec![int];
    }
    if let DeclarationKind::Object { witnesses, .. } = &mut module.declarations.get_mut(object).unwrap().kind {
        witnesses[0].slots = vec![method];
    }
    verify(&module).unwrap();
    let positive = module.clone();
    if let DeclarationKind::Role { methods, .. } = &mut module.declarations.get_mut(role).unwrap().kind {
        methods[0].signature.params.clear();
    }
    rejected(&module, RuleId::T5);
    let mut module = positive.clone();
    if let DeclarationKind::Object { fields, .. } = &mut module.declarations.get_mut(object).unwrap().kind {
        fields[0].default = Some(fid());
    }
    rejected(&module, RuleId::T5);
    let mut module = positive;
    if let DeclarationKind::Object { destroy, .. } = &mut module.declarations.get_mut(object).unwrap().kind {
        *destroy = Some(fid());
    }
    rejected(&module, RuleId::T5);
}

#[test]
fn t6_error_payload_is_a_scalar_or_recursively_permitted_enum() {
    let mut module = arithmetic();
    let int = module.types.intern(Type::Int).unwrap();
    let layout = module.declarations.alloc(Declaration {
        name: "ProblemPayload".into(), type_params: vec![], loc: None,
        kind: DeclarationKind::Object {
            fields: vec![Field { name: "code".into(), ty: int, default: None, once: false }],
            witnesses: vec![], destroy: None, builtin: false,
        },
    }).unwrap();
    module.declarations.alloc(Declaration {
        name: "Problem".into(), type_params: vec![], loc: None,
        kind: DeclarationKind::Variant {
            cases: vec![VariantCase { name: "failed".into(), layout }], is_error: true,
        },
    }).unwrap();
    verify(&module).unwrap();
    let array = module.types.intern(Type::Array(int)).unwrap();
    if let DeclarationKind::Object { fields, .. } = &mut module.declarations.get_mut(layout).unwrap().kind {
        fields[0].ty = array;
    }
    rejected(&module, RuleId::T6);
}

#[test]
fn iterative_metadata_walk_preserves_occurrences_and_limits_shared_dags() {
    let mut module = arithmetic();
    let body = body_mut(&mut module);
    let leaf = body.selections.alloc(SelectionExpr::Value(ValueId::from_u32(0))).unwrap();
    let mut cursor = leaf;
    for _ in 0..4096 { cursor = body.selections.alloc(SelectionExpr::Union(vec![cursor])).unwrap(); }
    let twice = body.selections.alloc(SelectionExpr::Union(vec![cursor, leaf, cursor])).unwrap();
    let op = Operation::Ownership(OwnershipOp::Split { packet: ValueId::from_u32(1), selection: twice });
    let uses = cfg::operation_uses(&op, body).unwrap();
    assert_eq!(uses.iter().map(|use_| use_.value).collect::<Vec<_>>(), [
        ValueId::from_u32(1), ValueId::from_u32(0), ValueId::from_u32(0), ValueId::from_u32(0),
    ]);
    // Verifier visits each declaration once, rather than expanding from each root.
    let verified = verify(&module).unwrap();
    assert!(verified.stats().steps < 200_000);
    let options = VerifyOptions {
        limits: VerifyLimits { expression_nodes_per_function: 1, ..VerifyLimits::default() },
        ..VerifyOptions::default()
    };
    assert_eq!(verify_with(&module, &options, VerifyContext::default()).unwrap_err().failure,
        VerifyFailure::Incomplete);
}

#[path = "scalar_tests.rs"]
mod scalar_tests;

#[test]
fn operand_families_accept_builders_and_reject_each_changed_operand() {
    for case in 0..38 {
        let mut module = ModuleBuilder::new("families", Profile::Own);
        let int = module.intern_type(Type::Int).unwrap();
        let array = module.intern_type(Type::Array(int)).unwrap();
        let map = module.intern_type(Type::Map(int, int)).unwrap();
        let buffer = module.intern_type(Type::Builder(int)).unwrap();
        let mbuffer = module.intern_type(Type::MapBuilder(int, int)).unwrap();
        let future = module.intern_type(Type::Future(Some(int))).unwrap();
        let own = module.intern_type(Type::Service(ServiceType::Own)).unwrap();
        let deps = module.intern_type(Type::Service(ServiceType::Deps)).unwrap();
        let scope_ty = module.intern_type(Type::Service(ServiceType::Scope(ScopeId::from_u32(0)))).unwrap();
        let failure = module.intern_type(Type::Service(ServiceType::Failure)).unwrap();
        let pending = module.intern_type(Type::Service(ServiceType::Pending)).unwrap();
        let cleanup = module.intern_type(Type::Service(ServiceType::CleanupState)).unwrap();
        let key = module.intern_type(Type::Service(ServiceType::StableKey)).unwrap();
        let snapshot = module.intern_type(Type::Service(ServiceType::OwnerSnapshot)).unwrap();
        let frame = module.intern_type(Type::Frame(fid())).unwrap();
        let cell = module.intern_type(Type::LowCell(int)).unwrap();
        let v = ValueId::from_u32;
        let binding = BindingId::from_u32(0);
        let selection = SelectionId::from_u32(0);
        let scope = ScopeId::from_u32(0);
        let boundary = BoundaryId::from_u32(0);
        let (mut params, op) = match case {
            0 => (vec![array, int], Operation::Collection(CollectionOp::ArrayGet { array: v(0), index: v(1) })),
            1 => (vec![array, int, int], Operation::Collection(CollectionOp::ArraySlice {
                array: v(0), start: v(1), end: v(2), packet: None,
            })),
            2 => (vec![array, int, int], Operation::Collection(CollectionOp::ArrayReplace {
                array: v(0), index: v(1), value: v(2), packet: None,
            })),
            3 => (vec![map, int], Operation::Collection(CollectionOp::MapGet { map: v(0), key: v(1) })),
            4 => (vec![map], Operation::Collection(CollectionOp::MapKeys { map: v(0) })),
            5 => (vec![int, int], Operation::Collection(CollectionOp::ArrayNew {
                values: vec![v(0), v(1)], ty: array, packet: None,
            })),
            6 => (vec![int, int], Operation::Collection(CollectionOp::MapNew {
                entries: vec![(v(0), v(1))], ty: map, packet: None,
            })),
            7 => (vec![buffer, int], Operation::Collection(CollectionOp::BuilderPush {
                buffer: v(0), value: v(1), packet: None,
            })),
            8 => (vec![buffer, array], Operation::Collection(CollectionOp::BuilderAppend {
                buffer: v(0), array: v(1), packet: None,
            })),
            9 => (vec![mbuffer, int, int], Operation::Collection(CollectionOp::MapBuilderPut {
                buffer: v(0), key: v(1), value: v(2), packet: None,
            })),
            10 => (vec![buffer], Operation::Collection(CollectionOp::BuilderFinish { buffer: v(0), packet: None })),
            11 => (vec![mbuffer], Operation::Collection(CollectionOp::MapBuilderFinish { buffer: v(0), packet: None })),
            12 => (vec![int], Operation::Lexical(LexicalOp::Bind { binding, value: v(0) })),
            13 => (vec![int], Operation::Lexical(LexicalOp::Rebind { binding, value: v(0), target: scope })),
            14 => (vec![own], Operation::Ownership(OwnershipOp::Move { packet: v(0) })),
            15 => (vec![own, own], Operation::Ownership(OwnershipOp::Join { packets: vec![v(0), v(1)] })),
            16 => (vec![scope_ty, own], Operation::Ownership(OwnershipOp::Admit { state: v(0), packet: v(1) })),
            17 => (vec![own], Operation::Ownership(OwnershipOp::Split { packet: v(0), selection })),
            18 => (vec![scope_ty], Operation::Ownership(OwnershipOp::ScopeSplit { state: v(0), selection })),
            19 => (vec![scope_ty], Operation::Ownership(OwnershipOp::ScopeEnd { state: v(0) })),
            20 => (vec![scope_ty, failure], Operation::Ownership(OwnershipOp::ScopeUnwind {
                state: v(0), failure: v(1),
            })),
            21 => (vec![own], Operation::Ownership(OwnershipOp::End { packet: v(0), boundary })),
            22 => (vec![key, snapshot], Operation::Ownership(OwnershipOp::DynamicCheck {
                key: v(0), snapshot: v(1),
            })),
            23 => (vec![future], Operation::Async(AsyncOp::Await { future: v(0), result: Some(int) })),
            24 => (vec![future], Operation::Async(AsyncOp::FutureDone { future: v(0) })),
            25 => (vec![future], Operation::Async(AsyncOp::FutureSpawn { future: v(0) })),
            26 => (vec![frame], Operation::Async(AsyncOp::FrameLoad { frame: v(0), slot: 0, ty: int })),
            27 => (vec![frame, own, deps, scope_ty], Operation::Async(AsyncOp::FutureNew {
                frame: v(0), packet: v(1), deps: v(2), owner: scope, owner_state: v(3), result: Some(int),
            })),
            28 => (vec![pending], Operation::Exit(ExitOp::Commit {
                pending: v(0), destination: ExitDestination::Caller, result: Some(int),
            })),
            29 => (vec![pending, cleanup], Operation::Exit(ExitOp::Abort {
                pending: v(0), cleanup: v(1), phase: AbortPhase::BeforeDestroy,
            })),
            30 => (vec![cleanup], Operation::Cleanup(CleanupOp::Next { cleanup: v(0) })),
            31 => (vec![cleanup], Operation::Cleanup(CleanupOp::Finish { cleanup: v(0) })),
            32 => (vec![int], Operation::Cleanup(CleanupOp::CellNew { value: v(0), ty: int })),
            33 => (vec![cell, int], Operation::Cleanup(CleanupOp::CellStore { cell: v(0), value: v(1) })),
            34 => (vec![cell], Operation::Cleanup(CleanupOp::CellLoad { cell: v(0) })),
            35 => (vec![failure, failure], Operation::Core(CoreOp::FailSecondary { primary: v(0), cleanup: v(1) })),
            36 => (vec![scope_ty, scope_ty], Operation::Ownership(OwnershipOp::ScopeTransfer {
                states: TransferStates::Between { from: v(0), to: v(1) }, selection,
            })),
            37 => (vec![int, own, scope_ty], Operation::Ownership(OwnershipOp::Bind {
                binding: ElaboratedBinding {
                    binding, resource: ResourceValue { value: v(0), packet: v(1) },
                    state: v(2), original_scope: scope,
                },
            })),
            _ => unreachable!(),
        };
        let operand_count = params.len();
        params.push(int);
        let function = module.declare_function(function("operation", params, vec![])).unwrap();
        let scope = module.declare_scope(ScopeDecl {
            function, parent: None, kind: ScopeKind::Action, loc: None,
        }).unwrap();
        let mut builder = module.function_builder(function, scope).unwrap();
        builder.declare_binding(BindingDecl { name: "n".into(), scope, ty: int, loc: None }).unwrap();
        builder.declare_boundary(Boundary { scope, kind: BoundaryKind::Lexical }).unwrap();
        builder.declare_selection(SelectionExpr::Value(v(operand_count as u32))).unwrap();
        let contract = ContractRegistry::new(builder.module()).lookup(&op).unwrap();
        if contract.unwind.is_some() {
            let edges = builder.invoke_own_successors(op.clone(), scope, scope).unwrap();
            builder.switch_to(edges.normal).unwrap();
            builder.terminate(TermKind::Ret { values: vec![] }).unwrap();
            builder.switch_to(edges.unwind).unwrap();
            builder.terminate(TermKind::Ret { values: vec![] }).unwrap();
        } else {
            builder.emit_schema(op.clone()).unwrap();
            builder.terminate(TermKind::Ret { values: vec![] }).unwrap();
        }
        let body = builder.finish().unwrap();
        module.define_function(function, body).unwrap();
        let positive = module.finish().unwrap();
        verify(&positive).unwrap_or_else(|report| panic!("case {case} {op:?}: {report:?}"));
        for index in 0..operand_count {
            let mut negative = positive.clone();
            let wrong = negative.types.intern(Type::Bool).unwrap();
            negative.functions.get_mut(function).unwrap().signature.params[index] = wrong;
            body_mut(&mut negative).values.get_mut(v(index as u32)).unwrap().ty = wrong;
            rejected(&negative, RuleId::T1);
        }
    }
}

#[test]
fn canonical_rationals_and_empty_collection_retag() {
    let mut module = arithmetic();
    for (numerator, denominator, valid) in [
        ("0", "1", true), ("-7", "11", true), ("2", "3", true), ("2", "4", false),
        ("0", "3", false), ("1", "0", false), ("1", "-2", false),
        ("12345678901234567890", "12345678901234567891", true),
    ] {
        body_mut(&mut module).insts.get_mut(InstId::from_u32(0)).unwrap().op = Operation::Const(Literal::Rat {
            numerator: numerator.into(), denominator: denominator.into(),
        });
        let report = verify(&module).unwrap_err();
        assert_eq!(report.diagnostics.iter().any(|diag| diag.rule == RuleId::T1), !valid, "{report:?}");
    }
    let mut module = ModuleBuilder::new("empty", Profile::High);
    let int = module.intern_type(Type::Int).unwrap();
    let never = module.intern_type(Type::Never).unwrap();
    let empty = module.intern_type(Type::Array(never)).unwrap();
    let array = module.intern_type(Type::Array(int)).unwrap();
    let function = module.declare_function(function("retag", vec![empty], vec![])).unwrap();
    let scope = module.declare_scope(ScopeDecl {
        function, parent: None, kind: ScopeKind::Action, loc: None,
    }).unwrap();
    let mut builder = module.function_builder(function, scope).unwrap();
    builder.emit_schema(Operation::Core(CoreOp::Retag { value: ValueId::from_u32(0), ty: array })).unwrap();
    builder.terminate(TermKind::Ret { values: vec![] }).unwrap();
    let body = builder.finish().unwrap();
    module.define_function(function, body).unwrap();
    verify(&module.finish().unwrap()).unwrap();
}

#[test]
fn type_depth_outcome_schema_and_nested_summary_limits_are_incomplete() {
    let mut module = arithmetic();
    let mut ty = module.types.intern(Type::Int).unwrap();
    for _ in 0..129 { ty = module.types.intern(Type::Array(ty)).unwrap(); }
    assert_eq!(verify(&module).unwrap_err().failure, VerifyFailure::Incomplete);
    let mut module = arithmetic();
    let mut selection = SummarySelection::Parameter(0);
    for _ in 0..129 { selection = SummarySelection::Union(vec![selection]); }
    let mut summary = own_summary(vec![]);
    summary.parameters.push(ParameterEffect {
        convention: FreshConvention::Borrow, retention: Some(selection), deps: vec![],
        destination: TransferDestination::None,
    });
    module.summaries.alloc(summary).unwrap();
    assert_eq!(verify(&module).unwrap_err().failure, VerifyFailure::Incomplete);
    let module = arithmetic();
    let options = VerifyOptions {
        limits: VerifyLimits { outcome_slots: 0, ..VerifyLimits::default() }, ..VerifyOptions::default()
    };
    assert_eq!(verify_with(&module, &options, VerifyContext::default()).unwrap_err().failure,
        VerifyFailure::Incomplete);
}

#[test]
fn external_abi_catalog_is_trusted_separately_from_input_declarations() {
    struct Catalog(Signature);
    impl ExternalContracts for Catalog {
        fn native(&self, name: &str) -> Option<&Signature> { (name == "host.identity").then_some(&self.0) }
        fn intrinsic(&self, _: &str) -> Option<&Signature> { None }
    }
    let mut module = ModuleBuilder::new("native", Profile::Own);
    let int = module.intern_type(Type::Int).unwrap();
    let mut summary = own_summary(vec![int]);
    summary.parameters.push(ParameterEffect {
        convention: FreshConvention::Borrow, retention: None, deps: vec![], destination: TransferDestination::None,
    });
    let summary = module.add_summary(summary).unwrap();
    let mut native = function("native", vec![int], vec![int]);
    native.summary = Some(summary);
    native.attrs.push(FunctionAttr::Extern("host.identity".into()));
    let catalog = Catalog(native.signature.clone());
    let native = module.declare_function(native).unwrap();
    let caller = module.declare_function(function("caller", vec![int], vec![])).unwrap();
    let scope = module.declare_scope(ScopeDecl {
        function: caller, parent: None, kind: ScopeKind::Action, loc: None,
    }).unwrap();
    let mut builder = module.function_builder(caller, scope).unwrap();
    builder.emit_schema(Operation::Call(CallOp::Native { function: native, call: CallSite {
        args: vec![CallArgument { value: ValueId::from_u32(0), convention: OperandConvention::Borrow }],
        packet: None, deps: vec![], descriptors: vec![], type_args: vec![], summary,
    } })).unwrap();
    builder.terminate(TermKind::Ret { values: vec![] }).unwrap();
    let body = builder.finish().unwrap();
    module.define_function(caller, body).unwrap();
    let module = module.finish().unwrap();
    rejected(&module, RuleId::T1);
    let context = VerifyContext { text_locations: None, external_contracts: &catalog };
    verify_with(&module, &VerifyOptions::default(), context).unwrap();
}

#[test]
fn first_keeps_one_error_without_exhausting_the_all_diagnostics_budget() {
    let mut module = arithmetic();
    module.functions.get_mut(fid()).unwrap().attrs = vec![FunctionAttr::Hidden; 10];
    let all = verify(&module).unwrap_err();
    let options = VerifyOptions {
        errors: ErrorPolicy::First,
        limits: VerifyLimits { diagnostics: 1, ..VerifyLimits::default() },
        ..VerifyOptions::default()
    };
    let first = verify_with(&module, &options, VerifyContext::default()).unwrap_err();
    assert_eq!(first.failure, VerifyFailure::Rejected);
    assert_eq!(first.diagnostics, all.diagnostics[..1]);
    assert_eq!(first.suppressed, all.diagnostics.len() - 1);
}

#[test]
fn diagnostic_order_follows_instruction_positions_and_keeps_source_locations() {
    let mut module = arithmetic();
    let file = module.files.alloc(SourceFileDecl { path: "source.ok".into() }).unwrap();
    let source = DebugLoc { file, line: 12, column: 3 };
    let mut second = body(&module).insts.get(InstId::from_u32(0)).unwrap().clone();
    second.results = vec![];
    second.op = Operation::Lexical(LexicalOp::Temp {
        value: ValueId::from_u32(0), scope: ScopeId::from_u32(0),
    });
    second.loc = Some(source);
    let id = body_mut(&mut module).insts.alloc(second).unwrap();
    body_mut(&mut module).blocks.get_mut(BlockId::from_u32(0)).unwrap().insts.insert(0, id);
    let own = module.types.intern(Type::Service(ServiceType::Own)).unwrap();
    module.functions.get_mut(fid()).unwrap().signature.params[0] = own;
    body_mut(&mut module).values.get_mut(ValueId::from_u32(0)).unwrap().ty = own;
    let report = rejected(&module, RuleId::T1);
    assert!(matches!(report.diagnostics[0].primary.entity, EntityLocation::Operand {
        site: OperationSite::Instruction { instruction, .. }, ..
    } if instruction == id));
    assert_eq!(report.diagnostics[0].primary.source_loc, Some(source));
    let options = VerifyOptions { errors: ErrorPolicy::First, ..VerifyOptions::default() };
    let first = verify_with(&module, &options, VerifyContext::default()).unwrap_err();
    assert_eq!(first.diagnostics, report.diagnostics[..1]);
}

#[test]
fn shared_metadata_dag_is_verified_without_expanding_all_leaf_paths() {
    let mut module = arithmetic();
    let mut selection = body_mut(&mut module).selections.alloc(SelectionExpr::Value(ValueId::from_u32(0))).unwrap();
    for _ in 0..40 {
        selection = body_mut(&mut module).selections.alloc(SelectionExpr::Union(vec![selection, selection])).unwrap();
    }
    let verified = verify(&module).unwrap();
    assert!(verified.stats().steps < 10_000);
}
