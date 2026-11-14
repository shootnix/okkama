use super::*;
use std::collections::BTreeSet;

#[derive(Default)]
struct TestNatives {
    named: BTreeMap<String, Summary>,
    callable: BTreeMap<TypeId, Summary>,
    services: BTreeMap<TypeId, Summary>,
    protocols: BTreeMap<ProtocolId, Summary>,
}
impl NativeSummaries for TestNatives {
    fn lookup(&self, boundary: ExternalBoundary<'_>, _: &Substitution) -> Result<ExternalSummary, NativeSummaryError> {
        let summary = match boundary {
            ExternalBoundary::Native(name) | ExternalBoundary::Intrinsic(name) => self.named.get(name),
            ExternalBoundary::Callable(ty) => self.callable.get(&ty),
            ExternalBoundary::Service(ty) => self.services.get(&ty),
            ExternalBoundary::ServiceProtocol(protocol) => self.protocols.get(&protocol),
        };
        summary.cloned().map(|summary| ExternalSummary { summary }).ok_or_else(|| NativeSummaryError {
            message: format!("unregistered {boundary:?}"),
        })
    }
}

fn function(symbol: &str, params: Vec<TypeId>, results: Vec<TypeId>) -> Function {
    Function {
        symbol: symbol.into(),
        signature: Signature { params, results, ownership_abi: OwnershipAbi::Unelaborated },
        attrs: vec![],
        type_params: vec![],
        summary: None,
        async_creation_summary: None,
        async_body_summary: None,
        body: None,
        loc: None,
    }
}
fn scope(module: &mut ModuleBuilder, function: FuncId) -> ScopeId {
    module.declare_scope(ScopeDecl { function, parent: None, kind: ScopeKind::Action, loc: None }).unwrap()
}
fn object(module: &mut ModuleBuilder, name: &str, fields: Vec<TypeId>, destroy: Option<FuncId>) -> TypeId {
    let decl = module.declare(Declaration {
        name: name.into(),
        type_params: vec![],
        kind: DeclarationKind::Object {
            fields: fields.into_iter().enumerate().map(|(index, ty)| Field {
                name: format!("hidden{index}"), ty, default: None, once: false,
            }).collect(),
            witnesses: vec![], destroy, builtin: false,
        },
        loc: None,
    }).unwrap();
    module.intern_type(Type::Object { decl, args: vec![] }).unwrap()
}
fn empty_body(module: &mut ModuleBuilder, function: FuncId) -> ScopeId {
    let root = scope(module, function);
    let mut builder = module.function_builder(function, root).unwrap();
    builder.terminate(TermKind::Ret { values: vec![] }).unwrap();
    let body = builder.finish().unwrap();
    module.define_function(function, body).unwrap();
    root
}
fn classify_test(module: &Module, options: &OwnOptions) -> BTreeMap<TypeId, TypeClassification> {
    classify::classify(module, options, &mut Budget::new(options)).unwrap()
}
fn summary(module: &Module, id: FuncId) -> &Summary {
    module.summaries.get(module.functions.get(id).unwrap().summary.unwrap()).unwrap()
}
fn call_site() -> CallSite {
    CallSite {
        cleanup: CallCleanupMode::Ordinary,
        args: vec![], packet: None, deps: vec![], descriptors: vec![], type_args: vec![], summary: None,
    }
}
fn with_unwind(builder: &mut FunctionBuilder<'_>, root: ScopeId) -> BlockId {
    let failure = builder.intern_type(Type::Service(ServiceType::Failure)).unwrap();
    let failed = builder.create_block(root, &[ParamSpec::new(failure)]).unwrap();
    let entry = builder.entry();
    builder.switch_to(failed).unwrap();
    let value = builder.block_params(failed).unwrap()[0];
    builder.terminate(TermKind::Raise(value)).unwrap();
    builder.switch_to(entry).unwrap();
    builder.set_unwind(Some(BlockUnwind { target: Target { block: failed, args: vec![] } })).unwrap();
    failed
}
fn instantiate(builder: &mut FunctionBuilder<'_>, ty: TypeId) -> ValueId {
    let Type::Object { decl, .. } = builder.module().types.get(ty).unwrap() else {
        panic!("object expected");
    };
    builder.emit_schema(Operation::Core(CoreOp::ObjNew {
        declaration: *decl, type_args: vec![], fields: vec![], packet: None,
        construction: ConstructionStatus::Complete,
    })).unwrap()[0]
}

