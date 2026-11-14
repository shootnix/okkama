use okkama_oir::{text, *};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 }
    fn size(&mut self, max: usize) -> usize { self.next() as usize % max }
}

fn generated(seed: u64) -> Module {
    let mut rng = Rng(seed);
    let profile = [Profile::High, Profile::Own, Profile::Coro, Profile::Low][rng.size(4)];
    let mut m = ModuleBuilder::new("generated", profile);
    let int = m.intern_type(Type::Int).unwrap(); let boolean = m.intern_type(Type::Bool).unwrap();
    let f = m.declare_function(Function { symbol: "cycle".into(), signature: Signature { params: vec![int, boolean], results: vec![int], ownership_abi: OwnershipAbi::Unelaborated }, attrs: vec![], type_params: vec![], summary: None, async_creation_summary: None, async_body_summary: None, body: None, loc: None }).unwrap();
    let scope = m.declare_scope(ScopeDecl { function: f, parent: None, kind: ScopeKind::Action, loc: None }).unwrap();
    let state_ty = m.intern_type(Type::Service(ServiceType::Scope(scope))).unwrap();
    let mut b = m.function_builder(f, scope).unwrap();
    let input = b.block_params(b.entry()).unwrap()[0]; let condition = b.block_params(b.entry()).unwrap()[1];
    let dep = b.intern_dependency(DepExpr::Empty).unwrap();
    let selected = b.declare_selection(SelectionExpr::Value(input)).unwrap();
    let admissible = b.declare_selection(SelectionExpr::Admissible(selected)).unwrap();
    let union = b.declare_selection(SelectionExpr::Union(vec![selected, admissible])).unwrap();
    let state = LoweringBuilder::new(&mut b).scope_begin(scope).unwrap();
    let packet = LoweringBuilder::new(&mut b).own_empty().unwrap();
    let state = LoweringBuilder::new(&mut b).own_accept(state, packet).unwrap();
    let count = 2 + rng.size(7);
    let blocks: Vec<_> = (0..count).map(|_| b.create_block(scope, &[ParamSpec::new(int), ParamSpec::new(state_ty)]).unwrap()).collect();
    let exit = b.create_block(scope, &[ParamSpec::new(int), ParamSpec::new(state_ty)]).unwrap();
    b.terminate(TermKind::Br(Target { block: blocks[0], args: vec![input, state] })).unwrap();
    for (i, &block) in blocks.iter().enumerate() {
        b.switch_to(block).unwrap();
        let mut value = b.block_params(block).unwrap()[0]; let q = b.block_params(block).unwrap()[1];
        for _ in 0..1 + rng.size(5) {
            let constant = b.emit_schema(Operation::Const(Literal::Int((rng.size(1000) + 1).to_string()))).unwrap()[0];
            value = b.emit_with_facts(Operation::Core(CoreOp::IntAdd { left: value, right: constant }), &[ResultSpec { ty: int, facts: ValueFacts::Plain { certificate: PlainCertificate::Scalar } }], InstructionFacts { result_deps: vec![(0, dep)], retention: vec![union], provenance: vec![ProvenanceFact { obligations: admissible, state: AdmissionProvenance::Bound }] }).unwrap()[0];
        }
        // The final block always has a backedge; intermediate blocks also jump
        // backwards, producing reducible and irreducible cycles across seeds.
        let next = if i + 1 == blocks.len() { blocks[0] } else { blocks[i + 1] };
        let other = if rng.size(2) == 0 { exit } else { blocks[rng.size(blocks.len())] };
        b.terminate(TermKind::Cond { condition, yes: Target { block: next, args: vec![value, q] }, no: Target { block: other, args: vec![value, q] } }).unwrap();
    }
    b.switch_to(exit).unwrap();
    let answer = b.block_params(exit).unwrap()[0]; let q = b.block_params(exit).unwrap()[1];
    let ends = b.invoke_own_successors(Operation::Ownership(OwnershipOp::ScopeEnd { state: q, reserve: None }), scope, scope).unwrap();
    b.switch_to(ends.normal).unwrap(); b.terminate(TermKind::Ret { values: vec![answer] }).unwrap();
    b.switch_to(ends.unwind).unwrap(); b.terminate(TermKind::Raise(b.block_params(ends.unwind).unwrap()[1])).unwrap();
    let body = b.finish().unwrap(); m.define_function(f, body).unwrap(); m.finish().unwrap()
}

fn renamed(source: &str) -> String {
    let mut output = String::new(); let mut chars = source.chars().peekable();
    while let Some(c) = chars.next() {
        output.push(c);
        if c == '%' || c == '^' {
            output.push_str("renamed_");
            while chars.peek().is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_') { output.push(chars.next().unwrap()); }
        }
    }
    output
}

