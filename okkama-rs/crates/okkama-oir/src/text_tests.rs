use crate::*;
use crate::text::{self, DiagnosticKind as K, Limits};
use std::collections::HashSet;

fn function(params: Vec<TypeId>) -> Function {
    Function { symbol: "codec::f#2".into(), signature: Signature { params, results: vec![], ownership_abi: OwnershipAbi::Unelaborated },
        attrs: vec![], type_params: vec![], summary: None, async_creation_summary: None, async_body_summary: None, body: None, loc: None }
}
fn setup(profile: Profile, param: Option<Type>) -> (ModuleBuilder, FuncId, ScopeId) {
    let mut b = ModuleBuilder::new("codec", profile);
    *b.raw_mut() = crate::contract_tests::module(); b.raw_mut().profile = profile;
    let int = TypeId::from_u32(0);
    let params = param.map(|ty| vec![b.intern_type(ty).unwrap()]).unwrap_or_default();
    let f = b.declare_function(function(params)).unwrap();
    let s = b.declare_scope(ScopeDecl { function: f, parent: None, kind: ScopeKind::Action, loc: None }).unwrap();
    b.declare(Declaration { name: "Object".into(), type_params: vec![], loc: None,
        kind: DeclarationKind::Object { fields: vec![Field { name: "x".into(), ty: int, default: None, once: false }], witnesses: vec![], destroy: None, builtin: false } }).unwrap();
    (b, f, s)
}
fn roundtrip(module: &Module) -> String {
    let printed = text::print(module).unwrap();
    let parsed = text::parse(&printed).unwrap_or_else(|e| panic!("{e}\n{printed}"));
    assert!(text::normalized_eq(module, &parsed).unwrap());
    assert_eq!(text::print(&parsed).unwrap(), printed);
    printed
}
fn build_op(op: Operation, profile: Profile) -> Module { build_op_config(op, profile, |_| {}) }
fn build_op_config(op: Operation, profile: Profile, configure: impl FnOnce(&mut ModuleBuilder)) -> Module {
    let name = op.opcode();
    let int = TypeId::from_u32(0);
    let param = if name.starts_with("array.") { Type::Array(int) }
        else if name.starts_with("map.") { Type::Map(int,int) }
        else if name.starts_with("mbuilder.") { Type::MapBuilder(int,int) }
        else if name.starts_with("builder.") { Type::Builder(int) }
        else if name.starts_with("cell.") { Type::LowCell(int) }
        else if name.starts_with("await") { Type::Future(Some(int)) }
        else { Type::Int };
    let (mut m, f, s) = setup(profile, Some(param));
    // Inline nominal spelling obtains the category from the declaration.
    // Keep enum inventory cases in an enum context, including outlying case indices.
    let enum_case = match &op {
        Operation::Const(Literal::Enum { case, .. }) | Operation::Core(CoreOp::EnumCase { case, .. }) => Some(*case),
        _ => None,
    };
    if let Some(case) = enum_case {
        let decl = m.raw_mut().declarations.get_mut(DeclId::from_u32(0)).unwrap();
        decl.name = "Status".into();
        decl.kind = DeclarationKind::Enum {
            cases: (0..=case).map(|index| format!("case{index}")).collect(), is_error: false,
        };
    }
    configure(&mut m);
    let mut b = m.function_builder(f, s).unwrap();
    let value = b.block_params(b.entry()).unwrap()[0];
    b.declare_binding(BindingDecl { name: "x".into(), scope: s, ty: int, loc: None }).unwrap();
    b.declare_boundary(Boundary { scope: s, kind: BoundaryKind::Call }).unwrap();
    b.declare_key(KeyExpr::Runtime(value)).unwrap();
    b.intern_dependency(DepExpr::Empty).unwrap();
    b.declare_selection(SelectionExpr::Value(value)).unwrap();
    // Creation evidence can refer to this instruction, even in a negative fixture.
    b.emit_schema(Operation::Const(Literal::Int("0".into()))).unwrap();
    let contract = ContractRegistry::new(b.module()).lookup(&op).unwrap();
    if contract.unwind.is_some() {
        let edges = b.invoke_own_successors(op, s, s).unwrap_or_else(|e| panic!("{name}: {e}"));
        b.switch_to(edges.normal).unwrap(); b.terminate(TermKind::Ret { values: vec![] }).unwrap();
        b.switch_to(edges.unwind).unwrap(); b.terminate(TermKind::Ret { values: vec![] }).unwrap();
    } else {
        b.emit_schema(op).unwrap_or_else(|e| panic!("{name}: {e}"));
        b.terminate(TermKind::Ret { values: vec![] }).unwrap();
    }
    let mut body = b.finish().unwrap();
    body.cleanup_orders.alloc(CleanupOrder { creations: vec![] }).unwrap();
    body.cleanup_roots.alloc(CleanupRoots { values: vec![], kinds: vec![], through_suspend: false }).unwrap();
    body.cleanup_plans.alloc(CleanupPlan { effects: None, entries: vec![], merge: true,
        order: CleanupOrderId::from_u32(0), roots: CleanupRootsId::from_u32(0) }).unwrap();
    m.define_function(f, body).unwrap();
    m.finish().unwrap()
}

#[test]
fn every_opcode_builder_roundtrips_and_every_invoke_slot_is_preserved() {
    let mut seen = HashSet::new();
    for op in crate::contract_tests::representatives() {
        seen.insert(op.opcode());
        let module = build_op(op, Profile::Own);
        roundtrip(&module);
    }
    assert_eq!(seen, Operation::opcode_inventory().into_iter().collect(), "opcode missing a builder round-trip case");
    assert_eq!(seen.len(), 272);
}

#[test]
fn literals_profiles_variadics_reasons_and_explicit_fields() {
    for profile in [Profile::High, Profile::Own, Profile::Coro, Profile::Low] {
        for literal in [Literal::Int("-123456789012345678901234567890".into()), Literal::Bool(false), Literal::Byte(255),
            Literal::Float(0), Literal::Float(1), Literal::Float(0x8000_0000_0000_0000), Literal::Float(f64::MAX.to_bits()),
            Literal::Rat { numerator: "-7".into(), denominator: "11".into() }, Literal::Char('🦀'),
            Literal::Str("UTF-8 🦀 // # \" \\ \n \r \t \0".into()), Literal::Enum { decl: DeclId::from_u32(0), case: 12 }] {
            roundtrip(&build_op(Operation::Const(literal), profile));
        }
        for n in [0,1,3] {
            roundtrip(&build_op(Operation::Ownership(OwnershipOp::Join { packets: vec![ValueId::from_u32(0); n] }), profile));
            roundtrip(&build_op(Operation::Collection(CollectionOp::ArrayNew { values: vec![ValueId::from_u32(0); n], ty: TypeId::from_u32(0), packet: None }), profile));
        }
    }
    for reason in [DynamicReason::Atomic, DynamicReason::Cell, DynamicReason::Channel, DynamicReason::Group, DynamicReason::Any, DynamicReason::Closure, DynamicReason::Merge] {
        roundtrip(&build_op(Operation::Ownership(OwnershipOp::Dynamic { state: ValueId::from_u32(0), packet: ValueId::from_u32(0), target: ScopeId::from_u32(0), reason, protocol: ProtocolId::from_u32(0) }), Profile::Own));
    }
    for boundary in [TakeBoundary::Existing { owner: ScopeId::from_u32(0) }, TakeBoundary::Temporary { awaiting: ScopeId::from_u32(0) }] {
        for result in [None, Some(TypeId::from_u32(0))] {
            roundtrip(&build_op(Operation::Async(AsyncOp::AwaitTake { future: ValueId::from_u32(0), result, unique: ValueId::from_u32(0), boundary }), Profile::Coro));
        }
    }
}

// Hand-checked goldens: deliberately literal strings, never rewritten by tests.
const EMPTY: &str = "oir 2 high\nmodule empty\n";
const SMALL: &str = r#"oir 2 high
module small v1.0.0
requires core
file 0 "small.ok"
entry @main#1
scope S0 action in @main#1

func @main#1() -> int [entry] !0:1:1 {
    ^b0 in S0:
    %0: int facts(plain(certificate=scalar)) = const 42 !0:2:5
    ret %0 !0:3:1
}
"#;
#[test]
fn hand_checked_goldens_and_crlf_alpha_renaming() {
    let empty = ModuleBuilder::new("empty", Profile::High).finish().unwrap();
    assert_eq!(text::print(&empty).unwrap(), EMPTY);
    assert_eq!(text::parse(EMPTY).unwrap(), empty);
    let mut m = ModuleBuilder::new("small", Profile::High);
    m.raw_mut().version = Some("1.0.0".into()); m.raw_mut().requires.push(Feature::Core);
    let int = m.intern_type(Type::Int).unwrap(); let file = m.add_file(SourceFileDecl { path: "small.ok".into() }).unwrap();
    let mut decl = function(vec![]); decl.symbol = "main#1".into(); decl.signature.results = vec![int]; decl.attrs.push(FunctionAttr::Entry); decl.loc = Some(DebugLoc { file, line: 1, column: 1 });
    let f = m.declare_function(decl).unwrap(); m.raw_mut().entry = Some(f);
    let s = m.declare_scope(ScopeDecl { function: f, parent: None, kind: ScopeKind::Action, loc: None }).unwrap();
    let mut b = m.function_builder(f, s).unwrap(); b.set_location(Some(DebugLoc { file, line: 2, column: 5 }));
    let value = b.emit(Operation::Const(Literal::Int("42".into())), &[ResultSpec { ty: int, facts: ValueFacts::Plain { certificate: PlainCertificate::Scalar } }]).unwrap()[0];
    b.set_location(Some(DebugLoc { file, line: 3, column: 1 })); b.terminate(TermKind::Ret { values: vec![value] }).unwrap();
    let body = b.finish().unwrap(); m.define_function(f, body).unwrap(); let module = m.finish().unwrap();
    assert_eq!(roundtrip(&module), SMALL);
    let renamed = SMALL.replace("%0", "%answer").replace("^b0", "^entry").replace("i0", "instruction").replace("\n", "\r\n");
    assert!(text::normalized_eq(&module, &text::parse(&renamed).unwrap()).unwrap());
    assert!(text::normalized_eq(&module, &text::parse(&format!("{SMALL}// tail # is text\n")).unwrap()).unwrap());
}

#[test]
fn versions_limits_bad_names_and_malformed_input_have_spans() {
    for (input, kind) in [("oir 1 high\n\"bad", K::UnsupportedVersion), ("oir 3 high\n{", K::UnsupportedVersion),
        ("oir 2\n{", K::MissingProfile), ("oir 2 wat\n{", K::UnknownProfile), ("wat 2 high", K::Header)] {
        assert_eq!(text::parse(input).unwrap_err().kind, kind);
    }
    for (input, kind) in [(SMALL.replace("ret %0", "ret %missing"), K::UnknownName),
        (SMALL.replace("scope S0 action in @main#1", "scope S0 action in @main#1\nscope S0 block in @main#1"), K::DuplicateName),
        (SMALL.replace("const 42", "const -"), K::Literal), (SMALL.replace("const 42", "const inf"), K::Literal),
        (SMALL.replace("small.ok", "bad\\q"), K::Lexical)] {
        let e = text::parse(&input).unwrap_err(); assert_eq!(e.kind, kind, "{e}"); assert!(e.span.start <= e.span.end && e.span.end <= input.len());
    }
    for limits in [Limits { max_text_bytes: 10, ..Limits::default() }, Limits { max_entities: 0, ..Limits::default() },
        Limits { max_literal_bytes: 3, ..Limits::default() }, Limits { max_nesting: 0, ..Limits::default() }, Limits { max_type_depth: 0, ..Limits::default() }] {
        assert_eq!(text::parse_with_limits(SMALL, limits).unwrap_err().kind, K::Limit);
    }
    // Every UTF-8-safe truncation must return a result without panicking.
    for end in 0..SMALL.len() { assert!(std::panic::catch_unwind(|| text::parse(&SMALL[..end])).is_ok()); }
    for broken in ["oir 2 high\n\"", "oir 2 high\n}", "oir 2 high\n\u{feff}", "oir 2 high\n\r", "oir 2 high\n\"\\u{ffffffff}\""] {
        assert!(std::panic::catch_unwind(|| text::parse(broken)).unwrap().is_err());
    }
}

#[test]
fn damaged_ir_is_only_diagnostic_and_semantic_errors_reach_verifier() {
    let mut module = text::parse(SMALL).unwrap();
    let f = FuncId::from_u32(0); let body = module.functions.get_mut(f).unwrap().body.as_mut().unwrap();
    // Duplicate consumption and a non-dominating, wrong-typed operand are codec data.
    body.insts.get_mut(InstId::from_u32(0)).unwrap().op = Operation::Ownership(OwnershipOp::Join { packets: vec![ValueId::from_u32(0); 2] });
    roundtrip(&module);
    module.functions.get_mut(f).unwrap().body.as_mut().unwrap().insts.get_mut(InstId::from_u32(0)).unwrap().op = Operation::Ownership(OwnershipOp::Move { packet: ValueId::from_u32(999) });
    assert!(text::print(&module).is_err());
    assert!(text::print_diagnostic(&module).starts_with("NON-CANONICAL"));
    for literal in [Literal::Int("01".into()), Literal::Float(f64::NAN.to_bits())] {
        let mut module = text::parse(SMALL).unwrap(); module.functions.get_mut(f).unwrap().body.as_mut().unwrap().insts.get_mut(InstId::from_u32(0)).unwrap().op = Operation::Const(literal);
        assert!(text::print(&module).is_err());
    }
}