#[test]
fn classification_closes_hidden_fields_roles_caps_and_scalar_graphs() {
    let mut module = ModuleBuilder::new("classify", Profile::High);
    let int = module.intern_type(Type::Int).unwrap();
    let array = module.intern_type(Type::Array(int)).unwrap();
    let plain = object(&mut module, "Plain", vec![array], None);
    let destroy = module.declare_function(function("destroy", vec![], vec![])).unwrap();
    empty_body(&mut module, destroy);
    let resource = object(&mut module, "Resource", vec![], Some(destroy));
    let hidden = object(&mut module, "Hidden", vec![plain, resource], None);
    let role = module.declare(Declaration {
        name: "Role".into(), type_params: vec![],
        kind: DeclarationKind::Role { methods: vec![], builtin: false }, loc: None,
    }).unwrap();
    let role_ty = module.intern_type(Type::Role { decl: role, args: vec![] }).unwrap();
    let Type::Object { decl, .. } = module.module().types.get(hidden).unwrap() else {
        panic!();
    };
    let decl = *decl;
    let DeclarationKind::Object { witnesses, .. } = &mut module.raw_mut().declarations.get_mut(decl).unwrap().kind
        else { panic!() };
    witnesses.push(Witness { role: role_ty, slots: vec![] });
    let cap = module.declare(Declaration {
        name: "Cap".into(), type_params: vec![],
        kind: DeclarationKind::Cap { owner: hidden, fields: vec![] }, loc: None,
    }).unwrap();
    let cap = module.intern_type(Type::Cap { decl: cap, args: vec![] }).unwrap();
    let module = module.finish().unwrap();
    let classes = classify_test(&module, &OwnOptions::default());
    for ty in [int, array, plain] {
        assert_eq!(classes[&ty].class, Classification::Plain);
        assert!(classes[&ty].certificate.is_some());
    }
    for ty in [resource, hidden, role_ty, cap] {
        assert_eq!(classes[&ty].class, Classification::Managed);
        assert!(classes[&ty].reachable_resources.contains(&resource));
        assert!(classes[&ty].destroy_entries.contains(&destroy));
        assert!(classes[&ty].certificate.is_none());
    }
}

#[test]
fn residual_descriptor_never_becomes_plain_by_default() {
    let mut module = ModuleBuilder::new("residual", Profile::High);
    let cleanup = module.declare_function(function("cleanup", vec![], vec![])).unwrap();
    empty_body(&mut module, cleanup);
    let ty = module.intern_type(Type::Parameter(TypeParameterRef {
        owner: GenericOwner::Function(cleanup), index: 0,
    })).unwrap();
    let descriptor_summary = summaries::bottom(module.module(), module.module().functions.get(cleanup).unwrap());
    let id = module.add_summary(descriptor_summary).unwrap();
    let module = module.finish().unwrap();
    let mut options = OwnOptions::default();
    let classes = classify_test(&module, &options);
    assert_eq!(classes[&ty].class, Classification::Residual);
    assert!(classes[&ty].may_own());
    assert!(classes[&ty].unresolved.contains(&ty));
    options.descriptors.insert(ty, DescriptorContract {
        managed: DescriptorManaged::Runtime, retention: SummarySelection::Descriptor(0),
        cleanup, async_destroy: false, summary: id,
    });
    let classes = classify_test(&module, &options);
    assert_eq!(classes[&ty].class, Classification::Residual);
    assert!(classes[&ty].unresolved.is_empty());
    assert_eq!(classes[&ty].destroy_entries, BTreeSet::from([cleanup]));
}

#[test]
fn scalar_scope_is_empty_and_api_preserves_high() {
    let mut module = ModuleBuilder::new("scalar", Profile::High);
    let id = module.declare_function(function("scalar", vec![], vec![])).unwrap();
    let root = scope(&mut module, id);
    let mut builder = module.function_builder(id, root).unwrap();
    with_unwind(&mut builder, root);
    let start = builder.emit_schema(Operation::Const(Literal::Int("0".into()))).unwrap()[0];
    let end = builder.emit_schema(Operation::Const(Literal::Int("3".into()))).unwrap()[0];
    builder.emit_schema(Operation::Core(CoreOp::RangeNew { start, end, inclusive: false })).unwrap();
    let ty = builder.body().values.get(start).unwrap().ty;
    let binding = builder.declare_binding(BindingDecl { name: "n".into(), scope: root, ty, loc: None }).unwrap();
    builder.emit_schema(Operation::Lexical(LexicalOp::Bind { binding, value: start })).unwrap();
    builder.terminate(TermKind::Ret { values: vec![] }).unwrap();
    let body = builder.finish().unwrap();
    module.define_function(id, body).unwrap();
    let module = module.finish().unwrap();
    let body = module.functions.get(id).unwrap().body.clone();
    let output = elaborate_ownership(module, &TestNatives::default(), &OwnOptions::default()).unwrap();
    assert_eq!(output.module.profile, Profile::High);
    assert_eq!(output.module.functions.get(id).unwrap().body, body);
    assert_eq!(output.analysis.scopes[&root].effects, ScopeEffectEnvelope::default());
    assert!(output.analysis.scopes[&root].empty_scope_eligible);
    assert_eq!(summary(&output.module, id).contract.failures, vec![FailureKind::Program]);
    assert_eq!(output.stats.empty_scope_candidates, 1);
    assert_eq!(output.stats.elided_lexical_scopes, 0);
}

#[test]
fn mutual_recursion_converges_in_a_frozen_scc() {
    let mut module = ModuleBuilder::new("recursive", Profile::High);
    let a = module.declare_function(function("a", vec![], vec![])).unwrap();
    let b = module.declare_function(function("b", vec![], vec![])).unwrap();
    for (id, target) in [(a, b), (b, a)] {
        let root = scope(&mut module, id);
        let mut builder = module.function_builder(id, root).unwrap();
        with_unwind(&mut builder, root);
        builder.emit_schema(Operation::Call(CallOp::Direct { function: target, call: call_site() })).unwrap();
        if id == b {
            let error = builder.emit_schema(Operation::Const(Literal::Str("panic".into()))).unwrap()[0];
            builder.terminate(TermKind::Panic(error)).unwrap();
        } else {
            builder.terminate(TermKind::Ret { values: vec![] }).unwrap();
        }
        let body = builder.finish().unwrap();
        module.define_function(id, body).unwrap();
    }
    let module = module.finish().unwrap();
    let output = elaborate_ownership(module, &TestNatives::default(), &OwnOptions::default()).unwrap();
    assert_eq!(output.stats.universe_nodes, 2);
    assert_eq!(output.stats.cyclic_sccs, 1);
    assert_eq!(summary(&output.module, a).contract.failures, vec![FailureKind::Panic]);
    assert_eq!(summary(&output.module, b).contract.failures, vec![FailureKind::Panic]);
}

