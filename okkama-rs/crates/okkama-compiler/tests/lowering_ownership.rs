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
    assert!(raises >= 2, "{module}");
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
