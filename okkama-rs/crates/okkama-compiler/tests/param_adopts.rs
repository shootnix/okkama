//! Compile-only calling convention checks (#555).

use okkama_compiler::SourceFile;
use okkama_compiler::ir::{self, Attr, FuncId, InstKind};
use okkama_compiler::lowering::{self, lower_program};
use okkama_compiler::syntax::{self, ParseOptions, prune};

const RES: &str = "module app v1.0.0; object Res { has n is int; Res(:n); \
    read() is int { n; } destroy { confirm n > 0; } }";
const USE: &str = "object Use { Use(); use(r is Res) is int { r.read(); } }";

fn module(text: &str) -> ir::Module {
    lowering::select_passes("none");
    let source = SourceFile::new("test.ok", text).expect("small source");
    let program = syntax::parse(&source, &ParseOptions::default()).expect("valid syntax");
    let module = lower_program(&prune::strip_test_items(program))
        .unwrap_or_else(|error| panic!("{text}: {error}"));
    ir::verify(&module).expect("lowering output verifies");
    module
}

fn function(module: &ir::Module, symbol: &str) -> usize {
    module.functions.iter().position(|function| function.symbol == symbol)
        .unwrap_or_else(|| panic!("missing {symbol}: {module}"))
}

fn adopts(module: &ir::Module, symbol: &str, parameter: usize) -> usize {
    let body = &module.functions[function(module, symbol)];
    body.blocks.iter().flat_map(|block| &block.insts).filter(|inst| {
        matches!(inst.kind, InstKind::RegionAdopt { value, .. } if value == body.params[parameter])
    }).count()
}

fn run(module: &mut ir::Module) -> Vec<usize> {
    let mut counts = Vec::new();
    let total = ir::param_adopts::elide_param_adopts(module, &mut counts);
    assert_eq!(total, counts.iter().sum());
    ir::verify(module).unwrap_or_else(|error| panic!("{error}: {module}"));
    counts
}

#[test]
fn bound_argument_elides_and_reports_then_regions_disappear() {
    let mut module = module(&format!("{RES} {USE} main() is int {{ let x = Res(n: 1); Use().use(x); }}"));
    assert_eq!(adopts(&module, "Use.use", 1), 1);
    let counts = run(&mut module);
    let index = function(&module, "Use.use");
    assert_eq!(adopts(&module, "Use.use", 1), 0);
    assert_eq!(counts[index], 1);
    ir::regions::elide_unused_regions(&mut module);
    ir::verify(&module).expect("regions output verifies");
    assert!(!module.functions[index].blocks.iter().flat_map(|block| &block.insts)
        .any(|inst| matches!(inst.kind, InstKind::RegionEnter)));
    let mut report = ir::ownership::analyze(&module);
    report.record_param_adopts(&counts);
    assert_eq!(report.totals.param_adopts_removed, counts.iter().sum());
    assert_eq!(report.functions[index].totals.param_adopts_removed, 1);
    assert!(report.to_json().contains("\"param_adopts_removed\":1"));
}

#[test]
fn a_fresh_argument_anywhere_keeps_the_adopt() {
    for body in [
        "Use().use(Res(n: 1));",
        "let x = Res(n: 1); Use().use(x); Use().use(Res(n: 2));",
    ] {
        let mut module = module(&format!("{RES} {USE} main() is int {{ {body} }}"));
        run(&mut module);
        assert_eq!(adopts(&module, "Use.use", 1), 1);
    }
}

#[test]
fn two_callers_one_fresh_keep_the_adopt() {
    let mut module = module(&format!("{RES} {USE} object Callers {{ Callers(); \
        owned() is int {{ let x = Res(n: 1); Use().use(x); }} \
        fresh() is int {{ Use().use(Res(n: 2)); }} }} main() is int {{ Callers().owned(); }}"));
    run(&mut module);
    assert_eq!(adopts(&module, "Use.use", 1), 1);
}