fn prepare_return(builder: &mut FunctionBuilder<'_>, root: ScopeId, value: ValueId) {
    let boundary = builder.declare_boundary(Boundary { scope: root, kind: BoundaryKind::Lexical }).unwrap();
    let selection = builder.declare_selection(SelectionExpr::Value(value)).unwrap();
    let prepared = InstId::from_u32(builder.body().insts.len() as u32);
    builder.emit_schema(Operation::Exit(ExitOp::Prepare { exit: PreparedExit {
        kind: ExitKind::Return, value: Some(value), packet: None, deps: None, selection, boundary,
    } })).unwrap();
    builder.add_high_result(HighResultExit {
        block: builder.current_block().unwrap(), edge: HighEdgeSelector::Normal(0), value,
        demand: ResultDemand::Want, prepared: Some(prepared),
    }).unwrap();
}
fn resource(module: &mut ModuleBuilder, name: &str) -> (TypeId, FuncId) {
    let destroy = module.declare_function(function(&format!("destroy_{name}"), vec![], vec![])).unwrap();
    let ty = object(module, name, vec![], Some(destroy));
    let Type::Object { decl, .. } = module.module().types.get(ty).unwrap() else {
        panic!();
    };
    let decl = *decl;
    let function = module.raw_mut().functions.get_mut(destroy).unwrap();
    function.signature.params = vec![ty];
    function.attrs.push(FunctionAttr::Destroy(decl));
    (ty, destroy)
}
fn constructor(module: &mut ModuleBuilder, ty: TypeId, name: &str) -> FuncId {
    let mut data = function(name, vec![], vec![ty]);
    let Type::Object { decl, .. } = module.module().types.get(ty).unwrap() else {
        panic!();
    };
    data.attrs.push(FunctionAttr::Ctor(*decl));
    let id = module.declare_function(data).unwrap();
    let root = scope(module, id);
    let mut builder = module.function_builder(id, root).unwrap();
    with_unwind(&mut builder, root);
    let value = instantiate(&mut builder, ty);
    prepare_return(&mut builder, root, value);
    builder.terminate(TermKind::Ret { values: vec![value] }).unwrap();
    let body = builder.finish().unwrap();
    module.define_function(id, body).unwrap();
    id
}
fn local_call(module: &mut ModuleBuilder, id: FuncId, target: FuncId) -> ScopeId {
    let root = scope(module, id);
    let mut builder = module.function_builder(id, root).unwrap();
    with_unwind(&mut builder, root);
    builder.emit_schema(Operation::Call(CallOp::Direct { function: target, call: call_site() })).unwrap();
    builder.terminate(TermKind::Ret { values: vec![] }).unwrap();
    let body = builder.finish().unwrap();
    module.define_function(id, body).unwrap();
    root
}

#[test]
fn m2_destroy_locals_and_throwing_work_reach_plain_caller_cleanup() {
    let mut module = ModuleBuilder::new("m2", Profile::High);
    let mut work_data = function("work", vec![], vec![]);
    work_data.attrs.push(FunctionAttr::Extern("work".into()));
    let work = module.declare_function(work_data).unwrap();
    let mut native = TestNatives::default();
    let mut work_summary = summaries::bottom(module.module(), module.module().functions.get(work).unwrap());
    work_summary.contract.failures.push(FailureKind::Thrown);
    native.named.insert("work".into(), work_summary);
    let (tmp, destroy_tmp) = resource(&mut module, "Tmp");
    empty_body(&mut module, destroy_tmp);
    let make_tmp = constructor(&mut module, tmp, "Tmp.new");
    let (node, destroy_node) = resource(&mut module, "Node");
    let root = scope(&mut module, destroy_node);
    let mut builder = module.function_builder(destroy_node, root).unwrap();
    with_unwind(&mut builder, root);
    builder.emit_schema(Operation::Call(CallOp::Direct { function: make_tmp, call: call_site() })).unwrap();
    builder.emit_schema(Operation::Call(CallOp::Native { function: work, call: call_site() })).unwrap();
    builder.terminate(TermKind::Ret { values: vec![] }).unwrap();
    let body = builder.finish().unwrap();
    module.define_function(destroy_node, body).unwrap();
    let make_node = constructor(&mut module, node, "Node.new");
    let f = module.declare_function(function("f", vec![], vec![])).unwrap();
    let f_scope = local_call(&mut module, f, make_node);
    // No explicit call or throw: fresh input cleanup alone must add Thrown.
    let accepted = module.declare_function(function("accepted", vec![node], vec![])).unwrap();
    let accepted_scope = empty_body(&mut module, accepted);
    let output = elaborate_ownership(module.finish().unwrap(), &native, &OwnOptions::default()).unwrap();
    assert!(summary(&output.module, f).contract.failures.contains(&FailureKind::Thrown));
    assert!(summary(&output.module, accepted).contract.failures.contains(&FailureKind::Thrown));
    assert!(!summary(&output.module, destroy_tmp).contract.failures.contains(&FailureKind::Thrown));
    assert!(output.analysis.cleanup_effects[&node].failures.contains(&FailureKind::Thrown));
    assert!(!output.analysis.cleanup_effects[&tmp].failures.contains(&FailureKind::Thrown));
    assert!(output.analysis.scopes[&f_scope].effects.failures.contains(&FailureKind::Thrown));
    assert!(output.analysis.scopes[&accepted_scope].effects.failures.contains(&FailureKind::Thrown));
    assert!(!output.analysis.scopes[&f_scope].empty_scope_eligible);
    assert!(output.stats.implicit_destroy_edges >= 4);
    assert_eq!(output.stats.universe_nodes, output.stats.functions);
}

