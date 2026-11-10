//! Lowering владения (OIR-1 §7, §19.3): регионы областей, `defer`, `destroy`.

use okkama_compiler::SourceFile;
use okkama_compiler::ir::{self, InstKind, TermKind};
use okkama_compiler::lowering::{LoweringError, lower_program};
use okkama_compiler::syntax::{self, ParseOptions, prune};

fn lower(text: &str) -> Result<ir::Module, LoweringError> {
    let source = SourceFile::new("test.ok", text).expect("small source");
    let program = syntax::parse(&source, &ParseOptions::default()).expect("valid syntax");
    lower_program(&prune::strip_test_items(program))
}

fn module(text: &str) -> ir::Module {
    let module = lower(text).unwrap_or_else(|error| panic!("{text}: {error}"));
    ir::verify(&module).expect("lowering output verifies");
    module
}

fn function<'m>(module: &'m ir::Module, symbol: &str) -> &'m ir::Function {
    module
        .functions
        .iter()
        .find(|function| function.symbol == symbol)
        .expect("function exists")
}

fn count(function: &ir::Function, matches: impl Fn(&InstKind) -> bool) -> usize {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.insts)
        .filter(|inst| matches(&inst.kind))
        .count()
}

const RES: &str =
    "module app v1.0.0; object Res { has n is int; Res(:n); destroy { confirm n > 0; } }";

#[test]
fn modules_without_destroy_emit_no_regions() {
    let text = module("module app v1.0.0; object P { has n is int; P(:n); } main() is int { let p = P(n: 1); 0; }")
        .to_string();
    assert!(!text.contains("region."), "{text}");
}

#[test]
fn bindings_own_and_call_results_are_tracked() {
    let module = module(&format!(
        "{RES} main() is int {{ let r = Res(n: 1); Res(n: 2); 0; }}"
    ));
    let text = module.to_string();
    assert!(text.contains("    destroy = @Res.destroy\n"), "{text}");
    assert!(text.contains("[destroy Res]"), "{text}");
    let main = function(&module, "main");
    // Оба результата конструктора учитываются, `let` принимает первый во владение.
    assert_eq!(
        count(main, |kind| matches!(kind, InstKind::RegionTrack { .. })),
        2
    );
    assert_eq!(
        count(main, |kind| matches!(kind, InstKind::RegionAdopt { .. })),
        1
    );
    // Регион тела закрывается на нормальном пути и на пути отказа. Регион
    // корня `main` ничем не владеет (параметров нет) и убран.
    assert_eq!(count(main, |kind| matches!(kind, InstKind::RegionEnter)), 1);
    assert_eq!(
        count(main, |kind| matches!(kind, InstKind::RegionLeave { .. })),
        1
    );
    assert_eq!(
        count(main, |kind| matches!(kind, InstKind::RegionUnwind { .. })),
        1
    );
    // Конструктор Res ничем не владеет (поле — int): региона у него нет.
    let ctor = function(&module, "Res.new#0");
    assert_eq!(count(ctor, |kind| matches!(kind, InstKind::RegionEnter)), 0);
}

#[test]
fn defers_run_on_exits_and_join_failures_as_secondary() {
    let module = module(&format!(
        "{RES} object Log {{ Log(); fail() is void {{ confirm false; }} }} \
         main() is int {{ defer Log().fail(); defer Log().fail(); 0; }}"
    ));
    let main = function(&module, "main");
    // Каждый defer — на нормальном выходе и в цепочке отказа.
    let calls = count(main, |kind| matches!(kind, InstKind::Call { .. }));
    assert!(calls >= 8, "{} calls in\n{module}", calls);
    assert_eq!(
        count(main, |kind| matches!(kind, InstKind::FailSecondary { .. })),
        2,
        "{module}"
    );
    let raises = main
        .blocks
        .iter()
        .filter(|block| matches!(block.term.kind, TermKind::Raise(_)))
        .count();
    // Цепочка отказа заканчивается одним `raise`: блоки, которые только
    // передают отказ дальше, пропускаются.
    assert_eq!(raises, 1, "{module}");
}