#[test]
fn owned_object_field_is_owned() {
    let mut module = module(&format!("{RES} {USE} object Holder {{ has res is Res; Holder(:res); \
        pass() is int {{ Use().use(res); }} }} main() is int {{ let h = Holder(res: Res(n: 1)); h.pass(); }}"));
    run(&mut module);
    assert_eq!(adopts(&module, "Holder.pass", 0), 0);
    assert_eq!(adopts(&module, "Use.use", 1), 0);
}

#[test]
fn parameters_forward_with_either_borrowed_or_adopted_inputs() {
    for (body, kept) in [("let x = Res(n: 1); A().a(x);", 0), ("A().a(Res(n: 1));", 1)] {
        let mut module = module(&format!("{RES} {USE} object A {{ A(); a(p is Res) is int {{ Use().use(p); }} }} \
            main() is int {{ {body} }}"));
        run(&mut module);
        assert_eq!(adopts(&module, "A.a", 1), kept);
        assert_eq!(adopts(&module, "Use.use", 1), 0);
    }
}

#[test]
fn role_calls_constrain_every_implementation_and_direct_calls_also_count() {
    for (extra, left) in [("", 0), ("Left().use(Res(n: 3));", 1)] {
        let mut module = module(&format!("{RES} role R {{ use(r is Res) is int; }} \
            object Left does R {{ Left(); use(r is Res) is int {{ r.read(); }} }} \
            object Right does R {{ Right(); use(r is Res) is int {{ r.read(); }} }} \
            object Dispatch {{ Dispatch(); call(target is R, r is Res) is int {{ target.use(r); }} }} \
            main() is int {{ let r = Res(n: 1); Dispatch().call(Left(), r); {extra} Dispatch().call(Right(), r); }}"));
        assert!(module.functions.iter().flat_map(|f| &f.blocks).flat_map(|b| &b.insts)
            .any(|inst| matches!(inst.kind, InstKind::CallRole { .. })));
        run(&mut module);
        assert_eq!(adopts(&module, "Left.use", 1), left);
        assert_eq!(adopts(&module, "Right.use", 1), 0);
    }
}

#[test]
fn closure_bodies_and_bound_actions_keep_adopts() {
    let mut module = module(&format!("{RES} {USE} main() is int {{ let x = Res(n: 1); \
        let f is (Res) is int = Use().use; Use().use(x); \
        let g = (r is Res) is int {{ r.read(); }}; f(x) + g(x); }}"));
    let closures: Vec<usize> = module.functions.iter().flat_map(|f| &f.blocks).flat_map(|b| &b.insts)
        .filter_map(|inst| match inst.kind { InstKind::FnClosure { func, .. } => Some(func.0), _ => None }).collect();
    assert!(!closures.is_empty());
    let before = module.clone();
    let counts = run(&mut module);
    for index in closures {
        assert_eq!(counts[index], 0);
        assert_eq!(module.functions[index], before.functions[index]);
    }
    assert_eq!(adopts(&module, "Use.use", 1), 1);
}

#[test]
fn async_main_and_unseen_entries_keep_adopts() {
    let mut module = module(&format!("{RES} {USE} object Job {{ Job(); \
        async work(r is Res) is int {{ r.read(); }} }} \
        async main() is int {{ let x = Res(n: 1); await Job().work(x); }}"));
    let entry = module.entry.expect("main").0;
    let main = module.functions[entry].clone();
    let counts = run(&mut module);
    assert_eq!(adopts(&module, "Job.work", 1), 1);
    assert_eq!(adopts(&module, "Use.use", 1), 1);
    assert_eq!(counts[entry], 0);
    assert_eq!(module.functions[entry], main);
}