#[test]
fn recursive_destroy_scc_includes_both_cleanup_and_constructor_edges() {
    let mut module = ModuleBuilder::new("destroy_scc", Profile::High);
    let (a, destroy_a) = resource(&mut module, "A");
    let (b, destroy_b) = resource(&mut module, "B");
    let make_a = constructor(&mut module, a, "A.new");
    let make_b = constructor(&mut module, b, "B.new");
    local_call(&mut module, destroy_a, make_b);
    let root = scope(&mut module, destroy_b);
    let mut builder = module.function_builder(destroy_b, root).unwrap();
    with_unwind(&mut builder, root);
    builder.emit_schema(Operation::Call(CallOp::Direct { function: make_a, call: call_site() })).unwrap();
    let message = builder.emit_schema(Operation::Const(Literal::Str("bad".into()))).unwrap()[0];
    builder.terminate(TermKind::Panic(message)).unwrap();
    let body = builder.finish().unwrap();
    module.define_function(destroy_b, body).unwrap();
    let module = module.finish().unwrap();
    let reparsed = text::parse(&text::print(&module).unwrap()).unwrap();
    let output = elaborate_ownership(module, &TestNatives::default(), &OwnOptions::default()).unwrap();
    let roundtrip = elaborate_ownership(reparsed, &TestNatives::default(), &OwnOptions::default()).unwrap();
    assert!(text::normalized_eq(&output.module, &roundtrip.module).unwrap());
    assert!(output.stats.cyclic_sccs >= 1);
    for id in [destroy_a, destroy_b, make_a, make_b] {
        assert!(summary(&output.module, id).contract.failures.contains(&FailureKind::Panic));
    }
    for ty in [a, b] {
        assert!(output.analysis.cleanup_effects[&ty].failures.contains(&FailureKind::Panic));
    }
}

#[test]
fn every_summary_component_has_a_canonical_join() {
    let mut module = ModuleBuilder::new("normal", Profile::High);
    let int = module.intern_type(Type::Int).unwrap();
    let data = function("f", vec![int, int], vec![int]);
    let mut a = summaries::bottom(module.module(), &data);
    let mut b = a.clone();
    let field = FieldRef { declaration: DeclId::from_u32(0), index: 2 };
    b.parameters[0] = ParameterEffect {
        convention: FreshConvention::Take,
        retention: Some(SummarySelection::Union(vec![SummarySelection::Parameter(1), SummarySelection::Parameter(1)])),
        deps: vec![1, 0, 1], destination: TransferDestination::Result,
    };
    b.results[0].fresh = vec![CreationSiteId::from_u32(3), CreationSiteId::from_u32(1), CreationSiteId::from_u32(3)];
    b.results[0].aliases = vec![SummarySelection::Path { parameter: 0, fields: vec![field; 5] }];
    b.results[0].external_deps = vec![1, 1, 0];
    b.contract.failures = vec![FailureKind::Panic, FailureKind::Thrown, FailureKind::Panic];
    b.contract.suspends = true;
    b.contract.async_destroy = true;
    let options = OwnOptions::default();
    let mut budget = Budget::new(&options);
    normal::normalize(&mut b, &mut budget).unwrap();
    normal::join(&mut a, &b, &mut budget).unwrap();
    normal::normalize(&mut a, &mut budget).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.parameters[0].deps, vec![0, 1]);
    assert_eq!(a.parameters[0].retention, Some(SummarySelection::Parameter(1)));
    assert_eq!(a.results[0].aliases, vec![SummarySelection::OwnerGraph(0)]);
    assert_eq!(a.results[0].external_deps, vec![0, 1]);
    let before = a.clone();
    normal::join(&mut a, &b, &mut budget).unwrap();
    assert_eq!(a, before);
    b.parameters[0].destination = TransferDestination::Cache;
    let error = normal::join(&mut a, &b, &mut budget).unwrap_err();
    assert!(matches!(error, OwnError::Diagnostics { diagnostics, .. }
        if diagnostics[0].kind == OwnDiagnosticKind::IncompatibleDestinationJoin));
}