#[test]
fn destroy_rules_are_semantic_errors() {
    for (text, expected) in [
        (
            "module app v1.0.0; object Box { has n is int = 0; Box(); next() is ^Box { ^n = n + 1; } \
             destroy { ^n = 1; } } main() is int { 0; }",
            "Successor assignment is not allowed inside destroy",
        ),
        (
            "module app v1.0.0; object Box { Box(); destroy { return; } } main() is int { 0; }",
            "return is only allowed inside an action",
        ),
        (
            "module app v1.0.0; object Box { Box(); destroy { } destroy { } } main() is int { 0; }",
            "Object \"Box\" cannot declare more than one destroy block",
        ),
        (
            "module app v1.0.0; object Counter { has v is int; Counter(:v); inc() is ^Counter { ^v = v + 1; } } \
             main() is int { let c = Counter(v: 1); defer c.inc(); 0; }",
            "Successor action result must be used",
        ),
    ] {
        let error = lower(text).unwrap_err();
        assert!(
            matches!(error, LoweringError::Invalid { .. }),
            "{text}: {error:?}"
        );
        assert_eq!(error.message(), expected, "{text}");
    }
}

#[test]
fn regions_without_identities_are_elided() {
    let module = module(&format!(
        "{RES} object Point {{ has x is int; Point(:x); sum(b is int) is int {{ x + b; }} }} \
         object Holder {{ has res is Res; Holder(:res); }} \
         main() is int {{ let r = Res(n: 1); let h = Holder(res: Res(n: 3)); Point(x: 2).sum(1); }}"
    ));
    // Только скаляры: регионы не открываются вовсе.
    let sum = function(&module, "Point.sum");
    assert_eq!(count(sum, |kind| matches!(kind, InstKind::RegionEnter)), 0);
    assert_eq!(count(sum, |kind| matches!(kind, InstKind::RegionLeave { .. })), 0);
    assert_eq!(count(sum, |kind| matches!(kind, InstKind::RegionUnwind { .. })), 0);
    // Конструктор, владеющий объектом с destroy, отдаёт его вызывающему:
    // `region.leave …, keep`.
    let holder = function(&module, "Holder.new#0");
    assert_eq!(
        count(holder, |kind| matches!(kind, InstKind::RegionLeave { keep: Some(_), .. })),
        1
    );
    // Объект с destroy по-прежнему принадлежит региону области.
    let main = function(&module, "main");
    assert!(count(main, |kind| matches!(kind, InstKind::RegionAdopt { .. })) >= 1);
    assert!(count(main, |kind| matches!(kind, InstKind::RegionLeave { .. })) >= 1);
}

/// Функции, которые ничего с `destroy` не захватывают, регионы не
/// учитывают, даже если `destroy` в программе есть (#292).
#[test]
fn plain_closures_are_not_tracked() {
    let module = module(&format!(
        "{RES} object Pipeline {{ Pipeline(); adder(amount is int) is (int) is int {{ (value is int) is int {{ value + amount; }}; }} \
         compose(f is (int) is int, g is (int) is int) is (int) is int {{ (value is int) is int {{ g(f(value)); }}; }} }} \
         main() is int {{ let r = Res(n: 1); let p = Pipeline(); let both = p.compose(p.adder(1), p.adder(2)); both(3); }}"
    ));
    for symbol in ["Pipeline.adder", "Pipeline.compose"] {
        let function = function(&module, symbol);
        assert_eq!(count(function, |kind| matches!(kind, InstKind::RegionEnter)), 0, "{symbol}");
        // Без регионов тело — один блок (`ir::cfg::merge_blocks`).
        assert_eq!(function.blocks.len(), 1, "{symbol}");
    }
    let main = function(&module, "main");
    assert_eq!(count(main, |kind| matches!(kind, InstKind::RegionTrack { .. })), 1, "only Res");
}

/// Plainness of a role is per instance (#268, done by #292): `Source<int>`
/// has only plain implementers, so a value known as one is not tracked,
/// although `Source<string>` has one that owns a resource and is.
#[test]
fn role_plainness_is_per_instance() {
    let module = module(&format!(
        "{RES} role Source<T> {{ item() is T; }} \
         object Plain<T> does Source<T> {{ has value is T; Plain(:value); item() is T {{ value; }} }} \
         object Leaky does Source<string> {{ has res is Res; Leaky(:res); item() is string {{ \"x\"; }} }} \
         object Factory {{ Factory(); ints() is Source<int> {{ Plain(value: 1); }} strings(owned is bool) is Source<string> {{ \
             if owned {{ Leaky(res: Res(n: 1)); }} else {{ Plain(value: \"y\"); }} }} }} \
         object Reader {{ Reader(); \
             ints(factory is Factory) is int {{ let source = factory.ints(); source.item(); }} \
             strings(factory is Factory) is int {{ let source = factory.strings(true); source.item().length(); }} }} \
         main() is int {{ let factory = Factory(); let reader = Reader(); reader.ints(factory) + reader.strings(factory); }}"
    ));
    let ownership = |kind: &InstKind| {
        matches!(kind, InstKind::RegionTrack { .. } | InstKind::RegionAdopt { .. } | InstKind::RegionTransfer { .. })
    };
    assert_eq!(count(function(&module, "Reader.ints"), ownership), 0, "Source<int> is plain");
    assert!(count(function(&module, "Reader.strings"), ownership) >= 1, "Source<string> is not");
}