#[test]
fn cap_computed_field_borrows_its_owned_owner() {
    let mut module = module(&format!("{RES} object Box {{ has item is Res; Box(:item); \
        cap View {{ count is int {{ item.read(); }} }} }} \
        main() is int {{ let b = Box(item: Res(n: 1)); let view = b::View; view::count; }}"));
    let target = module.caps[0].fields[0].1.0;
    let symbol = module.functions[target].symbol.clone();
    assert_eq!(adopts(&module, &symbol, 0), 1);
    run(&mut module);
    assert_eq!(adopts(&module, &symbol, 0), 0);
}

#[test]
fn test_and_intrinsic_entries_keep_adopts_even_with_visible_owned_calls() {
    for entry_kind in 2..4 {
        let mut module = module(&format!("{RES} {USE} main() is int {{ let x = Res(n: 1); Use().use(x); }}"));
        let target = FuncId(function(&module, "Use.use"));
        match entry_kind {
            2 => module.tests = Some(vec![("entry".into(), target)]),
            _ => {
                module.functions.push(ir::Function {
                    symbol: "runtime.helper".into(), params: vec![], result: ir::ResultType::Void,
                    attrs: vec![Attr::Intrinsic { name: "okkama.test.ok".into(), objects: vec![], helpers: vec![target] }],
                    values: vec![], blocks: vec![], span: module.span,
                });
            }
        }
        ir::verify(&module).expect("entry metadata verifies");
        let counts = run(&mut module);
        assert_eq!(adopts(&module, "Use.use", 1), 1);
        assert_eq!(counts[target.0], 0);
    }
}

#[test]
fn keeping_a_scope_result_makes_it_fresh_again() {
    let mut module = module(&format!("{RES} {USE} main() is int {{ \
        Use().use(if true {{ let x = Res(n: 1); x; }} else {{ let y = Res(n: 2); y; }}); }}"));
    run(&mut module);
    assert_eq!(adopts(&module, "Use.use", 1), 1);
}

#[test]
fn lookup_found_values_and_block_parameters_follow_all_edges() {
    for (fallback, expected) in [("x", 0), ("Res(n: 2)", 1)] {
        let mut module = module(&format!("{RES} {USE} main() is int {{ let x = Res(n: 1); \
            let values = [\"x\": x]; Use().use(values[\"x\"] else {fallback}); }}"));
        run(&mut module);
        assert_eq!(adopts(&module, "Use.use", 1), expected, "{module}");
    }
}

#[test]
fn tracked_call_results_stay_fresh() {
    let mut module = module(&format!("{RES} {USE} object Make {{ Make(); \
        fresh() is Res {{ Res(n: 1); }} }} main() is int {{ Use().use(Make().fresh()); }}"));
    run(&mut module);
    assert_eq!(adopts(&module, "Use.use", 1), 1);
}

// Start with lowered declarations; vary only the caller's ownership operations (#555).
struct Caller {
    module: ir::Module,
    main: usize,
    block: usize,
}

impl Caller {
    fn new() -> Self {
        let mut module = module(&format!("{RES} {USE} main() is int {{ 0; }}"));
        let main = module.entry.expect("main").0;
        module.functions[main].values.clear();
        module.functions[main].blocks.clear();
        let mut caller = Self { module, main, block: 0 };
        caller.add_block();
        caller
    }

    fn add_block(&mut self) -> ir::BlockId {
        let body = &mut self.module.functions[self.main];
        let id = ir::BlockId(body.blocks.len());
        body.blocks.push(ir::Block {
            params: vec![], unwind: None, insts: vec![],
            term: ir::Terminator { kind: ir::TermKind::Unreachable, span: body.span },
        });
        id
    }

    fn emit(&mut self, kind: InstKind, ty: Option<ir::Type>) -> Option<ir::ValueId> {
        let body = &mut self.module.functions[self.main];
        let result = ty.map(|ty| {
            let id = ir::ValueId(body.values.len());
            body.values.push(ty);
            id
        });
        body.blocks[self.block].insts.push(ir::Inst { result, kind, span: body.span });
        result
    }

    fn region(&mut self) -> ir::ValueId {
        self.emit(InstKind::RegionEnter, Some(ir::Type::Region)).unwrap()
    }

