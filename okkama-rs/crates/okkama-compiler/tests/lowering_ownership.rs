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
fn discarded_await_results_are_tracked_by_the_enclosing_block() {
    let module = module(&format!(
        "{RES} object Work {{ Work(); async make() is Res {{ Res(n: 2); }} }} \
         async main() is int {{ Res(n: 4); {{ Res(n: 1); await Work().make(); Res(n: 3); }} 0; }}"
    ));
    let main = function(&module, "main");
    let insts: Vec<_> = main.blocks.iter().flat_map(|block| &block.insts).collect();
    let awaited = insts.iter().find(|inst| matches!(inst.kind, InstKind::AwaitTake(_)))
        .expect("await exists").result.expect("await returns Res");
    let tracked_region = |value| {
        let regions: Vec<_> = insts.iter().filter_map(|inst| match inst.kind {
            InstKind::RegionTrack { region, value: tracked } if tracked == value => Some(region),
            _ => None,
        }).collect();
        assert_eq!(regions.len(), 1, "each temporary is tracked exactly once:\n{module}");
        regions[0]
    };
    let region = tracked_region(awaited);
    let calls: Vec<_> = insts.iter().filter_map(|inst| match &inst.kind {
        InstKind::Call { func, .. } if module.functions[func.0].symbol == "Res.new#0" => inst.result,
        _ => None,
    }).collect();
    assert_eq!(calls.len(), 3, "{module}");
    assert_ne!(tracked_region(calls[0]), region, "outer scope stays separate:\n{module}");
    assert_eq!(tracked_region(calls[1]), region, "preceding temporary shares the region:\n{module}");
    assert_eq!(tracked_region(calls[2]), region, "following temporary shares the region:\n{module}");
    assert_eq!(count(main, |kind| matches!(kind,
        InstKind::RegionLeave { region: closed, keep: None } if *closed == region
    )), 1, "discarded result does not escape the block:\n{module}");
    assert_eq!(count(main, |kind| matches!(kind,
        InstKind::RegionUnwind { region: closed, .. } if *closed == region
    )), 1, "failure also closes the block:\n{module}");
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

/// Whether `function` closes its regions with `region.settle` (#328): every
/// close of a body is one or the other, never a mix.
fn settles(module: &ir::Module, symbol: &str) -> bool {
    let body = function(module, symbol);
    let settles = count(body, |kind| matches!(kind, InstKind::RegionSettle { .. } | InstKind::RegionSettleUnwind { .. }));
    let leaves = count(body, |kind| matches!(kind, InstKind::RegionLeave { .. } | InstKind::RegionUnwind { .. }));
    assert!(settles == 0 || leaves == 0, "{symbol} mixes settles and leaves:\n{module}");
    settles > 0
}

/// `async destroy` in a program no longer makes every async body settle
/// (#328): only a body whose regions may take a value that reaches an
/// object with an async destroy does. Anything the analysis cannot rule out
/// — roles such objects do, generics instantiated with them, `any`,
/// closures — still settles.
#[test]
fn only_bodies_that_may_own_an_async_destroy_settle() {
    let prelude = "module app v1.0.0; \
        role Closing { ok() is bool; } \
        role Plain { ok() is bool; } \
        object Conn does Closing { has n is int; Conn(:n); ok() is bool { n > 0; } async destroy { confirm n > 0; } } \
        object Res does Plain { has n is int; Res(:n); ok() is bool { n > 0; } destroy { confirm n > 0; } } \
        object Holder { has conn is Conn; Holder(:conn); ok() is bool { conn.ok(); } } \
        object Box<T> { has item is T; Box(:item); } \
        object Maker { Maker(); async conn() is Conn { Conn(n: 1); } async res() is Res { Res(n: 1); } }";
    let bodies = "object Job { Job(); \
        async unrelated() is int { let r = Res(n: 1); let rs = [Res(n: 2)]; for 0..3 -> i { let inner = Res(n: i + 1); confirm inner.ok(); } confirm r.ok(); rs.size(); } \
        async failing() is int { let r = Res(n: 1); if r.ok() { throw Broken.broken; } 1; } \
        async direct() is int { let c = Conn(n: 1); confirm c.ok(); 1; } \
        async transitive() is int { let h = Holder(conn: Conn(n: 1)); confirm h.ok(); 1; } \
        async array() is int { let all = [Conn(n: 1)]; all.size(); } \
        async map() is int { let all = [\"a\": Conn(n: 1)]; all.size(); } \
        async role() is bool { let c is Closing = Conn(n: 1); c.ok(); } \
        async plain_role() is bool { let p is Plain = Res(n: 1); p.ok(); } \
        async boxed_res() is int { let b = Box(item: Res(n: 1)); 1; } \
        async boxed_conn() is int { let b = Box(item: Conn(n: 1)); 1; } \
        async closure() is int { let r = Res(n: 1); let f = () is bool { r.ok(); }; if f() { 1; } else { 0; } } \
        async returned() is int { let c = await Maker().conn(); confirm c.ok(); 1; } \
        async returned_res() is int { let r = await Maker().res(); confirm r.ok(); 1; } \
        async nested() is int { for 0..2 -> i { let c = Conn(n: i + 1); confirm c.ok(); } 1; } \
        sync_conn() is int { 1; } }";
    let program = format!(
        "{prelude} enum Broken is Error = (broken); {bodies} \
         async main() is int {{ let job = Job(); await job.unrelated(); await job.direct(); 0; }}"
    );
    let module = module(&program);
    for symbol in ["Job.unrelated", "Job.failing", "Job.plain_role", "Job.boxed_res", "Job.closure", "Job.returned_res", "Maker.res"] {
        assert!(!settles(&module, symbol), "{symbol} owns nothing with an async destroy:\n{module}");
    }
    for symbol in [
        "Job.direct", "Job.transitive", "Job.array", "Job.map", "Job.role", "Job.boxed_conn",
        "Job.returned", "Job.nested", "Maker.conn",
    ] {
        assert!(settles(&module, symbol), "{symbol} may own an async destroy:\n{module}");
    }
    // The unrelated body still closes its regions, synchronously, on every
    // exit: block, loop iteration, failure.
    let unrelated = function(&module, "Job.unrelated");
    assert!(count(unrelated, |kind| matches!(kind, InstKind::RegionLeave { .. })) >= 2, "{module}");
    assert!(count(unrelated, |kind| matches!(kind, InstKind::RegionUnwind { .. })) >= 1, "{module}");
    let failing = function(&module, "Job.failing");
    assert!(count(failing, |kind| matches!(kind, InstKind::RegionUnwind { .. })) >= 1, "{module}");

    // A function value hides what it captures: once a value that may reach
    // an async destroy is captured (or boxed as `any`) anywhere, every body
    // owning a function value settles, the one capturing `Res` included.
    let captured = crate::module(&format!(
        "{prelude} object Job {{ Job(); \
         async res_closure() is int {{ let r = Res(n: 1); let f = () is bool {{ r.ok(); }}; if f() {{ 1; }} else {{ 0; }} }} \
         async conn_closure() is int {{ let c = Conn(n: 1); let f = () is bool {{ c.ok(); }}; if f() {{ 1; }} else {{ 0; }} }} \
         async boxed() is int {{ let a is any = Conn(n: 1); 1; }} \
         async plain() is int {{ let r = Res(n: 1); confirm r.ok(); 1; }} }} \
         async main() is int {{ await Job().res_closure(); }}"
    ));
    for symbol in ["Job.res_closure", "Job.conn_closure", "Job.boxed"] {
        assert!(settles(&captured, symbol), "{symbol} owns a value that may hide an async destroy:\n{captured}");
    }
    assert!(!settles(&captured, "Job.plain"), "{captured}");
}

#[test]
fn cold_future_tracks_adopts_and_keeps_retained_dependencies() {
    let module = module(r#"
        module app v1.0.0;
        object Res {
            Res();
            async read(arg is Res) is string { "ok"; }
            destroy {}
        }
        object Box {
            has cold is Future<string>;
            Box(:cold);
        }
        object Factory {
            Factory();
            make() is Future<string> { Res().read(Res()); }
            boxed() is Box { Box(cold: self.make()); }
        }
        main() is int { let cold = Factory().make(); let boxed = Factory().boxed(); 0; }
    "#);
    let make = function(&module, "Factory.make");
    let tracks: Vec<_> = make.blocks.iter().flat_map(|block| &block.insts).filter_map(|inst| {
        if let InstKind::RegionTrack { region, value } = inst.kind { Some((region, value)) } else { None }
    }).collect();
    // Receiver, argument and Future all belong to the same lexical region;
    // both normal closes keep the escaping result, including the root close.
    assert_eq!(tracks.len(), 4, "{module}");
    assert_ne!(tracks[3].0, tracks[0].0, "the root tracks the body result");
    assert_eq!(tracks[3].1, tracks[2].1);
    assert!(tracks[..3].iter().all(|(region, _)| *region == tracks[0].0), "{module}");
    assert!(matches!(make.values[tracks[2].1.0], ir::Type::Future(_)), "{module}");
    for inst in make.blocks.iter().flat_map(|block| &block.insts) {
        if let InstKind::RegionLeave { keep, .. } = inst.kind {
            assert!(keep.is_some_and(|value| matches!(make.values[value.0], ir::Type::Future(_))), "{module}");
        }
    }
    let main = function(&module, "main");
    assert_eq!(count(main, |kind| matches!(kind, InstKind::RegionAdopt { .. })), 2, "{module}");
    // A wrapper with only a Future field is not plain either.
    let boxed = function(&module, "Factory.boxed");
    assert!(count(boxed, |kind| matches!(kind, InstKind::RegionTrack { .. })) >= 2, "{module}");
}

#[test]
fn future_ownership_corpus_lowers_with_stdlib() {
    use okkama_compiler::lowering::{LoadedModule, ModuleLoader, lower_linked};
    use std::path::PathBuf;

    struct CorpusLoader(PathBuf);
    impl ModuleLoader for CorpusLoader {
        fn load(&mut self, path: &str, _importer: &str) -> Result<Option<LoadedModule>, String> {
            let file = ["std", "t/support"].into_iter()
                .map(|root| self.0.join(root).join(format!("{path}.ok")))
                .find(|file| file.is_file());
            let Some(file) = file else { return Ok(None); };
            let text = std::fs::read_to_string(&file).map_err(|error| error.to_string())?;
            let id = file.to_string_lossy().into_owned();
            let source = SourceFile::new(id.clone(), text).unwrap();
            let program = syntax::parse(&source, &ParseOptions::default()).unwrap();
            Ok(Some(LoadedModule { id, source, program: prune::strip_test_items(program) }))
        }
    }

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for file in ["cold-futures", "future-completions", "async-results", "all-futures", "channel-transfer", "raced", "async-destroy"] {
        let path = root.join(format!("tests/ownership-corpus/{file}.ok"));
        let source = SourceFile::new(path.to_string_lossy(), std::fs::read_to_string(&path).unwrap()).unwrap();
        let program = syntax::parse(&source, &ParseOptions::default()).unwrap();
        let module = lower_linked(&program, source.name(), &mut CorpusLoader(root.clone())).unwrap();
        ir::verify(&module).unwrap();
    }

    // Compile the real signals fixture without opening sockets or running it.
    // Both HTTP and HTTPS server factories must take the listener before
    // returning it inside HttpServer, rather than leaving it in a dying cache.
    let path = root.join("okkama-rs/crates/okkama-cli/tests/signals/serve.ok");
    let source = SourceFile::new(path.to_string_lossy(), std::fs::read_to_string(&path).unwrap()).unwrap();
    let program = syntax::parse(&source, &ParseOptions::default()).unwrap();
    let module = lower_linked(&program, source.name(), &mut CorpusLoader(root)).unwrap();
    ir::verify(&module).unwrap();
    let mut listeners = 0;
    for function in module.functions.iter().filter(|function| function.symbol.contains("WebApplication.server")) {
        let insts: Vec<_> = function.blocks.iter().flat_map(|block| &block.insts).collect();
        for inst in &insts {
            if let InstKind::Call { func, .. } = &inst.kind
                && module.functions[func.0].symbol.ends_with("TcpListening.listener")
            {
                let future = inst.result.unwrap();
                assert!(insts.iter().any(|inst| matches!(inst.kind, InstKind::AwaitTake(value) if value == future)));
                listeners += 1;
            }
        }
    }
    assert_eq!(listeners, 2, "HTTP and HTTPS factories take their listeners");
}

#[test]
fn future_outcome_await_tracking_preserves_future_owner() {
    let module = module(&format!(
        "{RES} object Work {{ Work(); async make() is Res {{ Res(n: 1); }} }} \
         async main() is int {{ let f = Work().make(); {{ await f; }} await Work().make(); 0; }}"
    ));
    let main = function(&module, "main");
    let instructions: Vec<_> = main.blocks.iter().flat_map(|block| &block.insts).collect();
    let borrowed = instructions.iter().find(|inst| matches!(inst.kind, InstKind::Await(_)))
        .expect("bound Future borrows").result.unwrap();
    let taken = instructions.iter().find(|inst| matches!(inst.kind, InstKind::AwaitTake(_)))
        .expect("temporary Future takes").result.unwrap();
    assert!(!instructions.iter().any(|inst| matches!(inst.kind,
        InstKind::RegionTrack { value, .. } if value == borrowed)), "{module}");
    assert_eq!(instructions.iter().filter(|inst| matches!(inst.kind,
        InstKind::RegionTrack { value, .. } if value == taken)).count(), 1, "{module}");
}

#[test]
fn only_fresh_async_invocations_take_the_cache() {
    let module = module(&format!(
        "{RES} object Work {{ Work(); async make() is Res {{ Res(n: 1); }} \
         alias(f is Future<Res>) is Future<Res> {{ f; }} }} \
         async main() is int {{ let w = Work(); let f = w.make(); \
         await f; await f; await w.alias(f); let fs = [f]; await fs.item(0); \
         await w.make(); let action is async () is Res = w.make; await action(); 0; }}"
    ));
    let main = function(&module, "main");
    assert_eq!(count(main, |kind| matches!(kind, InstKind::Await(_))), 4, "{module}");
    assert_eq!(count(main, |kind| matches!(kind, InstKind::AwaitTake(_))), 2, "{module}");
    let text = module.to_string();
    assert!(text.contains("await.take "), "{text}");
    let program = okkama_compiler::codegen::sparrow::generate(&module).unwrap();
    let bytes = okkama_sparrow::encode(&program).unwrap();
    assert_eq!(okkama_sparrow::decode(&bytes).unwrap(), program);
    assert_eq!(okkama_sparrow::assemble(&okkama_sparrow::disassemble(&program)).unwrap(), program);
}

#[test]
fn borrowed_async_outcome_cleanup_settles_the_future_owner() {
    let module = module("module app v1.0.0; \
        object Res { Res(); async destroy {} } \
        object Work { Work(); async make() is Res { Res(); } } \
        async main() is int { let f = Work().make(); { await f; } 0; }");
    assert!(settles(&module, "main"), "{module}");
    assert_eq!(count(function(&module, "main"), |kind| matches!(kind, InstKind::Await(_))), 1);
}