#[test]
fn publication_and_protocol_joins_are_named_counted_and_never_pick_a_route() {
    let module = ModuleBuilder::new("normal", Profile::High);
    let data = function("f", vec![], vec![]);
    let mut a = summaries::bottom(module.module(), &data);
    let mut b = a.clone();
    b.contract.publication = Publication::ChannelReceipt;
    b.contract.protocol = Some(ProtocolId::from_u32(2));
    let options = OwnOptions::default();
    let mut budget = Budget::new(&options);
    let error = normal::join(&mut a, &b, &mut budget).unwrap_err();
    let OwnError::Diagnostics { diagnostics, stats } = error else {
        panic!();
    };
    assert_eq!(stats.incompatible_publication_joins, 1);
    assert_eq!(stats.incompatible_protocol_joins, 1);
    assert_eq!(diagnostics[0].kind, OwnDiagnosticKind::IncompatiblePublicationJoin);
    assert_eq!(diagnostics[1].kind, OwnDiagnosticKind::IncompatibleProtocolJoin);
    assert_eq!(a.contract.publication, Publication::Atomic);
    assert_eq!(a.contract.protocol, None);
}

#[test]
fn work_and_memory_exhaustion_return_no_module_and_are_deterministic() {
    let mut module = ModuleBuilder::new("limits", Profile::High);
    let id = module.declare_function(function("f", vec![], vec![])).unwrap();
    empty_body(&mut module, id);
    let module = module.finish().unwrap();
    for (work_limit, memory_limit) in [(0, usize::MAX), (usize::MAX, 0)] {
        let options = OwnOptions { work_limit, memory_limit, ..OwnOptions::default() };
        let run = || elaborate_ownership(module.clone(), &TestNatives::default(), &options).unwrap_err();
        let a = run();
        let b = run();
        let (OwnError::Incomplete { stats: a }, OwnError::Incomplete { stats: b }) = (a, b) else {
            panic!();
        };
        assert_eq!(a, b);
    }
}

#[test]
fn inferred_summaries_conform_to_existing_own_bodies() {
    let sources = [
        include_str!("../../tests/fixtures/verify/valid/B5-declared-summary.oir"),
        include_str!("../../tests/fixtures/verify/valid/B5-proved-empty-entry.oir"),
        include_str!("../../tests/fixtures/verify/valid/D18-pure-scope-end.oir"),
        include_str!("../../tests/fixtures/verify/valid/B5-inductive-SCC-summaries.oir"),
    ];
    for source in sources {
        let module = text::parse(source).unwrap();
        verify_own(&module).unwrap();
        let output = elaborate_ownership(module, &TestNatives::default(), &OwnOptions::default())
            .unwrap_or_else(|error| panic!("{source}\n{error:?}"));
        verify_own(&output.module).unwrap();
    }
}

#[test]
fn nested_generic_terms_keep_outer_substitutions_when_parameters_are_shadowed() {
    let mut module = ModuleBuilder::new("generic", Profile::High);
    let int = module.intern_type(Type::Int).unwrap();
    let (resource, _) = resource(&mut module, "R");
    let decl = module.declare(Declaration {
        name: "Box".into(), type_params: vec![TypeParameter { name: "T".into(), bounds: vec![] }],
        kind: DeclarationKind::Object { fields: vec![], witnesses: vec![], destroy: None, builtin: false }, loc: None,
    }).unwrap();
    let parameter = module.intern_type(Type::Parameter(TypeParameterRef {
        owner: GenericOwner::Declaration(decl), index: 0,
    })).unwrap();
    let array = module.intern_type(Type::Array(parameter)).unwrap();
    let inner = module.intern_type(Type::Object { decl, args: vec![array] }).unwrap();
    let DeclarationKind::Object { fields, .. } = &mut module.raw_mut().declarations.get_mut(decl).unwrap().kind
        else { panic!() };
    fields.push(Field { name: "value".into(), ty: parameter, default: None, once: false });
    let outer = module.declare(Declaration {
        name: "Outer".into(), type_params: vec![],
        kind: DeclarationKind::Object {
            fields: vec![Field { name: "nested".into(), ty: inner, default: None, once: false }],
            witnesses: vec![], destroy: None, builtin: false,
        }, loc: None,
    }).unwrap();
    let _ = outer;
    let resource_box = module.intern_type(Type::Object { decl, args: vec![resource] }).unwrap();
    let plain_box = module.intern_type(Type::Object { decl, args: vec![int] }).unwrap();
    let array_resource = module.intern_type(Type::Array(resource_box)).unwrap();
    let nested_resource = module.intern_type(Type::Object { decl, args: vec![array_resource] }).unwrap();
    let module = module.finish().unwrap();
    let classes = classify_test(&module, &OwnOptions::default());
    assert_eq!(classes[&plain_box].class, Classification::Plain);
    assert_eq!(classes[&resource_box].class, Classification::Managed);
    assert_eq!(classes[&nested_resource].class, Classification::Managed);
    assert!(classes[&nested_resource].reachable_resources.contains(&resource));
}