    fn fresh(&mut self) -> ir::ValueId {
        let n = self.emit(InstKind::Const(ir::Constant::Int(ir::IntValue::from_i64(1))), Some(ir::Type::Int)).unwrap();
        let ctor = function(&self.module, "Res.new#0");
        let ir::ResultType::Value(ty) = self.module.functions[ctor].result.clone() else { panic!("ctor") };
        self.emit(InstKind::Call { func: FuncId(ctor), args: vec![n] }, Some(ty)).unwrap()
    }

    fn term(&mut self, kind: ir::TermKind) {
        self.module.functions[self.main].blocks[self.block].term.kind = kind;
    }

    fn use_value(&mut self, value: ir::ValueId) {
        let ctor = function(&self.module, "Use.new#0");
        let ir::ResultType::Value(ty) = self.module.functions[ctor].result.clone() else { panic!("ctor") };
        let receiver = self.emit(InstKind::Call { func: FuncId(ctor), args: vec![] }, Some(ty)).unwrap();
        let callee = FuncId(function(&self.module, "Use.use"));
        let result = self.emit(InstKind::Call { func: callee, args: vec![receiver, value] }, Some(ir::Type::Int));
        self.term(ir::TermKind::Ret(result));
    }

    fn check(mut self, expected: usize) {
        ir::verify(&self.module).unwrap_or_else(|error| panic!("{error}: {}", self.module));
        run(&mut self.module);
        assert_eq!(adopts(&self.module, "Use.use", 1), expected, "{}", self.module);
    }
}

#[test]
fn track_and_transfer_cannot_adopt_free_identities() {
    for transfer in [false, true] {
        let mut caller = Caller::new();
        let outer = caller.region();
        let inner = caller.region();
        let value = caller.fresh();
        caller.emit(InstKind::RegionTrack { region: inner, value }, None);
        if transfer {
            caller.emit(InstKind::RegionTransfer { from: inner, to: outer, value }, None);
        }
        caller.use_value(value);
        caller.check(1);
    }
}

#[test]
fn transfer_preserves_known_ownership_after_source_closes() {
    let mut caller = Caller::new();
    let outer = caller.region();
    let inner = caller.region();
    let value = caller.fresh();
    caller.emit(InstKind::RegionAdopt { region: inner, value }, None);
    caller.emit(InstKind::RegionTransfer { from: inner, to: outer, value }, None);
    caller.emit(InstKind::RegionLeave { region: inner, keep: None }, None);
    caller.use_value(value);
    caller.check(0);
}

#[test]
fn readoption_does_not_move_an_existing_owner() {
    let mut caller = Caller::new();
    let outer = caller.region();
    let inner = caller.region();
    let value = caller.fresh();
    caller.emit(InstKind::RegionAdopt { region: inner, value }, None);
    caller.emit(InstKind::RegionAdopt { region: outer, value }, None);
    caller.emit(InstKind::RegionLeave { region: inner, keep: Some(value) }, None);
    caller.use_value(value);
    caller.check(1);
}

#[test]
fn adoption_must_hold_on_every_path_to_a_call() {
    for both in [false, true] {
        let mut caller = Caller::new();
        let region = caller.region();
        let value = caller.fresh();
        let condition = caller.emit(InstKind::Const(ir::Constant::Bool(true)), Some(ir::Type::Bool)).unwrap();
        let left = caller.add_block();
        let right = caller.add_block();
        let join = caller.add_block();
        caller.term(ir::TermKind::Cond {
            condition, then_target: ir::Target::new(left), else_target: ir::Target::new(right),
        });
        for block in [left, right] {
            caller.block = block.0;
            if both || block == left {
                caller.emit(InstKind::RegionAdopt { region, value }, None);
            }
            caller.term(ir::TermKind::Br(ir::Target::new(join)));
        }
        caller.block = join.0;
        caller.use_value(value);
        caller.check(usize::from(!both));
    }
}