// Explicit payload-branch inventory; schema additions change TEXT_VARIANTS and
// fail this inventory until a corresponding round-trip case is added.
trait CodecSample { fn cs() -> Self; }
fn cs<T: CodecSample>() -> T { T::cs() }
macro_rules! codec_samples { ($($ty:ty => $expr:expr),* $(,)?) => {$(impl CodecSample for $ty { fn cs() -> Self { $expr } })*}; }
codec_samples!(u8 => 0, u16 => 0, u32 => 0, u64 => 0, bool => false, String => "sample # // 🦀".into(),
Literal => Literal::Int("0".into()));
impl<T: CodecSample> CodecSample for Vec<T> { fn cs() -> Self { vec![] } }
impl<T: CodecSample> CodecSample for Option<T> { fn cs() -> Self { None } }
impl<T: CodecSample> CodecSample for Box<T> { fn cs() -> Self { Box::new(cs()) } }
impl<A: CodecSample, B: CodecSample> CodecSample for (A,B) { fn cs() -> Self { (cs(), cs()) } }
codec_samples!(ValueId => ValueId::from_u32(0));
codec_samples!(BlockId => BlockId::from_u32(0));
codec_samples!(InstId => InstId::from_u32(0));
codec_samples!(TypeId => TypeId::from_u32(0));
codec_samples!(DeclId => DeclId::from_u32(0));
codec_samples!(FuncId => FuncId::from_u32(0));
codec_samples!(ScopeId => ScopeId::from_u32(0));
codec_samples!(FileId => FileId::from_u32(0));
codec_samples!(BindingId => BindingId::from_u32(0));
codec_samples!(BoundaryId => BoundaryId::from_u32(0));
codec_samples!(SummaryId => SummaryId::from_u32(0));
codec_samples!(ProtocolId => ProtocolId::from_u32(0));
codec_samples!(SelectionId => SelectionId::from_u32(0));
codec_samples!(DepExprId => DepExprId::from_u32(0));
codec_samples!(KeyExprId => KeyExprId::from_u32(0));
codec_samples!(CreationSiteId => CreationSiteId::from_u32(0));
codec_samples!(Feature => Feature::Core);
codec_samples!(Import => Import { path: cs::<String>(), major: cs::<Option<u32>>(), alias: cs::<Option<String>>(), take: cs::<Vec<String>>() });
codec_samples!(TestDecl => TestDecl { name: cs::<String>(), function: cs::<FuncId>() });
codec_samples!(SourceFileDecl => SourceFileDecl { path: cs::<String>() });
codec_samples!(Declaration => Declaration { name: cs::<String>(), type_params: cs::<Vec<TypeParameter>>(), kind: cs::<DeclarationKind>(), loc: cs::<Option<DebugLoc>>() });
codec_samples!(DeclarationKind => DeclarationKind::Object { fields: cs::<Vec<Field>>(), witnesses: cs::<Vec<Witness>>(), destroy: cs::<Option<FuncId>>(), builtin: cs::<bool>() });
codec_samples!(Field => Field { name: cs::<String>(), ty: cs::<TypeId>(), default: cs::<Option<FuncId>>(), once: cs::<bool>() });
codec_samples!(Witness => Witness { role: cs::<TypeId>(), slots: cs::<Vec<FuncId>>() });
codec_samples!(RoleMethod => RoleMethod { name: cs::<String>(), signature: cs::<Signature>() });
codec_samples!(VariantCase => VariantCase { name: cs::<String>(), layout: cs::<DeclId>() });
codec_samples!(ComputedField => ComputedField { name: cs::<String>(), function: cs::<FuncId>() });
codec_samples!(Signature => Signature { params: cs::<Vec<TypeId>>(), results: cs::<Vec<TypeId>>(), ownership_abi: cs::<OwnershipAbi>() });
codec_samples!(OwnershipAbi => OwnershipAbi::Unelaborated);
codec_samples!(FunctionAttr => FunctionAttr::Entry);
codec_samples!(ScopeDecl => ScopeDecl { function: cs::<FuncId>(), parent: cs::<Option<ScopeId>>(), kind: cs::<ScopeKind>(), loc: cs::<Option<DebugLoc>>() });
codec_samples!(ScopeKind => ScopeKind::Action);
codec_samples!(BindingDecl => BindingDecl { name: cs::<String>(), scope: cs::<ScopeId>(), ty: cs::<TypeId>(), loc: cs::<Option<DebugLoc>>() });
codec_samples!(Type => Type::Int);
codec_samples!(FnType => FnType { params: cs::<Vec<TypeId>>(), result: cs::<Option<TypeId>>(), is_async: cs::<bool>() });
codec_samples!(ServiceType => ServiceType::Own);
codec_samples!(GenericOwner => GenericOwner::Declaration(cs::<DeclId>()));
codec_samples!(TypeParameterRef => TypeParameterRef { owner: cs::<GenericOwner>(), index: cs::<u32>() });
codec_samples!(TypeParameter => TypeParameter { name: cs::<String>(), bounds: cs::<Vec<TypeId>>() });
codec_samples!(Outcome => Outcome::Normal);
codec_samples!(EdgeResult => EdgeResult { outcome: cs::<Outcome>(), slot: cs::<u16>() });
codec_samples!(SwitchCase => SwitchCase { value: cs::<crate::Literal>(), target: cs::<Target>() });
codec_samples!(LookupKind => LookupKind::Array);
codec_samples!(TrapCode => TrapCode::Confirm);
codec_samples!(OperandMode => OperandMode::Borrow);
codec_samples!(OperandConvention => OperandConvention::Borrow);
codec_samples!(OperandUse => OperandUse { value: cs::<ValueId>(), mode: cs::<OperandMode>() });
codec_samples!(OperandRef => OperandRef::Value(cs::<ValueId>()));
codec_samples!(ReferencedOperand => ReferencedOperand { reference: cs::<OperandRef>(), mode: cs::<OperandMode>() });
codec_samples!(ValueFacts => ValueFacts::Unelaborated);
codec_samples!(PlainCertificate => PlainCertificate::Scalar);
codec_samples!(InstructionFacts => InstructionFacts { result_deps: cs::<Vec<(u16, DepExprId)>>(), retention: cs::<Vec<SelectionId>>(), provenance: cs::<Vec<ProvenanceFact>>() });
codec_samples!(ProvenanceFact => ProvenanceFact { obligations: cs::<SelectionId>(), state: cs::<AdmissionProvenance>() });
codec_samples!(AdmissionProvenance => AdmissionProvenance::UnboundResult);
codec_samples!(ResourceValue => ResourceValue { value: cs::<ValueId>(), packet: cs::<ValueId>() });
codec_samples!(KeyExpr => KeyExpr::Created { site: cs::<CreationSiteId>(), instance: cs::<ValueId>() });
codec_samples!(ServiceKeyKind => ServiceKeyKind::Cache);
codec_samples!(DepExpr => DepExpr::Empty);
codec_samples!(SelectionExpr => SelectionExpr::Value(cs::<ValueId>()));
codec_samples!(Projection => Projection::Field { value: cs::<ValueId>(), field: cs::<FieldRef>() });
codec_samples!(FieldRef => FieldRef { declaration: cs::<DeclId>(), index: cs::<u32>() });
codec_samples!(Boundary => Boundary { scope: cs::<ScopeId>(), kind: cs::<BoundaryKind>() });
codec_samples!(BoundaryKind => BoundaryKind::Lexical);
codec_samples!(ResultType => ResultType::Exact(cs::<TypeId>()));
codec_samples!(ScalarType => ScalarType::Int);
codec_samples!(SlotName => SlotName::Value);
codec_samples!(ResultSlot => ResultSlot { name: cs::<SlotName>(), ty: cs::<ResultType>() });
codec_samples!(Retention => Retention::Global(cs::<DeclId>()));
codec_samples!(RetentionRule => RetentionRule { result: cs::<SlotName>(), selection: cs::<Retention>() });
codec_samples!(ProvenanceTransition => ProvenanceTransition::Preserve);
codec_samples!(OutcomeContract => OutcomeContract { results: cs::<Vec<ResultSlot>>(), transfers: cs::<Vec<TransferRule>>(), postconditions: cs::<Vec<Postcondition>>(), retention: cs::<Vec<RetentionRule>>() });
codec_samples!(TransferRule => TransferRule::Preserve { input: cs::<ValueId>(), output: cs::<SlotName>() });
codec_samples!(Postcondition => Postcondition::Closed(cs::<ValueId>()));
codec_samples!(FailureKind => FailureKind::Thrown);
codec_samples!(Effects => Effects { failures: cs::<Vec<FailureKind>>(), suspends: cs::<bool>(), allocates: cs::<bool>(), publishes: cs::<bool>(), cleanup: cs::<bool>(), mutates: cs::<bool>() });
codec_samples!(RootKind => RootKind::Operands);
codec_samples!(RootContract => RootContract { roots: cs::<Vec<RootKind>>(), through_suspend: cs::<bool>() });
codec_samples!(CancellationContract => CancellationContract { outcome: cs::<OutcomeContract>(), delivery: cs::<CancellationDelivery>() });
codec_samples!(CancellationDelivery => CancellationDelivery::Unwind);
codec_samples!(OpContract => OpContract { operands: cs::<Vec<ReferencedOperand>>(), normal: cs::<OutcomeContract>(), unwind: cs::<Option<OutcomeContract>>(), cancellation: cs::<Option<CancellationContract>>(), effects: cs::<Effects>(), roots: cs::<RootContract>(), entry_kind: cs::<EntryKind>() });
codec_samples!(EntryKind => EntryKind::CallBoundary);
codec_samples!(FreshConvention => FreshConvention::Borrow);
codec_samples!(ParameterEffect => ParameterEffect { convention: cs::<FreshConvention>(), retention: cs::<Option<SummarySelection>>(), deps: cs::<Vec<u16>>(), destination: cs::<TransferDestination>() });
codec_samples!(SummarySelection => SummarySelection::Parameter(cs::<u16>()));
codec_samples!(TransferDestination => TransferDestination::None);
codec_samples!(ResultRelation => ResultRelation { fresh: cs::<Vec<CreationSiteId>>(), aliases: cs::<Vec<SummarySelection>>(), external_deps: cs::<Vec<u16>>(), provenance: cs::<AdmissionProvenance>() });
codec_samples!(Summary => Summary { entry_kind: cs::<EntryKind>(), parameters: cs::<Vec<ParameterEffect>>(), results: cs::<Vec<ResultRelation>>(), contract: cs::<CallContract>() });
codec_samples!(CallContract => CallContract { results: cs::<Vec<TypeId>>(), failures: cs::<Vec<FailureKind>>(), suspends: cs::<bool>(), async_destroy: cs::<bool>(), publication: cs::<Publication>(), protocol: cs::<Option<ProtocolId>>() });
codec_samples!(Publication => Publication::Atomic);
codec_samples!(HolderKind => HolderKind::Packet);
codec_samples!(HolderState => HolderState::Open);
codec_samples!(ProtocolTransition => ProtocolTransition::Attach);
codec_samples!(ProtocolCheck => ProtocolCheck::LiveDependencies);
codec_samples!(ProtocolPoststate => ProtocolPoststate { holder: cs::<HolderKind>(), state: cs::<HolderState>() });
codec_samples!(ProtocolRule => ProtocolRule { transition: cs::<ProtocolTransition>(), from: cs::<HolderKind>(), to: cs::<HolderKind>(), checks: cs::<Vec<ProtocolCheck>>(), normal: cs::<ProtocolPoststate>(), unwind: cs::<Option<ProtocolPoststate>>(), cancel: cs::<Option<ProtocolPoststate>>() });
codec_samples!(ProtocolDecl => ProtocolDecl { kind: ProtocolKind::Storage,  holders: cs::<Vec<HolderKind>>(),
    rules: cs::<Vec<ProtocolRule>>(), roots: cs::<Vec<RootKind>>() });
codec_samples!(DescriptorContract => DescriptorContract { managed: cs::<DescriptorManaged>(), retention: cs::<SummarySelection>(), cleanup: cs::<FuncId>(), async_destroy: cs::<bool>(), summary: cs::<SummaryId>() });
codec_samples!(DescriptorManaged => DescriptorManaged::Plain);
codec_samples!(NumericType => NumericType::Int);
codec_samples!(FieldValue => FieldValue { field: cs::<FieldRef>(), value: cs::<ValueId>() });
codec_samples!(ConstructionStatus => ConstructionStatus::Complete);
codec_samples!(ConstructionCompletion => ConstructionCompletion { state: cs::<ValueId>() });
codec_samples!(CallArgument => CallArgument { value: cs::<ValueId>(), convention: cs::<OperandConvention>() });
codec_samples!(CallSite => CallSite { cleanup: CallCleanupMode::Ordinary,  args: cs::<Vec<CallArgument>>(),
    packet: cs::<Option<ValueId>>(), deps: cs::<Vec<ValueId>>(), descriptors: cs::<Vec<ValueId>>(),
    type_args: cs::<Vec<TypeId>>(), summary: cs::<Option<SummaryId>>() });
codec_samples!(ResultDemand => ResultDemand::Want);
codec_samples!(DeferAction => DeferAction { replacements: vec![], function: cs::<FuncId>(),
    captures: cs::<Vec<ValueId>>(), bindings: cs::<Vec<BindingId>>() });
codec_samples!(TransferStates => TransferStates::Same(cs::<ValueId>()));
codec_samples!(DynamicReason => DynamicReason::Atomic);
codec_samples!(AccessContext => AccessContext { owner: cs::<ValueId>(), holder: cs::<ValueId>(), authority: cs::<ValueId>() });
codec_samples!(ElaboratedBinding => ElaboratedBinding { binding: cs::<BindingId>(), resource: cs::<ResourceValue>(), state: cs::<ValueId>(), original_scope: cs::<ScopeId>() });
codec_samples!(ElaboratedStage => ElaboratedStage { action: cs::<ScopeId>(), field: cs::<FieldRef>(), resource: cs::<ResourceValue>(), states: cs::<TransferStates>() });
codec_samples!(TakeBoundary => TakeBoundary::Existing { owner: cs::<ScopeId>() });
codec_samples!(UniqueCacheProof => UniqueCacheProof { future: cs::<ValueId>(), scope: cs::<ScopeId>(), region: cs::<Vec<BlockId>>(), evidence: cs::<UniqueCacheEvidence>() });
codec_samples!(CreationOrigin => CreationOrigin::Instruction(cs()));
codec_samples!(UniqueCacheEvidence => UniqueCacheEvidence { creation: cs::<CreationOrigin>(), aliases: cs::<Vec<ValueId>>(), frame_slots: cs::<Vec<u32>>(), no_external_storage: cs::<bool>(), no_other_waiters: cs::<bool>(), no_live_views: cs::<bool>(), no_collector_retention: cs::<bool>(), scheduler_cannot_publish: cs::<bool>() });
codec_samples!(FrameSlot => FrameSlot { slot: cs::<u32>(), value: cs::<ValueId>(), mode: cs::<OperandMode>() });
codec_samples!(SavedSlot => SavedSlot { slot: cs::<u32>(), ty: cs::<TypeId>(), linear: cs::<bool>() });
codec_samples!(CarrierDestination => CarrierDestination::FailureContext(cs::<ScopeId>()));
codec_samples!(ExitKind => ExitKind::Return);
codec_samples!(ExitDestination => ExitDestination::Caller);
codec_samples!(AbortPhase => AbortPhase::BeforeDestroy);
codec_samples!(PreparedExit => PreparedExit { kind: cs::<ExitKind>(), value: cs::<Option<ValueId>>(), packet: cs::<Option<ValueId>>(), deps: cs::<Option<ValueId>>(), selection: cs::<SelectionId>(), boundary: cs::<BoundaryId>() });