#[test]
fn role_publication_conflicts_name_each_call_and_both_contracts() {
    let mut module = ModuleBuilder::new("join", Profile::High);
    let role = module.declare(Declaration {
        name: "Run".into(), type_params: vec![], loc: None,
        kind: DeclarationKind::Role { builtin: false, methods: vec![RoleMethod {
            name: "run".into(),
            signature: Signature { params: vec![], results: vec![], ownership_abi: OwnershipAbi::Unelaborated },
        }] },
    }).unwrap();
    let role_ty = module.intern_type(Type::Role { decl: role, args: vec![] }).unwrap();
    let mut natives = TestNatives::default();
    for (name, publication) in [("A", Publication::ChannelReceipt), ("B", Publication::TaskState)] {
        let ty = object(&mut module, name, vec![], None);
        let protocol = module.add_protocol(ProtocolDecl {
            kind: ProtocolKind::Callable, holders: vec![], rules: vec![], roots: vec![],
        }).unwrap();
        let Type::Object { decl, .. } = module.module().types.get(ty).unwrap() else {
            panic!();
        };
        let decl = *decl;
        let mut data = function(&format!("{name}.run"), vec![ty], vec![]);
        data.attrs.push(FunctionAttr::Method(decl));
        data.attrs.push(FunctionAttr::Intrinsic { name: name.into(), objects: vec![], helpers: vec![] });
        let mut summary = summaries::bottom(module.module(), &data);
        summary.contract.publication = publication;
        summary.contract.protocol = Some(protocol);
        natives.named.insert(name.into(), summary);
        let method = module.declare_function(data).unwrap();
        let Type::Object { decl, .. } = module.module().types.get(ty).unwrap() else {
            panic!();
        };
        let decl = *decl;
        let DeclarationKind::Object { witnesses, .. } = &mut module.raw_mut().declarations.get_mut(decl).unwrap().kind
            else { panic!() };
        witnesses.push(Witness { role: role_ty, slots: vec![method] });
    }
    let f = module.declare_function(function("dispatch", vec![role_ty], vec![])).unwrap();
    let root = scope(&mut module, f);
    let mut builder = module.function_builder(f, root).unwrap();
    with_unwind(&mut builder, root);
    let receiver = builder.block_params(builder.entry()).unwrap()[0];
    for _ in 0..2 {
        builder.emit_schema(Operation::Call(CallOp::Role {
            receiver: CallArgument { value: receiver, convention: OperandConvention::Borrow },
            role, slot: 0, call: call_site(),
        })).unwrap();
    }
    builder.terminate(TermKind::Ret { values: vec![] }).unwrap();
    let body = builder.finish().unwrap();
    module.define_function(f, body).unwrap();
    let error = elaborate_ownership(module.finish().unwrap(), &natives, &OwnOptions::default()).unwrap_err();
    let OwnError::Diagnostics { diagnostics, stats } = error else {
        panic!("{error:?}");
    };
    assert_eq!(stats.adapter_equations, 1);
    assert_eq!(stats.incompatible_publication_joins, 2);
    assert_eq!(stats.incompatible_protocol_joins, 2);
    assert_eq!(diagnostics.len(), 4);
    for diagnostic in diagnostics {
        assert_eq!(diagnostic.function, Some(f));
        assert!(diagnostic.site.is_some());
        assert!(diagnostic.message.contains("ChannelReceipt"));
        assert!(diagnostic.message.contains("TaskState"));
        assert!(diagnostic.message.contains("dispatch"));
    }
}

#[test]
fn assigned_service_effects_and_defers_prevent_empty_scope_elision() {
    let source = include_str!("../../tests/fixtures/verify/valid/D16-descriptor-and-schedule.oir");
    let module = text::parse(source).unwrap();
    let protocol = ProtocolId::from_u32(0);
    let mut natives = TestNatives::default();
    let mut service = summaries::bottom(&module, module.functions.get(FuncId::from_u32(0)).unwrap());
    service.contract.failures = vec![FailureKind::Thrown, FailureKind::Cancelled];
    natives.protocols.insert(protocol, service);
    let output = elaborate_ownership(module, &natives, &OwnOptions::default()).unwrap();
    let facts = &output.analysis.scopes[&ScopeId::from_u32(0)];
    assert!(!facts.empty_scope_eligible);
    assert!(facts.blockers.contains(&ElisionBlocker::ServiceDuty));
    assert_eq!(facts.effects.failures, vec![FailureKind::Thrown]);
    assert!(!facts.effects.cancel);
    let source = include_str!("../../tests/fixtures/verify/valid/H3-dominating-defer-prefix.oir");
    let module = text::parse(source).unwrap();
    let output = elaborate_ownership(module, &TestNatives::default(), &OwnOptions::default()).unwrap();
    assert!(output.analysis.scopes.values().any(|facts| {
        !facts.empty_scope_eligible && facts.blockers.contains(&ElisionBlocker::Defer)
    }));
}

#[test]
fn d18_existing_resource_close_is_annotated_with_final_destroy_effects() {
    let source = [r#"oir 2 own
module narrow_close
object R {
    destroy @destroy
}
scope Sd action in @destroy
scope S action in @f
"#,
        "summary Md call-boundary parameters [parameter_effect(convention=borrow, destination=none)] ",
        "contract call_contract(failures=[panic], publication=atomic)\n",
        "summary Mf call-boundary parameters [parameter_effect(convention=borrow, destination=none)] ",
        "contract call_contract(failures=[thrown, program, panic, native], publication=atomic)\n",
        r#"func @destroy(%self: R facts(managed(deps=dd))) -> void [destroy R] summary(Md) {
    dependency dd: dependency = retention(value=%self, selection=selection(%self))
^entry in Sd:
    panic "bad"
}
func @f(%r: R facts(managed(deps=dr))) -> void summary(Mf) {
    dependency dr: dependency = retention(value=%r, selection=selection(%r))
^entry in S:
    %q: scope<S> = scope.begin S
"#,
        "    invoke.own scope.end %q, effects ",
        "scope_effects(failures=[thrown, program, panic, native], suspends=false, cancel=false)\n",
        r#"        normal ^done($closed)
        unwind ^failed($closed, $failure)
^done(%closed: closed) in S:
    ret
^failed(%closed2: closed, %failure: failure) in S:
    raise %failure
}
"#].concat();
    let module = text::parse(&source).unwrap();
    let output = elaborate_ownership(module, &TestNatives::default(), &OwnOptions::default()).unwrap();
    verify_own(&output.module).unwrap();
    let f = FuncId::from_u32(1);
    assert_eq!(summary(&output.module, f).contract.failures, vec![FailureKind::Panic]);
    let body = output.module.functions.get(f).unwrap().body.as_ref().unwrap();
    let TermKind::InvokeOwn { op, .. } = &body.blocks.get(body.entry).unwrap().term.kind else {
        panic!();
    };
    let Operation::Ownership(OwnershipOp::ScopeEnd { effects, .. }) = op.as_ref() else {
        panic!();
    };
    assert_eq!(effects.failures, vec![FailureKind::Panic]);
    assert!(!effects.suspends);
    assert!(!effects.cancel);
}