fn witness(mut module: Module) -> Vec<String> {
    let mut body = module.functions.get_mut(FuncId::from_u32(0)).unwrap().body.take().unwrap();
    FunctionEditor::new(&mut module, &mut body).renumber().unwrap();
    let mut result = vec![format!("{:?}", module.profile)];
    for (_, value) in body.values.iter() { result.push(format!("{:?}/{:?}/{:?}", module.types.get(value.ty).unwrap(), value.def, value.facts)); }
    for (_, block) in body.blocks.iter() { result.push(format!("{:?}", block)); }
    for (_, inst) in body.insts.iter() {
        result.push(format!("{:?}/{:?}/{:?}/{:?}", inst.op, inst.results, inst.loc, inst.facts.result_deps));
        for &selection in &inst.facts.retention {
            result.push(selection_shape(selection, &body));
        }
        for fact in &inst.facts.provenance { result.push(format!("{:?}/{}", fact.state, selection_shape(fact.obligations, &body))); }
    }
    result
}
fn selection_shape(id: SelectionId, body: &Body) -> String {
    match body.selections.get(id).unwrap() {
        SelectionExpr::Value(value) => format!("value({value:?})"),
        SelectionExpr::Admissible(child) => format!("admissible({})", selection_shape(*child, body)),
        SelectionExpr::Union(children) => format!("union({:?})", children.iter().map(|id| selection_shape(*id, body)).collect::<Vec<_>>()),
        other => panic!("generator did not declare {other:?}"),
    }
}

#[test]
fn bounded_generated_cycles_obey_both_roundtrip_formulas_and_mutation() {
    for seed in 1..=128 {
        let original = generated(seed);
        let printed = text::print(&original).unwrap(); let parsed = text::parse(&printed).unwrap();
        assert!(text::normalized_eq(&original, &parsed).unwrap(), "seed {seed}");
        assert_eq!(text::print(&parsed).unwrap(), printed, "seed {seed}");
        // Independently compare operations, CFG, types and fact contents after
        // canonical arena numbering; equivalent inline selections can duplicate
        // expression nodes during parsing.
        assert_eq!(witness(original.clone()), witness(parsed.clone()), "structural witness seed {seed}");
        let variants = [renamed(&printed), printed.replace('\n', "\r\n"), format!("{printed}// trailing comment\n")];
        for variant in variants { assert!(text::normalized_eq(&original, &text::parse(&variant).unwrap()).unwrap(), "seed {seed}"); }
        let mut changed = original.clone();
        let body = changed.functions.get_mut(FuncId::from_u32(0)).unwrap().body.as_mut().unwrap();
        let inst = body.insts.iter_mut().find(|(_, inst)| matches!(inst.op, Operation::Const(_))).unwrap().1;
        inst.op = Operation::Const(Literal::Int("100001".into()));
        assert_ne!(text::print(&changed).unwrap(), printed, "semantic mutation seed {seed}");
        assert!(!text::normalized_eq(&original, &changed).unwrap());
    }
}

#[test]
fn bounded_malformed_inputs_never_panic() {
    let valid = text::print(&generated(0x563)).unwrap();
    let mut rng = Rng(0x565_564_563);
    let alphabet = b"{}[](),:=!%@$^\n\r abcdef0123456789\"\\";
    for n in 0..2048 {
        let mut bytes = if n % 2 == 0 { valid.as_bytes().to_vec() } else { b"oir 2 own\n".to_vec() };
        for _ in 0..1 + rng.size(16) {
            let position = rng.size(bytes.len() + 1);
            match rng.size(3) {
                0 if position < bytes.len() => { bytes.remove(position); }
                1 if position < bytes.len() => { bytes[position] = alphabet[rng.size(alphabet.len())]; }
                _ => bytes.insert(position, alphabet[rng.size(alphabet.len())]),
            }
        }
        bytes.truncate(8192);
        let input = String::from_utf8(bytes).unwrap();
        let outcome = std::panic::catch_unwind(|| {
            if let Ok(module) = text::parse(&input) && let Ok(printed) = text::print(&module) { text::parse(&printed).unwrap(); }
        });
        assert!(outcome.is_ok(), "malformed seed {n}: {input}");
    }
    for depth in [64, 128, 512, 2048] {
        let input = format!("oir 2 high\nmodule nesting\nfunc @f(%x: {}int{}) -> void;\n", "array<".repeat(depth), ">".repeat(depth));
        assert!(std::panic::catch_unwind(|| text::parse(&input)).unwrap().is_err());
    }
}