#[test]
fn unwind_does_not_inherit_an_adopt_later_in_the_block() {
    let mut caller = Caller::new();
    let region = caller.region();
    let value = caller.fresh();
    let body = caller.add_block();
    let handler = caller.add_block();
    caller.term(ir::TermKind::Br(ir::Target::new(body)));
    let function = &mut caller.module.functions[caller.main];
    let failure = ir::ValueId(function.values.len());
    function.values.push(ir::Type::Failure);
    function.blocks[handler.0].params.push(failure);
    function.blocks[body.0].unwind = Some(ir::Target::new(handler));
    caller.block = body.0;
    caller.emit(InstKind::RegionAdopt { region, value }, None);
    caller.use_value(value);
    caller.block = handler.0;
    caller.use_value(value);
    caller.check(1);
}

#[test]
fn worker_entries_keep_adopts_without_running_workers() {
    let mut module = module(&format!("{RES} object W {{ has n is int; W(:n); \
        work(i is int) is int {{ i + n; }} destroy {{}} }} \
        async main() is int {{ await spawn W(n: 1).work(2); }}"));
    assert!(module.functions.iter().flat_map(|f| &f.blocks).flat_map(|b| &b.insts)
        .any(|inst| matches!(inst.kind, InstKind::WorkerSpawn { .. })));
    run(&mut module);
    assert_eq!(adopts(&module, "W.work", 0), 1);
}

#[test]
fn collection_copies_and_successor_results_are_fresh() {
    let mut module = module(&format!("{RES} object Counter {{ has r is Res; Counter(:r); \
        next() is ^Counter {{ ^r = r; }} destroy {{}} }} \
        object Check {{ Check(); array(rs is @Res) is int {{ rs.size(); }} \
        counter(c is Counter) is int {{ 0; }} }} main() is int {{ let rs = [Res(n: 1)]; \
        Check().array(rs + rs); let c = Counter(r: Res(n: 2)); Check().counter(c.next()); }}"));
    run(&mut module);
    assert_eq!(adopts(&module, "Check.array", 1), 1);
    assert_eq!(adopts(&module, "Check.counter", 1), 1);
}

#[test]
fn awaited_results_are_fresh_until_adopted() {
    for (body, expected) in [
        ("Use().use(await Job().make());", 1),
        ("let x = await Job().make(); Use().use(x);", 0),
    ] {
        let mut module = module(&format!("{RES} {USE} object Job {{ Job(); \
            async make() is Res {{ Res(n: 1); }} }} async main() is int {{ {body} }}"));
        run(&mut module);
        assert_eq!(adopts(&module, "Use.use", 1), expected);
    }
}

#[test]
fn loops_keep_owned_block_parameters_but_reject_fresh_backedges() {
    for fresh in [false, true] {
        let mut caller = Caller::new();
        let region = caller.region();
        let value = caller.fresh();
        caller.emit(InstKind::RegionAdopt { region, value }, None);
        let condition = caller.emit(InstKind::Const(ir::Constant::Bool(false)), Some(ir::Type::Bool)).unwrap();
        let header = caller.add_block();
        let backedge = caller.add_block();
        let exit = caller.add_block();
        let body = &mut caller.module.functions[caller.main];
        let param = ir::ValueId(body.values.len());
        body.values.push(body.values[value.0].clone());
        body.blocks[header.0].params.push(param);
        caller.term(ir::TermKind::Br(ir::Target { block: header, args: vec![value] }));
        caller.block = header.0;
        caller.term(ir::TermKind::Cond {
            condition, then_target: ir::Target::new(backedge), else_target: ir::Target::new(exit),
        });
        caller.block = backedge.0;
        let next = if fresh { caller.fresh() } else { param };
        caller.term(ir::TermKind::Br(ir::Target { block: header, args: vec![next] }));
        caller.block = exit.0;
        caller.use_value(param);
        caller.check(usize::from(fresh));
    }
}