#[test]
fn known_closure_equations_are_memoized_before_solving() {
    let mut module = ModuleBuilder::new("closure", Profile::High);
    let target = module.declare_function(function("target", vec![], vec![])).unwrap();
    empty_body(&mut module, target);
    let ty = module.intern_type(Type::Fn(FnType { params: vec![], result: None, is_async: false })).unwrap();
    let f = module.declare_function(function("f", vec![], vec![])).unwrap();
    let root = scope(&mut module, f);
    let mut builder = module.function_builder(f, root).unwrap();
    with_unwind(&mut builder, root);
    let closure = builder.emit_schema(Operation::Core(CoreOp::FnClosure {
        function: target, captures: vec![], ty, packet: None,
    })).unwrap()[0];
    for _ in 0..2 {
        builder.emit_schema(Operation::Call(CallOp::Closure {
            callee: CallArgument { value: closure, convention: OperandConvention::Borrow }, call: call_site(),
        })).unwrap();
    }
    builder.terminate(TermKind::Ret { values: vec![] }).unwrap();
    let body = builder.finish().unwrap();
    module.define_function(f, body).unwrap();
    let module = module.finish().unwrap();
    let output = elaborate_ownership(module, &TestNatives::default(), &OwnOptions::default()).unwrap();
    assert_eq!(output.stats.adapter_equations, 1);
    assert_eq!(output.stats.universe_nodes, 3);
}

#[test]
fn borrowing_an_external_async_holder_does_not_invent_owned_cleanup() {
    let mut module = ModuleBuilder::new("borrow_async", Profile::High);
    let future = module.intern_type(Type::Future(None)).unwrap();
    let f = module.declare_function(function("destroy", vec![], vec![])).unwrap();
    let node = object(&mut module, "Node", vec![future], Some(f));
    let Type::Object { decl, .. } = module.module().types.get(node).unwrap() else {
        panic!();
    };
    let decl = *decl;
    let data = module.raw_mut().functions.get_mut(f).unwrap();
    data.signature.params.push(node);
    data.attrs.push(FunctionAttr::Destroy(decl));
    let root = scope(&mut module, f);
    let mut builder = module.function_builder(f, root).unwrap();
    let self_value = builder.block_params(builder.entry()).unwrap()[0];
    let value = builder.emit_schema(Operation::Core(CoreOp::ObjGet {
        object: self_value, field: FieldRef { declaration: decl, index: 0 },
    })).unwrap()[0];
    let binding = builder.declare_binding(BindingDecl {
        name: "view".into(), scope: root, ty: future, loc: None,
    }).unwrap();
    builder.emit_schema(Operation::Lexical(LexicalOp::Bind { binding, value })).unwrap();
    builder.terminate(TermKind::Ret { values: vec![] }).unwrap();
    let body = builder.finish().unwrap();
    module.define_function(f, body).unwrap();
    let module = module.finish().unwrap();
    let output = elaborate_ownership(module, &TestNatives::default(), &OwnOptions::default()).unwrap();
    assert!(output.analysis.scopes[&root].potential_owned_types.is_empty());
    assert!(output.analysis.scopes[&root].empty_scope_eligible);
    assert!(!summary(&output.module, f).contract.suspends);
    assert!(!output.analysis.cleanup_effects.contains_key(&future));
}

#[test]
fn residual_runtime_summary_projects_cleanup_and_unbound_dictionary_is_an_error() {
    let mut module = ModuleBuilder::new("descriptor", Profile::High);
    let cleanup = module.declare_function(function("cleanup", vec![], vec![])).unwrap();
    empty_body(&mut module, cleanup);
    let f = module.declare_function(function("generic", vec![], vec![])).unwrap();
    let parameter = module.intern_type(Type::Parameter(TypeParameterRef {
        owner: GenericOwner::Function(f), index: 0,
    })).unwrap();
    let data = module.raw_mut().functions.get_mut(f).unwrap();
    data.type_params.push(TypeParameter { name: "T".into(), bounds: vec![] });
    data.signature.params.push(parameter);
    let root = empty_body(&mut module, f);
    let mut runtime = summaries::bottom(module.module(), module.module().functions.get(cleanup).unwrap());
    runtime.contract.failures.push(FailureKind::Thrown);
    let runtime = module.add_summary(runtime).unwrap();
    let module = module.finish().unwrap();
    let error = elaborate_ownership(module.clone(), &TestNatives::default(), &OwnOptions::default()).unwrap_err();
    assert!(matches!(error, OwnError::Diagnostics { diagnostics, .. }
        if diagnostics[0].kind == OwnDiagnosticKind::MissingDescriptor));
    let mut options = OwnOptions::default();
    options.descriptors.insert(parameter, DescriptorContract {
        managed: DescriptorManaged::Runtime, retention: SummarySelection::Descriptor(0),
        cleanup, async_destroy: false, summary: runtime,
    });
    let output = elaborate_ownership(module, &TestNatives::default(), &options).unwrap();
    assert_eq!(summary(&output.module, f).contract.failures, vec![FailureKind::Thrown]);
    assert_eq!(output.analysis.scopes[&root].effects.failures, vec![FailureKind::Thrown]);
    assert_eq!(output.analysis.types[&parameter].class, Classification::Residual);
    assert!(!output.analysis.scopes[&root].empty_scope_eligible);
}