#[test]
fn every_typed_payload_enum_branch_roundtrips() {
    let cases = vec![
        Feature::Core,
        Feature::Async,
        Feature::Process,
        Feature::Test
    ];
    assert_eq!(cases.len(), Feature::TEXT_VARIANTS.len(), "Feature branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        DeclarationKind::Object { fields: cs::<Vec<Field>>(), witnesses: cs::<Vec<Witness>>(), destroy: cs::<Option<FuncId>>(), builtin: cs::<bool>() },
        DeclarationKind::Role { methods: cs::<Vec<RoleMethod>>(), builtin: cs::<bool>() },
        DeclarationKind::Enum { cases: cs::<Vec<String>>(), is_error: cs::<bool>() },
        DeclarationKind::Variant { cases: cs::<Vec<VariantCase>>(), is_error: cs::<bool>() },
        DeclarationKind::Handle,
        DeclarationKind::Cap { owner: cs::<TypeId>(), fields: cs::<Vec<ComputedField>>() },
        DeclarationKind::Global { ty: cs::<TypeId>(), init: cs::<FuncId>() }
    ];
    assert_eq!(cases.len(), DeclarationKind::TEXT_VARIANTS.len(), "DeclarationKind branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        OwnershipAbi::Unelaborated,
        OwnershipAbi::Explicit { input_packet: cs::<u16>(), output_packet: cs::<u16>() },
        OwnershipAbi::Observation
    ];
    assert_eq!(cases.len(), OwnershipAbi::TEXT_VARIANTS.len(), "OwnershipAbi branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        FunctionAttr::Entry,
        FunctionAttr::Method(cs::<DeclId>()),
        FunctionAttr::Ctor(cs::<DeclId>()),
        FunctionAttr::Successor(cs::<DeclId>()),
        FunctionAttr::Default { object: cs::<DeclId>(), field: cs::<u32>() },
        FunctionAttr::Destroy(cs::<DeclId>()),
        FunctionAttr::Extern(cs::<String>()),
        FunctionAttr::Hidden,
        FunctionAttr::Intrinsic { name: cs::<String>(), objects: cs::<Vec<DeclId>>(), helpers: cs::<Vec<FuncId>>() },
        FunctionAttr::Async,
        FunctionAttr::Cleanup,
        FunctionAttr::DeferEntry { replacements: vec![] },
        FunctionAttr::Lambda { captures: cs::<u32>(), owner: cs::<Option<DeclId>>() }
    ];
    assert_eq!(cases.len(), FunctionAttr::TEXT_VARIANTS.len(), "FunctionAttr branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        ScopeKind::Action,
        ScopeKind::Block,
        ScopeKind::Branch,
        ScopeKind::Iteration,
        ScopeKind::Try,
        ScopeKind::Catch,
        ScopeKind::Defer
    ];
    assert_eq!(cases.len(), ScopeKind::TEXT_VARIANTS.len(), "ScopeKind branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        Type::Int,
        Type::Bool,
        Type::Byte,
        Type::Float,
        Type::Rat,
        Type::Char,
        Type::Str,
        Type::Range,
        Type::Any,
        Type::Never,
        Type::Object { decl: cs::<DeclId>(), args: cs::<Vec<TypeId>>() },
        Type::Role { decl: cs::<DeclId>(), args: cs::<Vec<TypeId>>() },
        Type::Enum(cs::<DeclId>()),
        Type::Variant(cs::<DeclId>()),
        Type::Handle(cs::<DeclId>()),
        Type::Cap { decl: cs::<DeclId>(), args: cs::<Vec<TypeId>>() },
        Type::Successor(cs::<TypeId>()),
        Type::Array(cs::<TypeId>()),
        Type::Map(cs::<TypeId>(), cs::<TypeId>()),
        Type::Builder(cs::<TypeId>()),
        Type::MapBuilder(cs::<TypeId>(), cs::<TypeId>()),
        Type::Fn(cs::<FnType>()),
        Type::Future(cs::<Option<TypeId>>()),
        Type::Parameter(cs::<TypeParameterRef>()),
        Type::Descriptor,
        Type::Frame(cs::<FuncId>()),
        Type::LowCell(cs::<TypeId>()),
        Type::Service(cs::<ServiceType>())
    ];
    assert_eq!(cases.len(), Type::TEXT_VARIANTS.len(), "Type branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        ServiceType::Own,
        ServiceType::Deps,
        ServiceType::Scope(cs::<ScopeId>()),
        ServiceType::Pending,
        ServiceType::Closed,
        ServiceType::UniqueCache,
        ServiceType::Failure,
        ServiceType::FailureState,
        ServiceType::ExitState,
        ServiceType::StableKey,
        ServiceType::Owner,
        ServiceType::Holder,
        ServiceType::OwnerSnapshot,
        ServiceType::CacheState,
        ServiceType::CarrierState,
        ServiceType::TaskState,
        ServiceType::Pin,
        ServiceType::Receipt,
        ServiceType::CleanupState
    ];
    assert_eq!(cases.len(), ServiceType::TEXT_VARIANTS.len(), "ServiceType branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        GenericOwner::Declaration(cs::<DeclId>()),
        GenericOwner::Function(cs::<FuncId>())
    ];
    assert_eq!(cases.len(), GenericOwner::TEXT_VARIANTS.len(), "GenericOwner branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        Outcome::Normal,
        Outcome::Unwind,
        Outcome::Cancel,
        Outcome::DeferContinue,
        Outcome::DeferReplace(0),
        Outcome::Dispatch(0)
    ];
    assert_eq!(cases.len(), Outcome::TEXT_VARIANTS.len(), "Outcome branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        LookupKind::Array,
        LookupKind::Map
    ];
    assert_eq!(cases.len(), LookupKind::TEXT_VARIANTS.len(), "LookupKind branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        TrapCode::Confirm,
        TrapCode::Range,
        TrapCode::Empty,
        TrapCode::Unbound,
        TrapCode::Unreachable,
        TrapCode::Pending
    ];
    assert_eq!(cases.len(), TrapCode::TEXT_VARIANTS.len(), "TrapCode branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        OperandMode::Borrow,
        OperandMode::Consume,
        OperandMode::Observe,
        OperandMode::DestroyBorrow,
        OperandMode::Forward
    ];
    assert_eq!(cases.len(), OperandMode::TEXT_VARIANTS.len(), "OperandMode branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        OperandConvention::Borrow,
        OperandConvention::Consume
    ];
    assert_eq!(cases.len(), OperandConvention::TEXT_VARIANTS.len(), "OperandConvention branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        OperandRef::Value(cs::<ValueId>()),
        OperandRef::Selection(cs::<SelectionId>()),
        OperandRef::Dependency(cs::<DepExprId>()),
        OperandRef::Key(cs::<KeyExprId>())
    ];
    assert_eq!(cases.len(), OperandRef::TEXT_VARIANTS.len(), "OperandRef branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        ValueFacts::Unelaborated,
        ValueFacts::Plain { certificate: cs::<PlainCertificate>() },
        ValueFacts::Managed { deps: cs::<DepExprId>() },
        ValueFacts::Service
    ];
    assert_eq!(cases.len(), ValueFacts::TEXT_VARIANTS.len(), "ValueFacts branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        PlainCertificate::Scalar,
        PlainCertificate::ClosedGraph { ty: cs::<TypeId>() },
        PlainCertificate::Descriptor(cs::<ValueId>())
    ];
    assert_eq!(cases.len(), PlainCertificate::TEXT_VARIANTS.len(), "PlainCertificate branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        AdmissionProvenance::UnboundResult,
        AdmissionProvenance::Bound,
        AdmissionProvenance::StagedOrCarrierOwned,
        AdmissionProvenance::CacheOwned,
        AdmissionProvenance::ExtractedAtExistingBoundary,
        AdmissionProvenance::TemporaryAwaitResult
    ];
    assert_eq!(cases.len(), AdmissionProvenance::TEXT_VARIANTS.len(), "AdmissionProvenance branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        KeyExpr::Created { site: cs::<CreationSiteId>(), instance: cs::<ValueId>() },
        KeyExpr::ServiceOf { value: cs::<ValueId>(), kind: cs::<ServiceKeyKind>() },
        KeyExpr::Runtime(cs::<ValueId>())
    ];
    assert_eq!(cases.len(), KeyExpr::TEXT_VARIANTS.len(), "KeyExpr branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        ServiceKeyKind::Cache,
        ServiceKeyKind::Frame,
        ServiceKeyKind::Channel,
        ServiceKeyKind::Carrier,
        ServiceKeyKind::Pin,
        ServiceKeyKind::Group
    ];
    assert_eq!(cases.len(), ServiceKeyKind::TEXT_VARIANTS.len(), "ServiceKeyKind branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        DepExpr::Empty,
        DepExpr::Key(cs::<KeyExprId>()),
        DepExpr::Union(cs::<Vec<DepExprId>>()),
        DepExpr::FromSsa(cs::<ValueId>()),
        DepExpr::Retention { value: cs::<ValueId>(), selection: cs::<SelectionId>() }
    ];
    assert_eq!(cases.len(), DepExpr::TEXT_VARIANTS.len(), "DepExpr branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        SelectionExpr::Admissible(cs()),
        SelectionExpr::Value(cs::<ValueId>()),
        SelectionExpr::Projection(cs::<Projection>()),
        SelectionExpr::OwnerGraph(cs::<ValueId>()),
        SelectionExpr::ViewOwner(cs::<ValueId>()),
        SelectionExpr::Union(cs::<Vec<SelectionId>>()),
        SelectionExpr::Descriptor { descriptor: cs::<ValueId>(), value: cs::<ValueId>() }
    ];
    assert_eq!(cases.len(), SelectionExpr::TEXT_VARIANTS.len(), "SelectionExpr branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        Projection::Field { value: cs::<ValueId>(), field: cs::<FieldRef>() },
        Projection::Index { value: cs::<ValueId>(), index: cs::<ValueId>() },
        Projection::Slice { value: cs::<ValueId>(), start: cs::<ValueId>(), end: cs::<ValueId>() },
        Projection::Map { value: cs::<ValueId>(), keys: cs::<ValueId>() }
    ];
    assert_eq!(cases.len(), Projection::TEXT_VARIANTS.len(), "Projection branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        BoundaryKind::Lexical,
        BoundaryKind::Call,
        BoundaryKind::Loop,
        BoundaryKind::AbortBeforeDestroy,
        BoundaryKind::AbortAfterPrefix
    ];
    assert_eq!(cases.len(), BoundaryKind::TEXT_VARIANTS.len(), "BoundaryKind branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        ResultType::Exact(cs::<TypeId>()),
        ResultType::ExactEnum(cs::<DeclId>()),
        ResultType::Scalar(cs::<ScalarType>()),
        ResultType::Service(cs::<ServiceType>()),
        ResultType::SameAs(cs::<ValueId>()),
        ResultType::Element(cs::<ValueId>()),
        ResultType::MapKey(cs::<ValueId>()),
        ResultType::MapValue(cs::<ValueId>()),
        ResultType::Field(cs::<FieldRef>()),
        ResultType::ArrayOf(cs::<Box<ResultType>>()),
        ResultType::Object { decl: cs::<DeclId>(), args: cs::<Vec<TypeId>>() },
        ResultType::FutureOf(cs::<Option<TypeId>>()),
        ResultType::Awaited(cs::<ValueId>()),
        ResultType::SuccessorOf(cs::<ValueId>()),
        ResultType::BuilderOf(cs::<TypeId>()),
        ResultType::MapBuilderOf(cs::<TypeId>(), cs::<TypeId>()),
        ResultType::Finished(cs::<ValueId>()),
        ResultType::Frame(cs::<FuncId>()),
        ResultType::CellOf(cs::<TypeId>()),
        ResultType::CellValue(cs::<ValueId>())
    ];
    assert_eq!(cases.len(), ResultType::TEXT_VARIANTS.len(), "ResultType branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        ScalarType::Int,
        ScalarType::Bool,
        ScalarType::Byte,
        ScalarType::Float,
        ScalarType::Rat,
        ScalarType::Char,
        ScalarType::Str,
        ScalarType::Range,
        ScalarType::Any,
        ScalarType::Descriptor
    ];
    assert_eq!(cases.len(), ScalarType::TEXT_VARIANTS.len(), "ScalarType branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        SlotName::Value,
        SlotName::Packet,
        SlotName::Deps,
        SlotName::Rest,
        SlotName::Selected,
        SlotName::ExternalDeps,
        SlotName::State,
        SlotName::From,
        SlotName::To,
        SlotName::Failure,
        SlotName::Pending,
        SlotName::Closed,
        SlotName::Buffer,
        SlotName::Unique,
        SlotName::Receipt,
        SlotName::Task,
        SlotName::Pin,
        SlotName::Snapshot,
        SlotName::Slot(cs::<u16>())
    ];
    assert_eq!(cases.len(), SlotName::TEXT_VARIANTS.len(), "SlotName branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        Retention::Global(cs::<DeclId>()),
        Retention::Value(cs::<ValueId>()),
        Retention::Field { base: cs::<ValueId>(), field: cs::<FieldRef>() },
        Retention::Index { base: cs::<ValueId>(), index: cs::<ValueId>() },
        Retention::Slice { base: cs::<ValueId>(), start: cs::<ValueId>(), end: cs::<ValueId>() },
        Retention::MapEntry { map: cs::<ValueId>(), key: cs::<ValueId>() },
        Retention::MapPosition { map: cs::<ValueId>(), index: cs::<ValueId>(), key: cs::<bool>() },
        Retention::Keys(cs::<ValueId>()),
        Retention::Values(cs::<ValueId>()),
        Retention::ArrayReplace { array: cs::<ValueId>(), index: cs::<ValueId>(), value: cs::<ValueId>() },
        Retention::ArrayRemove { array: cs::<ValueId>(), index: cs::<ValueId>() },
        Retention::Difference { base: cs::<ValueId>(), removed: cs::<Vec<ValueId>>() },
        Retention::MapMerge(cs::<Vec<ValueId>>()),
        Retention::Union(cs::<Vec<ValueId>>()),
        Retention::ObjectFields { object: cs::<ValueId>(), updates: cs::<Vec<FieldValue>>() },
        Retention::Entries(cs::<Vec<(ValueId, ValueId)>>()),
        Retention::OwnerGraph(cs::<ValueId>()),
        Retention::ViewOwner(cs::<ValueId>()),
        Retention::Descriptor { descriptor: cs::<ValueId>(), value: cs::<ValueId>() },
        Retention::Cache(cs::<ValueId>()),
        Retention::TakenOutcome { future: cs::<ValueId>(), boundary: cs::<TakeBoundary>() },
        Retention::AwaitFailure { future: cs::<ValueId>(), take: cs::<Option<TakeBoundary>>() },
        Retention::CarrierGraph(cs::<Vec<ValueId>>())
    ];
    assert_eq!(cases.len(), Retention::TEXT_VARIANTS.len(), "Retention branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        ProvenanceTransition::Preserve,
        ProvenanceTransition::Set(cs::<AdmissionProvenance>())
    ];
    assert_eq!(cases.len(), ProvenanceTransition::TEXT_VARIANTS.len(), "ProvenanceTransition branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        TransferRule::CloseReserve { scope: cs(), reserve: cs(), output: cs() },
        TransferRule::Preserve { input: cs::<ValueId>(), output: cs::<SlotName>() },
        TransferRule::Move { inputs: cs::<Vec<ValueId>>(), outputs: cs::<Vec<SlotName>>() },
        TransferRule::Partition { input: cs::<ValueId>(), selection: cs::<SelectionId>(), rest: cs::<SlotName>(), selected: cs::<SlotName>() },
        TransferRule::Admit { state: cs::<ValueId>(), packet: cs::<ValueId>(), output: cs::<SlotName>(), provenance: cs::<ProvenanceTransition>() },
        TransferRule::ScopeTransfer { states: cs::<TransferStates>(), selection: cs::<SelectionId>() },
        TransferRule::Settle { inputs: cs::<Vec<ValueId>>() },
        TransferRule::Close { input: cs::<ValueId>() },
        TransferRule::Protocol { protocol: cs::<ProtocolId>(), transition: cs::<ProtocolTransition>(), inputs: cs::<Vec<ValueId>>(), outputs: cs::<Vec<SlotName>>() },
        TransferRule::Publish { input: cs::<ValueId>(), holder: cs::<HolderKind>(), provenance: cs::<AdmissionProvenance>() },
        TransferRule::Extract { future: cs::<ValueId>(), capability: cs::<ValueId>(), boundary: cs::<TakeBoundary>() },
        TransferRule::Reserve { packet: cs::<ValueId>(), boundary: cs::<BoundaryId>() },
        TransferRule::ExtendReservation { pending: cs::<ValueId>(), packet: cs::<ValueId>() },
        TransferRule::ReplaceReservation { pending: cs::<ValueId>(), packet: cs::<Option<ValueId>>() },
        TransferRule::AbortReservation { pending: cs::<ValueId>(), cleanup: cs::<ValueId>(), phase: cs::<AbortPhase>() },
        TransferRule::FrameSave { frame: cs::<ValueId>(), values: cs::<Vec<ValueId>>() },
        TransferRule::FrameResume { frame: cs::<ValueId>(), slots: cs::<Vec<u32>>() },
        TransferRule::CompleteIdentity { identity: cs::<ValueId>(), state: cs::<ValueId>(), output: cs::<SlotName>() }
    ];
    assert_eq!(cases.len(), TransferRule::TEXT_VARIANTS.len(), "TransferRule branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        Postcondition::OwnerServiceDuty { value: SlotName::Value, owner: cs(), kind: ServiceKeyKind::Cache },
        Postcondition::Closed(cs::<ValueId>()),
        Postcondition::CacheConsumed(cs::<ValueId>()),
        Postcondition::AdmissionStaysCaller,
        Postcondition::PreserveFullDependencies,
        Postcondition::TransferableScopeDomainOnly,
        Postcondition::FreshAdmission(cs::<AdmissionProvenance>()),
        Postcondition::EpochOnRealTransfer,
        Postcondition::ProvesUniqueCache(cs::<UniqueCacheProof>()),
        Postcondition::WorkerSerializationOnly,
        Postcondition::CarrierGraph(cs::<ValueId>()),
        Postcondition::CleanupCompleteBeforeCommit,
        Postcondition::CancellationDeliveredBeforeCommit,
        Postcondition::ResultRelation(cs::<ResultRelation>()),
        Postcondition::NoPublication,
        Postcondition::CalleeInputsSettled,
        Postcondition::UniqueCacheRecheckAfterCoro,
        Postcondition::DestroyingBeforeCall,
        Postcondition::DeadAfterCall(cs::<ValueId>()),
        Postcondition::ContinueCleanup,
        Postcondition::Shielded,
        Postcondition::ServiceSettleBeforeSerialPlan,
        Postcondition::SerialOrder,
        Postcondition::NoFreshAdmission,
        Postcondition::ReceiptDeterminesHolder,
        Postcondition::TaskDeterminesHolder,
        Postcondition::PreserveProvenance,
        Postcondition::CacheConsumedIfSettled(cs::<ValueId>()),
        Postcondition::NonLinearSlotOnly,
        Postcondition::Dynamic { target: cs::<ScopeId>(), reason: cs::<DynamicReason>(), protocol: cs::<ProtocolId>() },
        Postcondition::LexicalExit { kind: cs::<ExitKind>(), boundary: cs::<BoundaryId>(), selection: cs::<SelectionId>() },
        Postcondition::CacheRehome(cs::<ScopeId>()),
        Postcondition::ConstructorFieldWrite(cs::<FieldRef>()),
        Postcondition::OnceValuePlain,
        Postcondition::IncompleteIdentity,
        Postcondition::CompleteIdentity(cs::<ValueId>()),
        Postcondition::DischargeMustUse(cs::<ValueId>()),
        Postcondition::CreatesMustUse,
        Postcondition::NoActiveFailureReplacement,
        Postcondition::FullCarrierGraphOwner,
        Postcondition::NativeTransferableOnly,
        Postcondition::ProtocolState { holder: cs::<HolderKind>(), state: cs::<HolderState>() },
        Postcondition::ProtocolAlternatives { protocol: cs::<ProtocolId>(), outcome: cs::<Outcome>(), states: cs::<Vec<ProtocolPoststate>>() }
    ];
    assert_eq!(cases.len(), Postcondition::TEXT_VARIANTS.len(), "Postcondition branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        FailureKind::Thrown,
        FailureKind::Program,
        FailureKind::Panic,
        FailureKind::Native,
        FailureKind::Cancelled
    ];
    assert_eq!(cases.len(), FailureKind::TEXT_VARIANTS.len(), "FailureKind branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        RootKind::Operands,
        RootKind::ActiveSelf,
        RootKind::CleanupRemainder,
        RootKind::PendingExit,
        RootKind::CarrierGraph,
        RootKind::Pins,
        RootKind::FrameSlots,
        RootKind::Cache,
        RootKind::ServiceSchedule
    ];
    assert_eq!(cases.len(), RootKind::TEXT_VARIANTS.len(), "RootKind branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        CancellationDelivery::Unwind,
        CancellationDelivery::AfterShieldedCleanup
    ];
    assert_eq!(cases.len(), CancellationDelivery::TEXT_VARIANTS.len(), "CancellationDelivery branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        EntryKind::CallBoundary,
        EntryKind::ObservationAdmissionStaysCaller,
        EntryKind::Internal
    ];
    assert_eq!(cases.len(), EntryKind::TEXT_VARIANTS.len(), "EntryKind branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        FreshConvention::Borrow,
        FreshConvention::Take
    ];
    assert_eq!(cases.len(), FreshConvention::TEXT_VARIANTS.len(), "FreshConvention branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        SummarySelection::Parameter(cs::<u16>()),
        SummarySelection::Field { parameter: cs::<u16>(), field: cs::<FieldRef>() },
        SummarySelection::Path { parameter: cs::<u16>(), fields: cs::<Vec<FieldRef>>() },
        SummarySelection::OwnerGraph(cs::<u16>()),
        SummarySelection::Descriptor(cs::<u16>()),
        SummarySelection::Union(cs::<Vec<SummarySelection>>())
    ];
    assert_eq!(cases.len(), SummarySelection::TEXT_VARIANTS.len(), "SummarySelection branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        TransferDestination::None,
        TransferDestination::Result,
        TransferDestination::Scope(cs::<ScopeId>()),
        TransferDestination::Protocol(cs::<ProtocolId>()),
        TransferDestination::Cache,
        TransferDestination::FailureContext
    ];
    assert_eq!(cases.len(), TransferDestination::TEXT_VARIANTS.len(), "TransferDestination branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        Publication::Atomic,
        Publication::ChannelReceipt,
        Publication::TaskState
    ];
    assert_eq!(cases.len(), Publication::TEXT_VARIANTS.len(), "Publication branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        HolderKind::Packet,
        HolderKind::Scope,
        HolderKind::Frame,
        HolderKind::Cache,
        HolderKind::Channel,
        HolderKind::Carrier,
        HolderKind::PendingExit,
        HolderKind::Pin,
        HolderKind::Group,
        HolderKind::Diagnostic,
        HolderKind::Receipt
    ];
    assert_eq!(cases.len(), HolderKind::TEXT_VARIANTS.len(), "HolderKind branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        HolderState::Open,
        HolderState::Unaccepted,
        HolderState::Accepted,
        HolderState::Delivered,
        HolderState::Running,
        HolderState::Settled,
        HolderState::Consumed,
        HolderState::Released,
        HolderState::Closed
    ];
    assert_eq!(cases.len(), HolderState::TEXT_VARIANTS.len(), "HolderState branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        ProtocolTransition::Attach,
        ProtocolTransition::Transfer,
        ProtocolTransition::Rehome,
        ProtocolTransition::Release,
        ProtocolTransition::Publish,
        ProtocolTransition::Settle,
        ProtocolTransition::Save,
        ProtocolTransition::Resume,
        ProtocolTransition::Cancel,
        ProtocolTransition::Pin,
        ProtocolTransition::Unpin,
        ProtocolTransition::Dynamic
    ];
    assert_eq!(cases.len(), ProtocolTransition::TEXT_VARIANTS.len(), "ProtocolTransition branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        ProtocolCheck::LiveDependencies,
        ProtocolCheck::Snapshot,
        ProtocolCheck::Owner,
        ProtocolCheck::Transferable,
        ProtocolCheck::UniqueCache,
        ProtocolCheck::NoReentry,
        ProtocolCheck::SufficientBoundary
    ];
    assert_eq!(cases.len(), ProtocolCheck::TEXT_VARIANTS.len(), "ProtocolCheck branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        DescriptorManaged::Plain,
        DescriptorManaged::Managed,
        DescriptorManaged::Runtime
    ];
    assert_eq!(cases.len(), DescriptorManaged::TEXT_VARIANTS.len(), "DescriptorManaged branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        NumericType::Int,
        NumericType::Byte,
        NumericType::Float,
        NumericType::Rat,
        NumericType::Bool,
        NumericType::Str
    ];
    assert_eq!(cases.len(), NumericType::TEXT_VARIANTS.len(), "NumericType branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        ConstructionStatus::Complete,
        ConstructionStatus::Incomplete
    ];
    assert_eq!(cases.len(), ConstructionStatus::TEXT_VARIANTS.len(), "ConstructionStatus branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        ResultDemand::Want,
        ResultDemand::Discard
    ];
    assert_eq!(cases.len(), ResultDemand::TEXT_VARIANTS.len(), "ResultDemand branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        TransferStates::Same(cs::<ValueId>()),
        TransferStates::Between { from: cs::<ValueId>(), to: cs::<ValueId>() }
    ];
    assert_eq!(cases.len(), TransferStates::TEXT_VARIANTS.len(), "TransferStates branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        DynamicReason::Atomic,
        DynamicReason::Cell,
        DynamicReason::Channel,
        DynamicReason::Group,
        DynamicReason::Any,
        DynamicReason::Closure,
        DynamicReason::Merge
    ];
    assert_eq!(cases.len(), DynamicReason::TEXT_VARIANTS.len(), "DynamicReason branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        TakeBoundary::Existing { owner: cs::<ScopeId>() },
        TakeBoundary::Temporary { awaiting: cs::<ScopeId>() }
    ];
    assert_eq!(cases.len(), TakeBoundary::TEXT_VARIANTS.len(), "TakeBoundary branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        CarrierDestination::FailureContext(cs::<ScopeId>()),
        CarrierDestination::Cache(cs::<ValueId>()),
        CarrierDestination::Diagnostic(cs::<ValueId>())
    ];
    assert_eq!(cases.len(), CarrierDestination::TEXT_VARIANTS.len(), "CarrierDestination branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        ExitKind::Return,
        ExitKind::Last,
        ExitKind::Next,
        ExitKind::Fallthrough
    ];
    assert_eq!(cases.len(), ExitKind::TEXT_VARIANTS.len(), "ExitKind branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        ExitDestination::Caller,
        ExitDestination::Loop(cs::<BoundaryId>()),
        ExitDestination::Scope(cs::<ScopeId>())
    ];
    assert_eq!(cases.len(), ExitDestination::TEXT_VARIANTS.len(), "ExitDestination branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
    let cases = vec![
        AbortPhase::BeforeDestroy,
        AbortPhase::AfterExecutedPrefix
    ];
    assert_eq!(cases.len(), AbortPhase::TEXT_VARIANTS.len(), "AbortPhase branch missing");
    for value in cases { crate::text::payload_roundtrip(value); }
}
codec_samples!(DebugLoc => DebugLoc { file: FileId::from_u32(0), line: 1, column: 2 },
    Target => Target { block: BlockId::from_u32(0), args: vec![] });

#[test]
fn all_terminators_facts_and_metadata_roundtrip_in_modules() {
    let v = ValueId::from_u32(0); let t = TypeId::from_u32(0); let f = FuncId::from_u32(0);
    let target = Target { block: BlockId::from_u32(0), args: vec![v] };
    let found = InvokeEdge { block: target.block, args: vec![EdgeArg::Value(v), EdgeArg::Produced(EdgeResult { outcome: Outcome::Normal, slot: 0 })] };
    let terms = vec![TermKind::Br(target.clone()), TermKind::Cond { condition: v, yes: target.clone(), no: target.clone() },
        TermKind::Switch { value: v, cases: vec![SwitchCase { value: Literal::Int("2".into()), target: target.clone() }], default: target.clone() },
        TermKind::Ret { values: vec![] }, TermKind::Throw(v), TermKind::Raise(v), TermKind::Panic(v),
        TermKind::Trap { code: TrapCode::Pending, message: "stop".into() }, TermKind::Unreachable,
        TermKind::Catch { failure: v, ty: t, handler: found.clone(), next: target.clone() },
        TermKind::Lookup { kind: LookupKind::Array, collection: v, key: v, found: found.clone(), missing: target.clone() },
        TermKind::Lookup { kind: LookupKind::Map, collection: v, key: v, found, missing: target.clone() }];
    for kind in terms {
        let mut module = build_op(Operation::Const(Literal::Int("1".into())), Profile::High);
        let file = module.files.alloc(SourceFileDecl { path: "meta.ok".into() }).unwrap();
        let loc = Some(DebugLoc { file, line: 123, column: 456 });
        module.name = "meta/module".into(); module.version = Some("1.2.3".into()); module.requires = vec![Feature::Core,Feature::Async,Feature::Process,Feature::Test];
        module.imports = vec![Import { path: "other/path".into(), major: Some(4), alias: Some("lib".into()), take: vec!["A".into(),"B".into()] }];
        module.entry = Some(f); module.tests = Some(vec![TestDecl { name: "test".into(), function: f }]);
        let decl = module.functions.get_mut(f).unwrap(); decl.loc = loc;
        decl.summary = Some(SummaryId::from_u32(0)); decl.async_creation_summary = decl.summary; decl.async_body_summary = decl.summary;
        decl.type_params = vec![TypeParameter { name: "T".into(), bounds: vec![t] }];
        decl.attrs = vec![FunctionAttr::Entry, FunctionAttr::Method(DeclId::from_u32(0)), FunctionAttr::Ctor(DeclId::from_u32(0)),
            FunctionAttr::Successor(DeclId::from_u32(0)), FunctionAttr::Default { object: DeclId::from_u32(0), field: 5 },
            FunctionAttr::Destroy(DeclId::from_u32(0)), FunctionAttr::Extern("native".into()), FunctionAttr::Hidden,
            FunctionAttr::Intrinsic { name: "intrinsic".into(), objects: vec![DeclId::from_u32(0)], helpers: vec![f] },
            FunctionAttr::Async, FunctionAttr::Lambda { captures: 3, owner: Some(DeclId::from_u32(0)) }];
        decl.signature.ownership_abi = OwnershipAbi::Explicit { input_packet: 0, output_packet: 1 };
        let body = decl.body.as_mut().unwrap();
        body.values.get_mut(v).unwrap().facts = ValueFacts::Managed { deps: DepExprId::from_u32(0) };
        body.blocks.get_mut(BlockId::from_u32(0)).unwrap().unwind = Some(BlockUnwind { target: target.clone() });
        body.blocks.get_mut(BlockId::from_u32(0)).unwrap().term = Terminator { kind, loc };
        let inst = body.insts.get_mut(InstId::from_u32(0)).unwrap(); inst.loc = loc;
        inst.facts = InstructionFacts { result_deps: vec![(0,DepExprId::from_u32(0))], retention: vec![SelectionId::from_u32(0)],
            provenance: vec![ProvenanceFact { obligations: SelectionId::from_u32(0), state: AdmissionProvenance::CacheOwned }] };
        body.bindings.get_mut(BindingId::from_u32(0)).unwrap().loc = loc;
        module.scopes.get_mut(ScopeId::from_u32(0)).unwrap().loc = loc;
        module.declarations.get_mut(DeclId::from_u32(0)).unwrap().loc = loc;
        roundtrip(&module);
    }
}

#[test]
fn invoke_schema_does_not_validate_consumption_and_rejects_foreign_slots() {
    let op = Operation::Call(CallOp::Direct { function: FuncId::from_u32(0), call: crate::contract_tests::sample() });
    let mut module = build_op(op, Profile::Own);
    let body = module.functions.get_mut(FuncId::from_u32(0)).unwrap().body.as_mut().unwrap();
    if let TermKind::InvokeOwn { op, .. } = &mut body.blocks.get_mut(BlockId::from_u32(0)).unwrap().term.kind
        && let Operation::Call(CallOp::Direct { call, .. }) = &mut **op { call.args[0].convention = OperandConvention::Consume; }
    let text = roundtrip(&module);
    let bad = text.replace("$value", "$selected");
    assert_eq!(text::parse(&bad).unwrap_err().kind, K::OutcomeSlot);
    let mut bad = module.clone();
    let body = bad.functions.get_mut(FuncId::from_u32(0)).unwrap().body.as_mut().unwrap();
    if let TermKind::InvokeOwn { normal, .. } = &mut body.blocks.get_mut(BlockId::from_u32(0)).unwrap().term.kind {
        normal.args[0] = EdgeArg::Produced(EdgeResult { outcome: Outcome::Unwind, slot: 0 });
    }
    assert!(text::print(&bad).is_err());
}

#[test]
fn inline_type_depth_and_forward_declarations_are_bounded() {
    let nested_type = SMALL.replace("int", "array<array<int>>");
    roundtrip(&text::parse(&nested_type).unwrap());
    assert_eq!(text::parse_with_limits(&nested_type,
        Limits { max_type_depth: 1, ..Limits::default() }).unwrap_err().kind, K::Limit);
    let nested = format!("oir 2 high\n{}", "[".repeat(10000));
    assert_eq!(text::parse(&nested).unwrap_err().kind, K::Limit);
    let forward = SMALL.replace("scope S0 action in @main#1\n", "") + "scope S0 action in @main#1\n";
    assert_eq!(text::print(&text::parse(&forward).unwrap()).unwrap(), SMALL);
    let leading = "oir 2 high\nmodule x\nsummary M0 internal parameters [parameter_effect(convention=borrow, retention=some(";
    let deep = format!("{leading}{}parameter(0){}))] contract call_contract(publication=atomic)\n",
        "union([".repeat(500), "])".repeat(500));
    assert_eq!(text::parse(&deep).unwrap_err().kind, K::Limit);
    let mut cyclic = text::parse(SMALL).unwrap();
    cyclic.types.intern(Type::Array(TypeId::from_u32(cyclic.types.len() as u32))).unwrap();
    assert!(text::print(&cyclic).is_err());
    assert!(text::parse(&SMALL.replace("scope S0", "type t0 = int\nscope S0")).is_err());
}

#[test]
fn arena_renumbering_and_type_interning_order_normalize() {
    let mut module = build_op(Operation::Const(Literal::Int("4".into())), Profile::High);
    let before = module.clone();
    let function = FuncId::from_u32(0);
    let mut body = module.functions.get_mut(function).unwrap().body.take().unwrap();
    let mut editor = FunctionEditor::new(&mut module, &mut body);
    let int = TypeId::from_u32(0);
    editor.insert_instruction(BlockId::from_u32(0), 0, Operation::Const(Literal::Int("9".into())), &[ResultSpec::new(int)], InstructionFacts::default(), None).unwrap();
    let unnumbered = editor.body().clone();
    editor.renumber().unwrap();
    let numbered = editor.body().clone();
    let mut a = module.clone(); a.functions.get_mut(function).unwrap().body = Some(unnumbered);
    let mut b = module; b.functions.get_mut(function).unwrap().body = Some(numbered);
    assert!(text::normalized_eq(&a, &b).unwrap());
    assert!(!text::normalized_eq(&a, &before).unwrap());
    let mut left = ModuleBuilder::new("types", Profile::Low);
    let li = left.intern_type(Type::Int).unwrap(); left.intern_type(Type::Bool).unwrap(); left.intern_type(Type::Array(li)).unwrap();
    let mut right = ModuleBuilder::new("types", Profile::Low);
    right.intern_type(Type::Bool).unwrap(); let ri = right.intern_type(Type::Int).unwrap(); right.intern_type(Type::Array(ri)).unwrap();
    assert!(text::normalized_eq(&left.finish().unwrap(), &right.finish().unwrap()).unwrap());
}

#[test]
fn call_result_schemas_include_void_and_indexed_slots_and_protocol_receipts() {
    for n in [0,1,3] {
        for packet in [None, Some(ValueId::from_u32(0))] {
            for publication in [Publication::Atomic, Publication::ChannelReceipt, Publication::TaskState] {
                let mut call: CallSite = crate::contract_tests::sample(); call.packet = packet;
                let op = Operation::Call(CallOp::Direct { function: FuncId::from_u32(0), call });
                let module = build_op_config(op, Profile::Own, |b| {
                    let summary = b.raw_mut().summaries.get_mut(SummaryId::from_u32(0)).unwrap();
                    summary.contract.results = vec![TypeId::from_u32(0); n]; summary.contract.publication = publication;
                    summary.contract.protocol = Some(ProtocolId::from_u32(0));
                });
                let text = roundtrip(&module);
                if n > 1 { assert!(text.contains("$slot2")); }
                if publication == Publication::ChannelReceipt { assert!(text.contains("$receipt")); }
                if publication == Publication::TaskState { assert!(text.contains("$task")); }
            }
        }
    }
}

#[test]
fn every_variadic_opcode_field_has_zero_one_many_cases() {
    roundtrip(&build_op(Operation::Core(CoreOp::ObjNew { declaration: crate::contract_tests::sample::<DeclId>(), type_args: Vec::<TypeId>::new(), fields: crate::contract_tests::sample::<Vec<ValueId>>(), packet: crate::contract_tests::sample::<Option<ValueId>>(), construction: crate::contract_tests::sample::<ConstructionStatus>() }), Profile::Own));
    roundtrip(&build_op(Operation::Core(CoreOp::ObjNew { declaration: crate::contract_tests::sample::<DeclId>(), type_args: vec![cs::<TypeId>(); 1], fields: crate::contract_tests::sample::<Vec<ValueId>>(), packet: crate::contract_tests::sample::<Option<ValueId>>(), construction: crate::contract_tests::sample::<ConstructionStatus>() }), Profile::Own));
    roundtrip(&build_op(Operation::Core(CoreOp::ObjNew { declaration: crate::contract_tests::sample::<DeclId>(), type_args: vec![cs::<TypeId>(); 3], fields: crate::contract_tests::sample::<Vec<ValueId>>(), packet: crate::contract_tests::sample::<Option<ValueId>>(), construction: crate::contract_tests::sample::<ConstructionStatus>() }), Profile::Own));
    roundtrip(&build_op(Operation::Core(CoreOp::ObjNew { declaration: crate::contract_tests::sample::<DeclId>(), type_args: crate::contract_tests::sample::<Vec<TypeId>>(), fields: Vec::<ValueId>::new(), packet: crate::contract_tests::sample::<Option<ValueId>>(), construction: crate::contract_tests::sample::<ConstructionStatus>() }), Profile::Own));
    roundtrip(&build_op(Operation::Core(CoreOp::ObjNew { declaration: crate::contract_tests::sample::<DeclId>(), type_args: crate::contract_tests::sample::<Vec<TypeId>>(), fields: vec![cs::<ValueId>(); 1], packet: crate::contract_tests::sample::<Option<ValueId>>(), construction: crate::contract_tests::sample::<ConstructionStatus>() }), Profile::Own));
    roundtrip(&build_op(Operation::Core(CoreOp::ObjNew { declaration: crate::contract_tests::sample::<DeclId>(), type_args: crate::contract_tests::sample::<Vec<TypeId>>(), fields: vec![cs::<ValueId>(); 3], packet: crate::contract_tests::sample::<Option<ValueId>>(), construction: crate::contract_tests::sample::<ConstructionStatus>() }), Profile::Own));
    roundtrip(&build_op(Operation::Core(CoreOp::ObjNext { object: crate::contract_tests::sample::<ValueId>(), updates: Vec::<FieldValue>::new(), packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Core(CoreOp::ObjNext { object: crate::contract_tests::sample::<ValueId>(), updates: vec![cs::<FieldValue>(); 1], packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Core(CoreOp::ObjNext { object: crate::contract_tests::sample::<ValueId>(), updates: vec![cs::<FieldValue>(); 3], packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Core(CoreOp::VariantNew { ty: crate::contract_tests::sample::<TypeId>(), case: crate::contract_tests::sample::<u32>(), fields: Vec::<ValueId>::new(), packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Core(CoreOp::VariantNew { ty: crate::contract_tests::sample::<TypeId>(), case: crate::contract_tests::sample::<u32>(), fields: vec![cs::<ValueId>(); 1], packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Core(CoreOp::VariantNew { ty: crate::contract_tests::sample::<TypeId>(), case: crate::contract_tests::sample::<u32>(), fields: vec![cs::<ValueId>(); 3], packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Core(CoreOp::FnClosure { function: crate::contract_tests::sample::<FuncId>(), captures: Vec::<ValueId>::new(), ty: crate::contract_tests::sample::<TypeId>(), packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Core(CoreOp::FnClosure { function: crate::contract_tests::sample::<FuncId>(), captures: vec![cs::<ValueId>(); 1], ty: crate::contract_tests::sample::<TypeId>(), packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Core(CoreOp::FnClosure { function: crate::contract_tests::sample::<FuncId>(), captures: vec![cs::<ValueId>(); 3], ty: crate::contract_tests::sample::<TypeId>(), packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Collection(CollectionOp::ArrayNew { values: Vec::<ValueId>::new(), ty: crate::contract_tests::sample::<TypeId>(), packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Collection(CollectionOp::ArrayNew { values: vec![cs::<ValueId>(); 1], ty: crate::contract_tests::sample::<TypeId>(), packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Collection(CollectionOp::ArrayNew { values: vec![cs::<ValueId>(); 3], ty: crate::contract_tests::sample::<TypeId>(), packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Collection(CollectionOp::ArrayConcat { arrays: Vec::<ValueId>::new(), ty: crate::contract_tests::sample::<TypeId>(), packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Collection(CollectionOp::ArrayConcat { arrays: vec![cs::<ValueId>(); 1], ty: crate::contract_tests::sample::<TypeId>(), packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Collection(CollectionOp::ArrayConcat { arrays: vec![cs::<ValueId>(); 3], ty: crate::contract_tests::sample::<TypeId>(), packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Collection(CollectionOp::ArrayDifference { array: crate::contract_tests::sample::<ValueId>(), removed: Vec::<ValueId>::new(), packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Collection(CollectionOp::ArrayDifference { array: crate::contract_tests::sample::<ValueId>(), removed: vec![cs::<ValueId>(); 1], packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Collection(CollectionOp::ArrayDifference { array: crate::contract_tests::sample::<ValueId>(), removed: vec![cs::<ValueId>(); 3], packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Collection(CollectionOp::MapNew { entries: Vec::<(ValueId, ValueId)>::new(), ty: crate::contract_tests::sample::<TypeId>(), packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Collection(CollectionOp::MapNew { entries: vec![cs::<(ValueId, ValueId)>(); 1], ty: crate::contract_tests::sample::<TypeId>(), packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Collection(CollectionOp::MapNew { entries: vec![cs::<(ValueId, ValueId)>(); 3], ty: crate::contract_tests::sample::<TypeId>(), packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Collection(CollectionOp::MapMerge { maps: Vec::<ValueId>::new(), ty: crate::contract_tests::sample::<TypeId>(), packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Collection(CollectionOp::MapMerge { maps: vec![cs::<ValueId>(); 1], ty: crate::contract_tests::sample::<TypeId>(), packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Collection(CollectionOp::MapMerge { maps: vec![cs::<ValueId>(); 3], ty: crate::contract_tests::sample::<TypeId>(), packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Collection(CollectionOp::MapDifference { map: crate::contract_tests::sample::<ValueId>(), removed: Vec::<ValueId>::new(), packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Collection(CollectionOp::MapDifference { map: crate::contract_tests::sample::<ValueId>(), removed: vec![cs::<ValueId>(); 1], packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Collection(CollectionOp::MapDifference { map: crate::contract_tests::sample::<ValueId>(), removed: vec![cs::<ValueId>(); 3], packet: crate::contract_tests::sample::<Option<ValueId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Ownership(OwnershipOp::Join { packets: Vec::<ValueId>::new() }), Profile::Own));
    roundtrip(&build_op(Operation::Ownership(OwnershipOp::Join { packets: vec![cs::<ValueId>(); 1] }), Profile::Own));
    roundtrip(&build_op(Operation::Ownership(OwnershipOp::Join { packets: vec![cs::<ValueId>(); 3] }), Profile::Own));
    roundtrip(&build_op(Operation::Async(AsyncOp::FrameSave { frame: crate::contract_tests::sample::<ValueId>(), slots: Vec::<FrameSlot>::new(), continuation: crate::contract_tests::sample::<u32>() }), Profile::Own));
    roundtrip(&build_op(Operation::Async(AsyncOp::FrameSave { frame: crate::contract_tests::sample::<ValueId>(), slots: vec![cs::<FrameSlot>(); 1], continuation: crate::contract_tests::sample::<u32>() }), Profile::Own));
    roundtrip(&build_op(Operation::Async(AsyncOp::FrameSave { frame: crate::contract_tests::sample::<ValueId>(), slots: vec![cs::<FrameSlot>(); 3], continuation: crate::contract_tests::sample::<u32>() }), Profile::Own));
    roundtrip(&build_op(Operation::Async(AsyncOp::FrameResume { frame: crate::contract_tests::sample::<ValueId>(), slots: Vec::<SavedSlot>::new() }), Profile::Own));
    roundtrip(&build_op(Operation::Async(AsyncOp::FrameResume { frame: crate::contract_tests::sample::<ValueId>(), slots: vec![cs::<SavedSlot>(); 1] }), Profile::Own));
    roundtrip(&build_op(Operation::Async(AsyncOp::FrameResume { frame: crate::contract_tests::sample::<ValueId>(), slots: vec![cs::<SavedSlot>(); 3] }), Profile::Own));
    roundtrip(&build_op(Operation::Async(AsyncOp::WorkerBind { action: crate::contract_tests::sample::<ValueId>(), entry: crate::contract_tests::sample::<ValueId>(), fields: Vec::<ValueId>::new(), ty: crate::contract_tests::sample::<TypeId>() }), Profile::Own));
    roundtrip(&build_op(Operation::Async(AsyncOp::WorkerBind { action: crate::contract_tests::sample::<ValueId>(), entry: crate::contract_tests::sample::<ValueId>(), fields: vec![cs::<ValueId>(); 1], ty: crate::contract_tests::sample::<TypeId>() }), Profile::Own));
    roundtrip(&build_op(Operation::Async(AsyncOp::WorkerBind { action: crate::contract_tests::sample::<ValueId>(), entry: crate::contract_tests::sample::<ValueId>(), fields: vec![cs::<ValueId>(); 3], ty: crate::contract_tests::sample::<TypeId>() }), Profile::Own));
    roundtrip(&build_op(Operation::Async(AsyncOp::WorkerSpawn { entry: crate::contract_tests::sample::<ValueId>(), args: Vec::<ValueId>::new(), direct: crate::contract_tests::sample::<bool>(), result: crate::contract_tests::sample::<Option<TypeId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Async(AsyncOp::WorkerSpawn { entry: crate::contract_tests::sample::<ValueId>(), args: vec![cs::<ValueId>(); 1], direct: crate::contract_tests::sample::<bool>(), result: crate::contract_tests::sample::<Option<TypeId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Async(AsyncOp::WorkerSpawn { entry: crate::contract_tests::sample::<ValueId>(), args: vec![cs::<ValueId>(); 3], direct: crate::contract_tests::sample::<bool>(), result: crate::contract_tests::sample::<Option<TypeId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Async(AsyncOp::ProcessSpawn { constructor: crate::contract_tests::sample::<FuncId>(), constructor_args: Vec::<ValueId>::new(), action: crate::contract_tests::sample::<FuncId>(), args: crate::contract_tests::sample::<Vec<ValueId>>(), result: crate::contract_tests::sample::<Option<TypeId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Async(AsyncOp::ProcessSpawn { constructor: crate::contract_tests::sample::<FuncId>(), constructor_args: vec![cs::<ValueId>(); 1], action: crate::contract_tests::sample::<FuncId>(), args: crate::contract_tests::sample::<Vec<ValueId>>(), result: crate::contract_tests::sample::<Option<TypeId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Async(AsyncOp::ProcessSpawn { constructor: crate::contract_tests::sample::<FuncId>(), constructor_args: vec![cs::<ValueId>(); 3], action: crate::contract_tests::sample::<FuncId>(), args: crate::contract_tests::sample::<Vec<ValueId>>(), result: crate::contract_tests::sample::<Option<TypeId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Async(AsyncOp::ProcessSpawn { constructor: crate::contract_tests::sample::<FuncId>(), constructor_args: crate::contract_tests::sample::<Vec<ValueId>>(), action: crate::contract_tests::sample::<FuncId>(), args: Vec::<ValueId>::new(), result: crate::contract_tests::sample::<Option<TypeId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Async(AsyncOp::ProcessSpawn { constructor: crate::contract_tests::sample::<FuncId>(), constructor_args: crate::contract_tests::sample::<Vec<ValueId>>(), action: crate::contract_tests::sample::<FuncId>(), args: vec![cs::<ValueId>(); 1], result: crate::contract_tests::sample::<Option<TypeId>>() }), Profile::Own));
    roundtrip(&build_op(Operation::Async(AsyncOp::ProcessSpawn { constructor: crate::contract_tests::sample::<FuncId>(), constructor_args: crate::contract_tests::sample::<Vec<ValueId>>(), action: crate::contract_tests::sample::<FuncId>(), args: vec![cs::<ValueId>(); 3], result: crate::contract_tests::sample::<Option<TypeId>>() }), Profile::Own));
}

#[test]
fn call_argument_lists_and_proof_evidence_preserve_order_and_holder_details() {
    for n in [0,1,3] {
        let v = ValueId::from_u32(0);
        let call = CallSite { cleanup: CallCleanupMode::Ordinary,  args: vec![CallArgument { value: v,
            convention: OperandConvention::Borrow }; n],
            packet: Some(v), deps: vec![v; n], descriptors: vec![v; n], type_args: vec![TypeId::from_u32(0); n], summary: Some(SummaryId::from_u32(0)) };
        roundtrip(&build_op_config(Operation::Call(CallOp::Direct { function: FuncId::from_u32(0), call }), Profile::Own, |b| {
            let summary = b.raw_mut().summaries.get_mut(SummaryId::from_u32(0)).unwrap();
            summary.parameters = vec![ParameterEffect { convention: FreshConvention::Borrow, retention: None, deps: vec![], destination: TransferDestination::None }; n];
        }));
    }
    let proof = UniqueCacheProof { future: ValueId::from_u32(0), scope: ScopeId::from_u32(0), region: vec![BlockId::from_u32(0)],
        evidence: UniqueCacheEvidence { creation: CreationOrigin::Instruction(InstId::from_u32(0)), aliases: vec![ValueId::from_u32(0); 3], frame_slots: vec![7,4],
            no_external_storage: true, no_other_waiters: false, no_live_views: true, no_collector_retention: false, scheduler_cannot_publish: true } };
    let printed = roundtrip(&build_op(Operation::Async(AsyncOp::UniqueCacheProof { proof }), Profile::Coro));
    assert!(printed.contains("frame_slots=[7, 4]"));
    let renamed = printed.replace("i0", "iorigin").replace("%0", "%input").replace("^b0", "^entry");
    assert!(text::normalized_eq(&text::parse(&printed).unwrap(), &text::parse(&renamed).unwrap()).unwrap());
}

#[test]
fn annotation_forward_names_duplicates_and_cross_function_ssa_names() {
    let mut module = build_op(Operation::Ownership(OwnershipOp::SplitChecked { packet: ValueId::from_u32(0), selection: SelectionId::from_u32(0) }), Profile::Own);
    *module.functions.get_mut(FuncId::from_u32(0)).unwrap().body.as_mut().unwrap()
        .selections.get_mut(SelectionId::from_u32(0)).unwrap() = SelectionExpr::Union(vec![]);
    let text = roundtrip(&module);
    let selection = text.lines().find(|line| line.trim_start().starts_with("selection ")).unwrap();
    let mut forward = text.replace(&format!("{selection}\n"), "");
    let end = forward.rfind("}\n").unwrap(); forward.insert_str(end, &format!("{selection}\n"));
    assert_eq!(text::print(&text::parse(&forward).unwrap()).unwrap(), text);
    assert_eq!(text::parse(&text.replace(", s0", ", s999")).unwrap_err().kind, K::UnknownName);
    let duplicated = text.replace(selection, &format!("{selection}\n{selection}"));
    assert_eq!(text::parse(&duplicated).unwrap_err().kind, K::DuplicateName);
    let mut duplicate_value = text::parse(SMALL).unwrap();
    let func = duplicate_value.functions.get_mut(FuncId::from_u32(0)).unwrap();
    func.signature.params.push(TypeId::from_u32(0));
    let body = func.body.as_mut().unwrap();
    let another = body.values.alloc(ValueData { ty: TypeId::from_u32(0), def: Definition::BlockParam { block: BlockId::from_u32(0), index: 0 }, facts: ValueFacts::Service }).unwrap();
    body.blocks.get_mut(BlockId::from_u32(0)).unwrap().params.push(another);
    let text = text::print(&duplicate_value).unwrap();
    assert_eq!(text::parse(&text.replace("%1", "%0")).unwrap_err().kind, K::DuplicateName);
    let mut second = module.functions.get(FuncId::from_u32(0)).unwrap().clone(); second.symbol = "other".into();
    module.functions.alloc(second).unwrap(); roundtrip(&module); // identical local names in different functions
    assert_eq!(text::parse_with_limits(SMALL, Limits { max_tokens: 3, ..Limits::default() }).unwrap_err().kind, K::Limit);
    assert_eq!(text::parse_with_limits("oir 1 high\n{", Limits { max_text_bytes: 0, ..Limits::default() }).unwrap_err().kind, K::UnsupportedVersion);
}

#[test]
fn specification_own_fragments_parse_and_roundtrip() {
    // Complete instruction lines are copied verbatim from OIR-2 §16.
    // Only schematic ellipses are replaced; surrounding declarations supply context.
    for source in [
        include_str!("../tests/fixtures/text/spec-16-1.oir"),
        include_str!("../tests/fixtures/text/spec-16-3.oir"),
        include_str!("../tests/fixtures/text/spec-16-4.oir"),
        include_str!("../tests/fixtures/text/spec-16-5.oir"),
    ] {
        let module = text::parse(source).unwrap_or_else(|e| panic!("{e}\n{source}"));
        roundtrip(&module);
        let crlf = source.replace('\n', "\r\n");
        assert!(text::normalized_eq(&module, &text::parse(&crlf).unwrap()).unwrap());
    }
}

#[test]
fn compact_declarations_generics_and_nondefault_metadata_roundtrip() {
    let source = r#"oir 2 high
module declarations v2.3.4
requires core, test
import std/core @1 as core
import "native/lib" as lib take A, B
file 7 "source.ok"
entry @main
test "smoke" = @main
native object Native
enum Status = (ready, done)
variant Choice is Error = (item: Box)
role Read<T> [builtin] {
    read(T) -> (int)
}
object Box<T: Read<int>> {
    field item: T = @default once
    does Read<int> { read = @read }
    destroy @destroy
}
cap Box<int> :: View {
    field item = @read
}
global g0: int = @default
scope S0 action in @main
scope S1 block in @main parent S0 !7:1:1
summary M0 internal contract call_contract(results=[int], publication=atomic)
protocol opaque
extern @default() -> int = "native.default"
func @read() -> int [method Box] summary(M0);
func @destroy() -> void [destroy Box];
func @main<T>(%a: T; %p: own) -> int [entry, default Box.item, lambda captures 1 owner Box] {
    boundary B0 lexical in S0
    binding b0 "item": T in S0 !7:2:1
    key k0: key = runtime(%a)
    dependency d0: dependency = key(k0)
    selection s0: selection = union([selection(%a)])
    ^entry in S1:
    %status: Status = const Status.ready
    %selected: own = own.move %p facts(retention=[s0])
    %result: int = const 42
    ret %result
}
"#;
    let module = text::parse(source).unwrap_or_else(|e| panic!("{e}\n{source}"));
    let canonical = roundtrip(&module);
    assert!(canonical.contains("extern @default() -> int = \"native.default\""));
    assert!(canonical.contains("const Status.ready"));
    assert!(canonical.contains("union([selection(%0)])"));
    assert!(!canonical.contains("type t"));
    assert!(!canonical.contains("abi unelaborated"));
}

#[test]
fn unused_types_inline_selections_and_positional_operands_normalize() {
    let mut module = text::parse(SMALL).unwrap();
    let before = module.clone();
    let extra = module.types.intern(Type::Bool).unwrap();
    module.types.intern(Type::Array(extra)).unwrap();
    assert!(text::normalized_eq(&module, &before).unwrap());
    let op = Operation::Core(CoreOp::IntAdd { left: ValueId::from_u32(0), right: ValueId::from_u32(0) });
    let source = roundtrip(&build_op(op, Profile::High));
    assert!(source.contains("int.add %0, %0"));
    assert!(!source.contains("int.add("));
    assert!(!source.lines().any(|line| line.trim_start().starts_with("[i")));
    assert!(!source.contains("facts(result_deps=[],"));
    assert!(!source.contains("    selection "));
}

#[test]
fn prefix_types_and_shared_type_expansion_obey_limits() {
    let prefix = format!("oir 2 high\nmodule bounded\nfunc @f(%x: {}int) -> void;\n", "^".repeat(10000));
    assert_eq!(text::parse(&prefix).unwrap_err().kind, K::Limit);
    let mut module = text::parse(SMALL).unwrap();
    let mut ty = TypeId::from_u32(0);
    for _ in 0..40 { ty = module.types.intern(Type::Map(ty, ty)).unwrap(); }
    module.functions.get_mut(FuncId::from_u32(0)).unwrap().signature.results = vec![ty];
    assert!(text::print(&module).is_err());
}

#[test]
fn exits_keep_every_optional_operand_combination() {
    let value = ValueId::from_u32(0);
    for present_value in [None, Some(value)] {
        for packet in [None, Some(value)] {
            for deps in [None, Some(value)] {
                let op = Operation::Exit(ExitOp::Prepare { exit: PreparedExit {
                    kind: ExitKind::Return, value: present_value, packet, deps,
                    selection: SelectionId::from_u32(0), boundary: BoundaryId::from_u32(0),
                } });
                roundtrip(&build_op(op, Profile::Own));
            }
        }
    }
}

#[test]
fn profile_defaults_and_literal_spelling_are_canonical() {
    let high = "oir 2 high\nmodule answer\nscope S0 action in @main\n\nfunc @main() -> int [entry] {\n    ^b0 in S0:\n    %0: int = const 42\n    ret %0\n}\n";
    let own = concat!(
        "oir 2 own\nmodule empty\nscope S0 action in @main\n\nfunc @main() -> void {\n    ^b0 in S0:",
        "\n    %0: scope<S0> = scope.begin S0\n    %1: own = own.empty\n    %2: scope<S0> = own.admit",
        " %0, %1\n    scope.end %2, effects scope_effects(failures=[thrown, program, panic, native], ",
        "suspends=true, cancel=false)\n    ret\n}\n",
    );
    for source in [high, own] {
        assert_eq!(roundtrip(&text::parse(source).unwrap()), source);
    }
    for literal in [Literal::Float(0), Literal::Float(0x8000_0000_0000_0000), Literal::Float(1)] {
        let expected = if let Literal::Float(bits) = literal { format!("const {:?}", f64::from_bits(bits)) }
            else { unreachable!() };
        assert!(roundtrip(&build_op(Operation::Const(literal), Profile::High)).contains(&expected));
    }
    let mut module = text::parse(high).unwrap();
    let byte = module.types.intern(Type::Byte).unwrap();
    module.functions.get_mut(FuncId::from_u32(0)).unwrap().body.as_mut().unwrap()
        .values.get_mut(ValueId::from_u32(0)).unwrap().ty = byte;
    // A deliberately wrong result annotation must not change the literal kind.
    assert!(roundtrip(&module).contains("const int(42)"));
}

#[test]
fn ambiguous_nominal_types_are_diagnostic_instead_of_changing_variant() {
    let (mut builder, _, _) = setup(Profile::High, Some(Type::Role {
        decl: DeclId::from_u32(0), args: vec![],
    }));
    assert!(text::print(builder.raw_mut()).is_err());
    builder.raw_mut().declarations.get_mut(DeclId::from_u32(0)).unwrap().kind = DeclarationKind::Role {
        methods: vec![], builtin: false,
    };
    roundtrip(builder.raw_mut());
    builder.raw_mut().declarations.get_mut(DeclId::from_u32(0)).unwrap().name = "own".into();
    assert!(text::print(builder.raw_mut()).is_err());
}

#[test]
fn implicit_call_summaries_resolve_after_all_forward_declarations() {
    let source = r#"oir 2 own
module forward
scope S action in @main
func @callee() -> (int, int, int) summary(M1);
func @main() -> void {
^entry:
    invoke.own call @callee()
        normal ^ok($slot0, $slot1, $slot2)
        unwind ^bad($failure)
^ok(%a: int, %b: int, %c: int):
    ret
^bad(%f: failure):
    raise %f
}
summary M0 call-boundary contract call_contract(results=[int], failures=[thrown], publication=atomic)
summary M1 call-boundary contract call_contract(results=[int, int, int], failures=[thrown], publication=atomic)
"#;
    roundtrip(&text::parse(source).unwrap());
}

#[test]
fn field_names_on_forward_ssa_uses_remain_nonvalidating() {
    let source = r#"oir 2 high
module forward_field
native object Other
object Object {
    field x: int
}
scope S action in @main
func @main() -> void {
^entry:
    %result: int = obj.get %later, x
    %later: Object = undef ty Object
    ret
}
"#;
    // The SSA use is intentionally non-dominating; #564 diagnoses it later.
    roundtrip(&text::parse(source).unwrap());
}

#[test]
fn creation_origins_and_reserve_schemas_roundtrip() {
    let origins = [CreationOrigin::Instruction(cs()), CreationOrigin::Invoke { block: cs(), outcome: Outcome::Normal, slot: 2 }, CreationOrigin::Invoke { block: cs(), outcome: Outcome::Unwind, slot: 1 }];
    assert_eq!(CreationOrigin::TEXT_VARIANTS.len(), 2);
    for creation in origins {
        crate::text::payload_roundtrip(creation);
        let proof = UniqueCacheProof { evidence: UniqueCacheEvidence { creation, ..crate::contract_tests::sample::<UniqueCacheProof>().evidence }, ..crate::contract_tests::sample() };
        roundtrip(&build_op(Operation::Async(AsyncOp::UniqueCacheProof { proof }), Profile::Coro));
    }
    for reserve in [None, Some(ValueId::from_u32(0))] {
        roundtrip(&build_op(Operation::Ownership(OwnershipOp::ScopeEnd { state: cs(), reserve,
            effects: ScopeEffectEnvelope::conservative() }), Profile::Own));
        roundtrip(&build_op(Operation::Ownership(OwnershipOp::ScopeUnwind { state: cs(), failure: cs(), reserve,
            effects: ScopeEffectEnvelope::conservative() }), Profile::Own));
    }
}

#[test]
fn missing_summaries_and_explicit_none_are_preserved_without_arena_defaults() {
    for declared in [None, Some(SummaryId::from_u32(0))] {
        for call_summary in [None, Some(SummaryId::from_u32(0)), Some(SummaryId::from_u32(1))] {
            let mut m = build_op(Operation::Const(Literal::Int("0".into())), Profile::High);
            let summary = m.summaries.get(SummaryId::from_u32(0)).unwrap().clone();
            m.summaries.alloc(summary).unwrap();
            m.functions.get_mut(FuncId::from_u32(0)).unwrap().summary = declared;
            let body = m.functions.get_mut(FuncId::from_u32(0)).unwrap().body.as_mut().unwrap();
            body.insts.get_mut(InstId::from_u32(1)).unwrap().op = Operation::Call(CallOp::Direct { function: cs(),
                call: CallSite { cleanup: CallCleanupMode::Ordinary,  summary: call_summary, args: vec![],
                packet: None, deps: vec![], descriptors: vec![], type_args: vec![] } });
            let printed = roundtrip(&m);
            let call = printed.lines().find(|line| line.contains("= call ")).unwrap();
            assert_eq!(call.contains("summary("), declared != call_summary);
            if call_summary.is_none() && declared.is_some() { assert!(call.contains("summary(none)")); }
        }
    }
    let source = "oir 2 high\nmodule absent\nscope S action in @f\nfunc @f(%fn: fn() -> int) -> int {\n^entry:\n%v: int = fn.call %fn()\nret %v\n}\n";
    let m = text::parse(source).unwrap();
    let printed = roundtrip(&m);
    assert!(!printed.contains("summary("));
    assert!(matches!(&m.functions.get(cs()).unwrap().body.as_ref().unwrap().insts.get(cs()).unwrap().op, Operation::Call(CallOp::Closure { call: CallSite { summary: None, .. }, .. })));
}

#[test]
fn field_and_exit_context_never_default_to_first_arena_entity() {
    let explicit = "oir 2 high\nmodule fields\nobject First {\nfield x: int\n}\nscope S action in @f\nfunc @f(%x: int) -> void {\nboundary Bx lexical in S\n^entry:\n%v: int = obj.get %x, field_ref(declaration=First, index=0)\n%p: pending = exit.prepare return %v at Bx\nret\n}\n";
    let m = text::parse(explicit).unwrap();
    let printed = roundtrip(&m);
    assert!(printed.contains("field_ref(declaration=First"));
    assert!(printed.contains(" at B0"));
    assert_eq!(text::parse(&explicit.replace("field_ref(declaration=First, index=0)", "x")).unwrap_err().kind, K::Syntax);
    assert_eq!(text::parse(&explicit.replace(" at Bx", "")).unwrap_err().kind, K::Syntax);
}

fn coverage_modules() -> Vec<Module> {
    let mut modules: Vec<_> = crate::contract_tests::representatives().into_iter().map(|op| build_op(op, Profile::Own)).collect();
    let v = ValueId::from_u32(0);
    for reserve in [None, Some(v)] {
        modules.push(build_op(Operation::Ownership(OwnershipOp::ScopeEnd { state: v, reserve,
            effects: ScopeEffectEnvelope::conservative() }), Profile::Own));
        modules.push(build_op(Operation::Ownership(OwnershipOp::ScopeUnwind { state: v, failure: v, reserve,
            effects: ScopeEffectEnvelope::conservative() }), Profile::Own));
    }
    for result in [None, Some(TypeId::from_u32(0))] {
        modules.push(build_op(Operation::Async(AsyncOp::Await { future: v, result }), Profile::Coro));
        for boundary in [TakeBoundary::Existing { owner: cs() }, TakeBoundary::Temporary { awaiting: cs() }] {
            modules.push(build_op(Operation::Async(AsyncOp::AwaitTake { future: v, result, unique: v, boundary }), Profile::Coro));
        }
    }
    for packet in [None, Some(v)] {
        for op in [CollectionOp::BuilderFinish { buffer: v, packet }, CollectionOp::BuilderAbandon { buffer: v, packet }, CollectionOp::BuilderPush { buffer: v, value: v, packet }] {
            modules.push(build_op(Operation::Collection(op), Profile::Own));
        }
        modules.push(build_op(Operation::Core(CoreOp::ObjNew { declaration: cs(), type_args: vec![], fields: vec![v], packet, construction: ConstructionStatus::Incomplete }), Profile::Own));
        for results in [0, 1, 3] {
            modules.push(build_op_config(Operation::Call(CallOp::Direct { function: cs(), call: CallSite { packet, ..crate::contract_tests::sample() } }), Profile::Own, |b| {
                b.raw_mut().summaries.get_mut(SummaryId::from_u32(0)).unwrap().contract.results = vec![TypeId::from_u32(0); results];
            }));
        }
    }
    modules.push(build_op(Operation::Ownership(OwnershipOp::ScopeTransfer { states: TransferStates::Between { from: v, to: v }, selection: cs() }), Profile::Own));
    modules.push(build_op(Operation::Carrier(CarrierOp::Transfer { state: v, target: CarrierDestination::FailureContext(cs()), protocol: cs() }), Profile::Own));
    for n in [0, 1, 3] {
        modules.push(build_op(Operation::Async(AsyncOp::FrameResume { frame: v, slots: vec![SavedSlot { slot: 7, ty: cs(), linear: true }; n] }), Profile::Coro));
    }
    for publication in [Publication::ChannelReceipt, Publication::TaskState] {
        modules.push(build_op_config(Operation::Call(CallOp::Direct { function: cs(), call: crate::contract_tests::sample() }), Profile::Own, |b| {
            let contract = &mut b.raw_mut().summaries.get_mut(SummaryId::from_u32(0)).unwrap().contract;
            contract.publication = publication; contract.protocol = Some(ProtocolId::from_u32(0));
        }));
    }
    modules
}

#[test]
fn coverage_inventory_requires_opcode_fixtures_and_every_schema_slot_set() {
    use std::collections::BTreeSet;
    let expected: BTreeSet<_> = include_str!("../tests/fixtures/opcodes.txt").lines().collect();
    assert_eq!(expected, Operation::opcode_inventory().into_iter().collect(), "opcode fixture inventory must be reviewed");
    let mut seen = BTreeSet::new(); let mut shapes = BTreeSet::new();
    let mut modules = coverage_modules();
    modules.push(text::parse(include_str!("../tests/fixtures/text/d12-deferred-future.oir")).unwrap());
    let frame_op = crate::contract_tests::representatives().into_iter().find(|op| {
        matches!(op, Operation::Async(AsyncOp::FutureNew { .. }))
    }).unwrap();
    modules.push(build_op(frame_op, Profile::Coro));
    for module in &modules {
        roundtrip(module);
        let registry = ContractRegistry::new(module);
        for (_, function) in module.functions.iter() {
            let Some(body) = &function.body else { continue };
            for (_, block) in body.blocks.iter() {
                for &id in &block.insts { seen.insert(body.insts.get(id).unwrap().op.opcode()); }
                if let TermKind::InvokeOwn { op, normal, unwind } = &block.term.kind {
                    seen.insert(op.opcode());
                    let contract = registry.lookup(op).unwrap();
                    for (schema, edge) in [(&contract.normal, normal), (contract.unwind.as_ref().unwrap(), unwind)] {
                        // This fixture forwards every produced slot, in order.
                        assert_eq!(edge.args.len(), schema.results.len());
                        for (index, arg) in edge.args.iter().enumerate() { assert!(matches!(arg, EdgeArg::Produced(r) if usize::from(r.slot) == index)); }
                    }
                }
            }
            let ops = body.insts.iter().map(|(_, i)| &i.op).chain(body.blocks.iter().filter_map(|(_, b)| if let TermKind::InvokeOwn { op, .. } = &b.term.kind { Some(op.as_ref()) } else { None }));
            for op in ops {
                let c = registry.lookup(op).unwrap();
                for schema in [Some(&c.normal), c.unwind.as_ref(), c.cancellation.as_ref().map(|c| &c.outcome)].into_iter().flatten() {
                    shapes.insert(schema.results.iter().map(|s| crate::text::slot_name(s.name)).collect::<Vec<_>>().join(","));
                }
            }
        }
    }
    assert_eq!(seen, expected);
    let expected_shapes: BTreeSet<String> = include_str!("../tests/fixtures/outcomes.txt").lines().map(|s| if s == "void" { String::new() } else { s.into() }).collect();
    assert_eq!(shapes, expected_shapes, "outcome slot-set inventory must be reviewed");
}

#[test]
fn fixture_annotation_profile_and_dynamic_reason_inventory_is_exhaustive() {
    use std::collections::{BTreeMap, BTreeSet};
    let mut modules = coverage_modules();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for dir in ["spec", "invalid"] {
        for entry in std::fs::read_dir(root.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "oir") { modules.push(text::parse(&std::fs::read_to_string(&path).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", path.display()))); }
        }
    }
    let mut seen: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    macro_rules! mark { ($group:literal, $value:expr) => { seen.entry($group).or_default().insert($value); }; }
    for module in &modules {
        mark!("profile", match module.profile { Profile::High => "high", Profile::Own => "own", Profile::Coro => "coro", Profile::Low => "low" });
        for (_, scope) in module.scopes.iter() { mark!("scope", match scope.kind { ScopeKind::Action => "action", ScopeKind::Block => "block", ScopeKind::Branch => "branch", ScopeKind::Iteration => "iteration", ScopeKind::Try => "try", ScopeKind::Catch => "catch", ScopeKind::Defer => "defer" }); }
        for (_, function) in module.functions.iter() {
            let Some(body) = &function.body else { continue };
            for (_, value) in body.values.iter() {
                mark!("value_facts", match &value.facts { ValueFacts::Unelaborated => "unelaborated", ValueFacts::Service => "service", ValueFacts::Managed { .. } => "managed", ValueFacts::Plain { .. } => "plain" });
                if let ValueFacts::Plain { certificate } = &value.facts { mark!("plain", match certificate { PlainCertificate::Scalar => "scalar", PlainCertificate::ClosedGraph { .. } => "closed_graph", PlainCertificate::Descriptor(_) => "descriptor" }); }
            }
            for (_, boundary) in body.boundaries.iter() { mark!("boundary", match boundary.kind { BoundaryKind::Lexical => "lexical", BoundaryKind::Call => "call", BoundaryKind::Loop => "loop", BoundaryKind::AbortBeforeDestroy => "abort_before_destroy", BoundaryKind::AbortAfterPrefix => "abort_after_prefix" }); }
            for (_, key) in body.keys.iter() {
                mark!("key", match key { KeyExpr::Created { .. } => "created", KeyExpr::Runtime(_) => "runtime", KeyExpr::ServiceOf { .. } => "service_of" });
                if let KeyExpr::ServiceOf { kind, .. } = key { mark!("service_key", match kind { ServiceKeyKind::Cache => "cache", ServiceKeyKind::Frame => "frame", ServiceKeyKind::Channel => "channel", ServiceKeyKind::Carrier => "carrier", ServiceKeyKind::Pin => "pin", ServiceKeyKind::Group => "group" }); }
            }
            for (_, dep) in body.deps.iter() { mark!("dependency", match dep { DepExpr::Empty => "empty", DepExpr::Key(_) => "key", DepExpr::Union(_) => "union", DepExpr::FromSsa(_) => "from_ssa", DepExpr::Retention { .. } => "retention" }); }
            for (_, selection) in body.selections.iter() {
                mark!("selection", match selection { SelectionExpr::Value(_) => "value", SelectionExpr::Admissible(_) => "admissible", SelectionExpr::Projection(_) => "projection", SelectionExpr::OwnerGraph(_) => "owner_graph", SelectionExpr::ViewOwner(_) => "view_owner", SelectionExpr::Union(_) => "union", SelectionExpr::Descriptor { .. } => "descriptor" });
                if let SelectionExpr::Projection(p) = selection { mark!("projection", match p { Projection::Field { .. } => "field", Projection::Index { .. } => "index", Projection::Slice { .. } => "slice", Projection::Map { .. } => "map" }); }
            }
            for (_, inst) in body.insts.iter() {
                if !inst.facts.result_deps.is_empty() { mark!("instruction_facts", "result_deps"); }
                if !inst.facts.retention.is_empty() { mark!("instruction_facts", "retention"); }
                if !inst.facts.provenance.is_empty() { mark!("instruction_facts", "provenance"); }
                for fact in &inst.facts.provenance { mark!("provenance", match fact.state { AdmissionProvenance::UnboundResult => "unbound_result", AdmissionProvenance::Bound => "bound", AdmissionProvenance::StagedOrCarrierOwned => "staged_or_carrier_owned", AdmissionProvenance::CacheOwned => "cache_owned", AdmissionProvenance::ExtractedAtExistingBoundary => "extracted_at_existing_boundary", AdmissionProvenance::TemporaryAwaitResult => "temporary_await_result" }); }
            }
            for op in body.insts.iter().map(|(_, i)| &i.op).chain(body.blocks.iter().filter_map(|(_, b)| if let TermKind::InvokeOwn { op, .. } = &b.term.kind { Some(op.as_ref()) } else { None })) {
                if let Operation::Ownership(OwnershipOp::Dynamic { reason, .. }) = op { mark!("reason", match reason { DynamicReason::Atomic => "atomic", DynamicReason::Cell => "cell", DynamicReason::Channel => "channel", DynamicReason::Group => "group", DynamicReason::Any => "any", DynamicReason::Closure => "closure", DynamicReason::Merge => "merge" }); }
                if let Operation::Async(AsyncOp::UniqueCacheProof { proof }) = op { mark!("creation", match proof.evidence.creation { CreationOrigin::Instruction(_) => "instruction", CreationOrigin::Invoke { .. } => "invoke" }); }
            }
        }
    }
    let groups = [
        ("profile", &["high", "own", "coro", "low"][..]), ("reason", DynamicReason::TEXT_VARIANTS),
        ("scope", ScopeKind::TEXT_VARIANTS), ("boundary", BoundaryKind::TEXT_VARIANTS),
        ("value_facts", ValueFacts::TEXT_VARIANTS), ("plain", PlainCertificate::TEXT_VARIANTS),
        ("key", KeyExpr::TEXT_VARIANTS), ("service_key", ServiceKeyKind::TEXT_VARIANTS),
        ("dependency", DepExpr::TEXT_VARIANTS), ("selection", SelectionExpr::TEXT_VARIANTS),
        ("projection", Projection::TEXT_VARIANTS), ("provenance", AdmissionProvenance::TEXT_VARIANTS),
        ("creation", CreationOrigin::TEXT_VARIANTS), ("instruction_facts", &["result_deps", "retention", "provenance"][..]),
    ];
    for (name, required) in groups { assert_eq!(seen.get(name).unwrap(), &required.iter().copied().collect(), "missing {name} fixture"); }
}

#[test]
fn block_scope_is_derived_only_from_its_own_function_action_scope() {
    let source = "oir 2 high\nmodule scopes\nscope S0 action in @other\nfunc @other() -> void;\nfunc @f() -> void {\n^entry:\nret\n}\n";
    assert_eq!(text::parse(source).unwrap_err().kind, K::Syntax);
    let explicit = source.replace("^entry:", "^entry in S0:");
    roundtrip(&text::parse(&explicit).unwrap()); // wrong owner belongs to #564
    let own = source.replace("action in @other", "action in @f");
    roundtrip(&text::parse(&own).unwrap());
}

#[test]
fn hand_checked_optional_summary_golden() {
    let golden = include_str!("../tests/fixtures/text/d7-summary-none.oir");
    assert_eq!(roundtrip(&text::parse(golden).unwrap()), golden);
}

#[test]
fn every_call_form_keeps_optional_summary_and_indirect_forms_print_present_summary() {
    let source = r#"oir 2 high
module calls
object Owner {
    field value: int
}
role Read {
    read() -> int
}
cap Owner :: View {
    field value = @computed
}
scope S action in @main
summary M0 call-boundary contract call_contract(results=[int], publication=atomic)
func @callee() -> int summary(M0);
func @computed(%owner: Owner) -> int summary(M0);
func @main(%fn: fn() -> int, %role: Read, %cap: View, %frame: frame<@callee>) -> void {
^entry in S:
    %a: int = call @callee() summary(none)
    %b: int = call.native @callee() summary(none)
    %c: int = call.intrinsic @callee() summary(none)
    %d: int = fn.call %fn()
    %e: int = call.role %role, Read slot 0()
    %f: int = call.cap %cap, field_ref(declaration=View, index=0)()
    %g: int = call.resume %frame, @callee()
    %h: int = fn.call %fn() summary(M0)
    %i: int = call.role %role, Read slot 0() summary(M0)
    %j: int = call.cap %cap, field_ref(declaration=View, index=0)() summary(M0)
    %k: int = call.resume %frame, @callee() summary(M0)
    ret
}
"#;
    let module = text::parse(source).unwrap(); let printed = roundtrip(&module);
    let mut summaries = Vec::new();
    let body = module.functions.iter().find(|(_, f)| f.symbol == "main").unwrap().1.body.as_ref().unwrap();
    for (_, inst) in body.insts.iter() {
        if let Operation::Call(op) = &inst.op {
            let call = match op { CallOp::Direct { call, .. } | CallOp::Native { call, .. } | CallOp::Intrinsic { call, .. } | CallOp::Role { call, .. } | CallOp::Closure { call, .. } | CallOp::Cap { call, .. } | CallOp::Resume { call, .. } => call };
            summaries.push(call.summary);
        }
    }
    assert_eq!(&summaries[..7], &[None; 7]);
    assert_eq!(&summaries[7..], &[Some(SummaryId::from_u32(0)); 4]);
    assert_eq!(printed.lines().filter(|l| l.contains("= ") && l.contains("summary(M0)")).count(), 4);
}

#[test]
fn elaboration_payload_enums_roundtrip() {
    let kinds = [ProtocolKind::Storage, ProtocolKind::Callable, ProtocolKind::Atomic, ProtocolKind::Cell,
        ProtocolKind::Channel, ProtocolKind::Group, ProtocolKind::Any, ProtocolKind::Closure, ProtocolKind::Services];
    assert_eq!(kinds.len(), ProtocolKind::TEXT_VARIANTS.len());
    for kind in kinds { crate::text::payload_roundtrip(kind); }
    let environments = [
        FutureEnvironment::Deferred { function: cs(), arguments: vec![], descriptors: vec![] },
        FutureEnvironment::Frame(cs()),
    ];
    assert_eq!(environments.len(), FutureEnvironment::TEXT_VARIANTS.len());
    for environment in environments { crate::text::payload_roundtrip(environment); }
    let edges = [HighEdgeSelector::Normal(0), HighEdgeSelector::Unwind];
    assert_eq!(edges.len(), HighEdgeSelector::TEXT_VARIANTS.len());
    for edge in edges { crate::text::payload_roundtrip(edge); }
}

#[test]
fn defer_edges_expose_only_their_own_schemas_and_editor_preserves_the_tags() {
    let source = include_str!("../tests/fixtures/verify/valid/D8-tagged-defer.oir");
    let mut module = text::parse(source).unwrap();
    roundtrip(&module);
    let caller = FuncId::from_u32(1);
    let body = module.functions.get(caller).unwrap().body.as_ref().unwrap();
    let mut definitions = vec![];
    cfg::walk_body(body, ContractRegistry::new(&module), |event| {
        if let cfg::CfgEvent::EdgeDefinition { kind, result, schema, .. } = event {
            definitions.push((kind, result.outcome, result.slot, schema.name));
        }
    }).unwrap();
    assert_eq!(definitions, vec![
        (cfg::EdgeKind::DeferReplace(0), Outcome::DeferReplace(0), 0, SlotName::Value),
        (cfg::EdgeKind::DeferReplace(0), Outcome::DeferReplace(0), 1, SlotName::Packet),
        (cfg::EdgeKind::DeferReplace(0), Outcome::DeferReplace(0), 2, SlotName::Deps),
        (cfg::EdgeKind::DeferReplace(1), Outcome::DeferReplace(1), 0, SlotName::Packet),
        (cfg::EdgeKind::DeferReplace(1), Outcome::DeferReplace(1), 1, SlotName::Deps),
        (cfg::EdgeKind::DeferUnwind, Outcome::Unwind, 0, SlotName::Failure),
    ]);
    let entry = body.entry;
    let mut body = module.functions.get_mut(caller).unwrap().body.take().unwrap();
    let mut editor = FunctionEditor::new(&mut module, &mut body);
    let arguments = vec![
        EdgeArg::Produced(EdgeResult { outcome: Outcome::DeferReplace(0), slot: 0 }),
        EdgeArg::Produced(EdgeResult { outcome: Outcome::DeferReplace(0), slot: 1 }),
        EdgeArg::Produced(EdgeResult { outcome: Outcome::DeferReplace(0), slot: 2 }),
    ];
    editor.replace_edge_arguments(entry, cfg::EdgeKind::DeferReplace(0), arguments.clone()).unwrap();
    let mut foreign = arguments;
    foreign[1] = EdgeArg::Produced(EdgeResult { outcome: Outcome::DeferReplace(1), slot: 0 });
    assert!(editor.replace_edge_arguments(entry, cfg::EdgeKind::DeferReplace(0), foreign).is_err());
    editor.renumber().unwrap();
    module.functions.get_mut(caller).unwrap().body = Some(body);
    roundtrip(&module);
}

#[test]
fn own_future_has_two_slots_and_frame_future_keeps_lower_profile_schema() {
    let source = include_str!("../tests/fixtures/text/d12-deferred-future.oir");
    let module = text::parse(source).unwrap();
    let printed = roundtrip(&module);
    assert!(printed.contains("environment deferred("));
    let body = module.functions.get(FuncId::from_u32(1)).unwrap().body.as_ref().unwrap();
    let (_, inst) = body.insts.iter().next().unwrap();
    let registry = ContractRegistry::new(&module);
    let contract = registry.lookup(&inst.op).unwrap();
    assert_eq!(contract.normal.results.iter().map(|slot| slot.name).collect::<Vec<_>>(),
        [SlotName::Value, SlotName::To]);
    assert!(!printed.contains("cache-state"));
    assert!(contract.normal.postconditions.iter().any(|condition| matches!(condition,
        Postcondition::OwnerServiceDuty { value: SlotName::Value, kind: ServiceKeyKind::Cache, .. })));
    let mut frame_op = inst.op.clone();
    if let Operation::Async(AsyncOp::FutureNew { environment, .. }) = &mut frame_op {
        *environment = FutureEnvironment::Frame(cs());
    }
    let mut lower_module = module.clone();
    lower_module.profile = Profile::Coro;
    let lower = ContractRegistry::new(&lower_module).lookup(&frame_op).unwrap();
    assert_eq!(lower.normal.results.iter().map(|slot| slot.name).collect::<Vec<_>>(),
        [SlotName::Value, SlotName::State, SlotName::To]);
}

#[test]
fn removed_exit_forms_are_not_in_the_codec() {
    for opcode in ["exit.finish_abort", "exit.discard_empty"] {
        let source = format!(
            "oir 2 own\nmodule removed\nscope S action in @f\nfunc @f(%p: pending) -> void {{\n\
             ^entry in S:\n    {opcode} %p\n    ret\n}}\n"
        );
        assert!(text::parse(&source).is_err());
        assert!(!Operation::opcode_inventory().contains(&opcode));
    }
}