/// Замыкание, захватившее объект с `destroy`, владеет им: функции программы
/// по-прежнему учитываются регионами (#292).
#[test]
fn closures_that_capture_resources_stay_tracked() {
    let module = module(&format!(
        "{RES} object Keeper {{ Keeper(); keep(res is Res) is () is Res {{ () is Res {{ res; }}; }} }} \
         main() is int {{ let keep = Keeper().keep(Res(n: 1)); let res = keep(); 0; }}"
    ));
    let keep = function(&module, "Keeper.keep");
    assert_eq!(count(keep, |kind| matches!(kind, InstKind::RegionTrack { .. })), 1);
    let main = function(&module, "main");
    assert!(count(main, |kind| matches!(kind, InstKind::RegionAdopt { .. })) >= 1);
}

/// `async destroy` (#154): scopes of async functions close with
/// `region.settle`, so the destroy is awaited; synchronous ones keep
/// `region.leave`, and a program without an async destroy never settles.
#[test]
fn async_destroys_are_settled_in_async_functions() {
    let conn = "module app v1.0.0; object Conn { has n is int; Conn(:n); ok() is bool { n > 0; } async destroy { confirm n > 0; } } \
        object Res { has n is int; Res(:n); destroy { confirm n > 0; } }";
    let settled = module(&format!(
        "{conn} object Job {{ Job(); async run() is int {{ let c = Conn(n: 1); confirm c.ok(); 1; }} work() is int {{ let r = Res(n: 1); 2; }} }} \
         async main() is int {{ await Job().run(); }}"
    ));
    let text = settled.to_string();
    let run = function(&settled, "Job.run");
    assert!(count(run, |kind| matches!(kind, InstKind::RegionSettle { .. })) >= 1, "{text}");
    assert!(count(run, |kind| matches!(kind, InstKind::RegionSettleUnwind { .. })) >= 1, "{text}");
    assert_eq!(count(run, |kind| matches!(kind, InstKind::RegionLeave { .. } | InstKind::RegionUnwind { .. })), 0);
    let work = function(&settled, "Job.work");
    assert_eq!(count(work, |kind| matches!(kind, InstKind::RegionSettle { .. } | InstKind::RegionSettleUnwind { .. })), 0);
    assert!(count(work, |kind| matches!(kind, InstKind::RegionLeave { .. })) >= 1, "{text}");

    let plain = module(&format!(
        "{RES} object Job {{ Job(); async run() is int {{ let r = Res(n: 1); 1; }} }} async main() is int {{ await Job().run(); }}"
    ));
    assert!(!plain.to_string().contains("region.settle"));
}

/// A `reduce` that grows a builder (#243) hands what it pushes to the scope
/// of the `reduce` and never tracks, adopts or keeps the builder (#266).
#[test]
fn reduce_builders_hand_pushed_resources_to_their_scope() {
    let built = module(&format!(
        "{RES} object Maker {{ Maker(); \
         fresh(count is int) is @Res {{ (0..count).reduce([]) -> found, i {{ found + [Res(n: i + 1)]; }}; }} \
         bound(count is int) is %int:Res {{ (0..count).reduce([:]) -> found, i {{ let r = Res(n: i + 1); found + [i: r]; }}; }} }} \
         main() is int {{ let m = Maker(); m.fresh(2).size() + m.bound(2).size(); }}"
    ));
    let text = built.to_string();
    for symbol in ["Maker.fresh", "Maker.bound"] {
        let body = function(&built, symbol);
        let builders: std::collections::HashSet<ir::ValueId> =
            (0..body.values.len()).map(ir::ValueId).filter(|value| body.values[value.0].is_builder()).collect();
        let touches_builder = |kind: &InstKind| match kind {
            InstKind::RegionTrack { value, .. } | InstKind::RegionAdopt { value, .. } | InstKind::RegionTransfer { value, .. } => {
                builders.contains(value)
            }
            InstKind::RegionLeave { keep: Some(value), .. } => builders.contains(value),
            _ => false,
        };
        assert_eq!(count(body, touches_builder), 0, "{symbol}: {text}");
        assert!(count(body, |kind| matches!(kind, InstKind::RegionAdopt { .. })) >= 1, "{symbol}: {text}");
    }
    let bound = function(&built, "Maker.bound");
    assert!(count(bound, |kind| matches!(kind, InstKind::RegionTransfer { .. })) >= 1, "{text}");
}