#[test]
fn shared_type_dag_has_a_budgeted_deterministic_classification() {
    let mut module = ModuleBuilder::new("shared", Profile::High);
    let mut ty = module.intern_type(Type::Int).unwrap();
    for index in 0..80 {
        ty = object(&mut module, &format!("Layer{index}"), vec![ty, ty], None);
    }
    let module = module.finish().unwrap();
    let options = OwnOptions::default();
    let mut first = Budget::new(&options);
    let a = classify::classify(&module, &options, &mut first).unwrap();
    let mut second = Budget::new(&options);
    let b = classify::classify(&module, &options, &mut second).unwrap();
    assert_eq!(a, b);
    assert_eq!(first.stats, second.stats);
    assert_eq!(a[&ty].class, Classification::Plain);
    assert!(first.stats.charged_work < 100_000);
}

#[test]
fn unknown_intrinsic_is_not_given_a_guessed_summary() {
    let mut module = ModuleBuilder::new("unknown", Profile::High);
    let mut data = function("unknown", vec![], vec![]);
    data.attrs.push(FunctionAttr::Intrinsic { name: "missing".into(), objects: vec![], helpers: vec![] });
    module.declare_function(data).unwrap();
    let error = elaborate_ownership(module.finish().unwrap(), &TestNatives::default(), &OwnOptions::default())
        .unwrap_err();
    let OwnError::Diagnostics { diagnostics, .. } = error else {
        panic!();
    };
    assert_eq!(diagnostics[0].kind, OwnDiagnosticKind::MissingBoundary);
    assert!(diagnostics[0].message.contains("Intrinsic"));
    assert!(diagnostics[0].message.contains("missing"));
}

#[test]
fn exact_work_and_memory_bounds_are_enforced_at_the_end_of_inference() {
    let mut module = ModuleBuilder::new("budget", Profile::High);
    let f = module.declare_function(function("f", vec![], vec![])).unwrap();
    empty_body(&mut module, f);
    let module = module.finish().unwrap();
    let output = elaborate_ownership(module.clone(), &TestNatives::default(), &OwnOptions::default()).unwrap();
    for options in [
        OwnOptions { work_limit: output.stats.charged_work - 1, ..OwnOptions::default() },
        OwnOptions { memory_limit: output.stats.peak_auxiliary_bytes - 1, ..OwnOptions::default() },
    ] {
        let error = elaborate_ownership(module.clone(), &TestNatives::default(), &options).unwrap_err();
        assert!(matches!(error, OwnError::Incomplete { .. }));
    }
}

#[test]
fn native_lookup_receives_the_actual_substitution_before_the_universe_is_frozen() {
    use std::cell::RefCell;
    struct RecordingNative {
        seen: RefCell<Vec<Substitution>>,
        summary: Summary,
    }
    impl NativeSummaries for RecordingNative {
        fn lookup(&self, boundary: ExternalBoundary<'_>, substitution: &Substitution)
            -> Result<ExternalSummary, NativeSummaryError> {
            assert_eq!(boundary, ExternalBoundary::Native("generic"));
            self.seen.borrow_mut().push(substitution.clone());
            Ok(ExternalSummary { summary: self.summary.clone() })
        }
    }
    let mut module = ModuleBuilder::new("substitution", Profile::High);
    let int = module.intern_type(Type::Int).unwrap();
    let mut data = function("generic", vec![], vec![]);
    data.attrs.push(FunctionAttr::Extern("generic".into()));
    data.type_params.push(TypeParameter { name: "T".into(), bounds: vec![] });
    let native = module.declare_function(data).unwrap();
    let catalog = RecordingNative {
        seen: RefCell::new(vec![]),
        summary: summaries::bottom(module.module(), module.module().functions.get(native).unwrap()),
    };
    let f = module.declare_function(function("f", vec![], vec![])).unwrap();
    let root = scope(&mut module, f);
    let mut builder = module.function_builder(f, root).unwrap();
    with_unwind(&mut builder, root);
    let mut call = call_site();
    call.type_args.push(int);
    builder.emit_schema(Operation::Call(CallOp::Native { function: native, call })).unwrap();
    builder.terminate(TermKind::Ret { values: vec![] }).unwrap();
    let body = builder.finish().unwrap();
    module.define_function(f, body).unwrap();
    let output = elaborate_ownership(module.finish().unwrap(), &catalog, &OwnOptions::default()).unwrap();
    assert_eq!(catalog.seen.borrow().as_slice(), &[Substitution { types: vec![int], descriptors: vec![] }]);
    assert_eq!(output.stats.universe_nodes, 2);
}
