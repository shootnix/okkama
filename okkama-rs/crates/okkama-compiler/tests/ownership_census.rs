//! Read-only ownership census (#553); these tests lower but never execute Okkama.

use okkama_compiler::SourceFile;
use okkama_compiler::ir::{self, InstKind};
use okkama_compiler::ir::ownership::{self, AggregateOwner, FunctionReport, Owner, Report};
use okkama_compiler::lowering::{self, LoadedModule, LoweringError, ModuleLoader, lower_program};
use okkama_compiler::syntax::{self, ParseOptions, prune};

const RES: &str = "module app v1.0.0; object Res { has n is int; Res(:n); \
    read() is int { n; } destroy { confirm n > 0; } }";

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

fn census(text: &str) -> Report {
    let module = module(text);
    let before = module.clone();
    let report = ownership::analyze(&module);
    assert_eq!(module, before, "analysis must be read-only");
    assert_eq!(report, ownership::analyze(&module), "deterministic report");
    report
}

fn function<'a>(report: &'a Report, symbol: &str) -> &'a FunctionReport {
    report.functions.iter().find(|function| function.symbol == symbol).expect(symbol)
}

fn has_reason(function: &FunctionReport, reason: &str) -> bool {
    function.creations.iter().any(|site| matches!(&site.owner, Owner::Unknown(reasons) if reasons.contains(reason)))
}

#[test]
fn local_object_is_scope_and_constructor_allocation_is_result() {
    let report = census(&format!("{RES} main() is int {{ let r = Res(n: 1); r.read(); }}"));
    let main = function(&report, "main");
    assert_eq!(main.creations.len(), 1);
    assert_eq!(main.creations[0].owner, Owner::Scope);
    assert!(main.region_ops.iter().all(|op| op.statically_owned));
    assert!(main.totals.region_static > 0);
    assert_eq!(function(&report, "Res.new#0").creations[0].owner, Owner::Result);
}

#[test]
fn returning_a_resource_and_an_array_distinguishes_owners() {
    let report = census(&format!("{RES} object Factory {{ Factory(); \
        one() is Res {{ Res(n: 1); }} many() is @Res {{ [Res(n: 2)]; }} }} \
        main() is int {{ 0; }}"));
    assert_eq!(function(&report, "Factory.one").creations[0].owner, Owner::Result);
    let many = function(&report, "Factory.many");
    assert!(many.creations.iter().any(|site| site.kind == "call.result" && matches!(site.owner,
        Owner::Aggregate { owner: AggregateOwner::Result, .. })), "{many:?}");
    assert!(many.creations.iter().any(|site| site.kind == "arraynew" && site.owner == Owner::Result));
}

#[test]
fn action_summaries_distinguish_reading_aliasing_and_storing() {
    let report = census(&format!("{RES} object Box {{ has item is Res; Box(:item); }} \
        object Actions {{ Actions(); read(r is Res) is int {{ r.read(); }} \
        alias(r is Res) is Res {{ r; }} store(r is Res) is Box {{ Box(item: r); }} \
        local_store(r is Res) is int {{ let b = Box(item: r); 0; }} \
        local() is int {{ self.read(Res(n: 1)); }} \
        returned() is Box {{ self.store(Res(n: 2)); }} \
        identity() is Res {{ self.alias(Res(n: 3)); }} }} main() is int {{ 0; }}"));
    assert!(!function(&report, "Actions.read").parameters[1].escapes());
    assert!(!function(&report, "Actions.local_store").parameters[1].escapes());
    assert!(function(&report, "Actions.alias").parameters[1].result);
    assert!(function(&report, "Actions.store").parameters[1].result_field);
    assert_eq!(function(&report, "Actions.local").creations[0].owner, Owner::Scope);
    assert!(function(&report, "Actions.returned").creations.iter().any(|site| matches!(site.owner,
        Owner::Aggregate { owner: AggregateOwner::Result, .. })));
    let identity = function(&report, "Actions.identity");
    assert_eq!(identity.creations.len(), 1, "an identity call is not a creation");
    assert_eq!(identity.creations[0].owner, Owner::Result);
    assert!(function(&report, "Actions.alias").region_ops.iter().any(|op| !op.statically_owned));
}

#[test]
fn mutual_recursion_reaches_a_fixed_point() {
    let report = census(&format!("{RES} object Cycle {{ Cycle(); \
        left(r is Res, n is int) is Res {{ if n > 0 {{ self.right(r, n - 1); }} else {{ r; }} }} \
        right(r is Res, n is int) is Res {{ self.left(r, n); }} \
        make(n is int) is Res {{ self.right(Res(n: 1), n); }} }} main() is int {{ 0; }}"));
    for symbol in ["Cycle.left", "Cycle.right"] {
        let parameter = &function(&report, symbol).parameters[1];
        assert!(parameter.result, "{symbol}: {parameter:?}");
        assert!(parameter.reasons.is_empty());
    }
    assert_eq!(function(&report, "Cycle.make").creations[0].owner, Owner::Result);
}

#[test]
fn captured_and_boxed_resources_are_unknown() {
    let report = census(&format!("{RES} object Escape {{ Escape(); \
        closure() is () is Res {{ let r = Res(n: 1); () is Res {{ r; }}; }} \
        boxed() is any {{ Res(n: 2); }} invoke(f is () is Res) is Res {{ f(); }} }} main() is int {{ 0; }}"));
    assert!(has_reason(function(&report, "Escape.closure"), "closure"));
    assert!(has_reason(function(&report, "Escape.boxed"), "any"));
    assert!(has_reason(function(&report, "Escape.invoke"), "closure"));
    assert!(function(&report, "Escape.closure").creations.iter().all(|site| matches!(site.owner, Owner::Unknown(_))));
}

#[test]
fn plain_objects_and_programs_without_destroy_have_no_sites() {
    let plain = "object Plain { has n is int; Plain(:n); } main() is int { let p = Plain(n: 1); 0; }";
    let report = census(&format!("module app v1.0.0; {plain}"));
    assert!(report.functions.iter().all(|function| function.creations.is_empty()));
    assert_eq!(report.totals, ownership::Totals::default());
    let boxed = census("module app v1.0.0; object Box { Box(); value() is any { 1; } } main() is int { Box().value(); 0; }");
    assert_eq!(boxed.totals, ownership::Totals::default());
    let report = census(&format!("{RES} {plain}"));
    assert!(function(&report, "main").creations.is_empty());
    assert!(function(&report, "Plain.new#0").creations.is_empty());
}

struct Std;

impl ModuleLoader for Std {
    fn load(&mut self, path: &str, _importer: &str) -> Result<Option<LoadedModule>, String> {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std").join(format!("{path}.ok"));
        let text = std::fs::read_to_string(&file).map_err(|error| format!("{}: {error}", file.display()))?;
        let source = SourceFile::new(file.to_string_lossy(), text).expect("std source");
        let program = syntax::parse(&source, &ParseOptions::default()).map_err(|error| format!("{error:?}"))?;
        Ok(Some(LoadedModule {
            id: source.name().to_string(), source, program: prune::strip_test_items(program),
        }))
    }
}

fn linked(imports: &str, body: &str) -> ir::Module {
    let text = format!("module app v1.0.0; {imports} {} {body}", RES.strip_prefix("module app v1.0.0;").unwrap());
    let source = SourceFile::new("test.ok", text).expect("small source");
    let program = syntax::parse(&source, &ParseOptions::default()).expect("valid syntax");
    let module = lowering::lower_linked(&prune::strip_test_items(program), source.name(), &mut Std).expect("linked lowering");
    ir::verify(&module).expect("linked output verifies");
    module
}

#[test]
fn atomic_storage_uses_the_real_std_intrinsic() {
    let module = linked("import \"async/atomic\" take Atomic;",
        "main() is int { let r = Res(n: 1); let cell = Atomic(r); 0; }");
    let report = ownership::analyze(&module);
    assert!(has_reason(function(&report, "main"), "atomic"));
    assert!(module.functions.iter().any(|function| function.intrinsic().is_some_and(|(name, _)| name == "okkama.atomic.Atomic.new_value")));
}

#[test]
fn channel_send_uses_the_real_std_intrinsic() {
    let module = linked("import \"async/channel\" take Channel;",
        "async main() is int { let channel is Channel<Res> = Channel(capacity: 1); \
         let r = Res(n: 1); await channel.sending().send(r); 0; }");
    let report = ownership::analyze(&module);
    assert!(has_reason(function(&report, "main"), "channel"));
}

#[test]
fn implicit_destroy_calls_cannot_hide_an_escape() {
    let module = linked("import \"async/atomic\" take Atomic;",
        "object Dangerous { Dangerous(); destroy { let cell = Atomic(self); } } \
         main() is int { let r = Dangerous(); 0; }");
    let before = module.clone();
    let report = ownership::analyze(&module);
    assert_eq!(module, before);
    assert!(has_reason(function(&report, "main"), "atomic"));
}

#[test]
fn recursive_sinks_propagate_to_callers() {
    let report = census(&format!("{RES} object Cycle {{ Cycle(); \
        left(r is Res, n is int) is any {{ if n > 0 {{ self.right(r, n - 1); }} else {{ r; }} }} \
        right(r is Res, n is int) is any {{ self.left(r, n); }} \
        make() is int {{ self.right(Res(n: 1), 2); 0; }} }} main() is int {{ 0; }}"));
    for symbol in ["Cycle.left", "Cycle.right"] {
        assert!(function(&report, symbol).parameters[1].reasons.contains("any"));
    }
    assert!(has_reason(function(&report, "Cycle.make"), "any"));
}

#[test]
fn roles_union_implementations_and_flag_disagreement() {
    let report = census(&format!("{RES} role Choose {{ pick(r is Res) is Res; }} \
        object Alias does Choose {{ Alias(); pick(r is Res) is Res {{ r; }} }} \
        object Fresh does Choose {{ Fresh(); pick(r is Res) is Res {{ Res(n: 2); }} }} \
        object Use {{ Use(); run(c is Choose) is Res {{ c.pick(Res(n: 1)); }} }} \
        main() is int {{ 0; }}"));
    assert!(has_reason(function(&report, "Use.run"), "role-call"));
}

#[test]
fn agreeing_role_readers_do_not_escape_arguments() {
    let report = census(&format!("{RES} role Read {{ read(r is Res) is int; }} \
        object First does Read {{ First(); read(r is Res) is int {{ r.read(); }} }} \
        object Second does Read {{ Second(); read(r is Res) is int {{ r.read() + 1; }} }} \
        object Use {{ Use(); run(c is Read) is int {{ c.read(Res(n: 1)); }} }} \
        main() is int {{ 0; }}"));
    assert_eq!(function(&report, "Use.run").creations[0].owner, Owner::Scope);
}

#[test]
fn managed_projections_do_not_claim_the_parent_escapes() {
    let report = census(&format!("{RES} object Box {{ has item is Res; Box(:item); \
        item() is Res {{ item; }} }} object Use {{ Use(); \
        run() is Res {{ Box(item: Res(n: 1)).item(); }} }} main() is int {{ 0; }}"));
    assert!(has_reason(function(&report, "Use.run"), "other:objget"));
    assert_eq!(function(&report, "Use.run").totals.result, 0);
}

#[test]
fn async_frames_and_settle_keep_are_conservative() {
    let report = census("module app v1.0.0; object Conn { Conn(); async destroy {} } \
        object Job { Job(); async run() is Conn { Conn(); } } main() is int { 0; }");
    let run = function(&report, "Job.run");
    assert!(has_reason(run, "async"));
    assert!(run.region_ops.iter().any(|op| op.kind == "regionsettle" && !op.statically_owned));
}

#[test]
fn variants_maps_successors_and_builders_preserve_contents() {
    let report = census(&format!("{RES} enum Item = (some(value is Res), none); \
        object Holder {{ has r is Res; Holder(:r); next() is ^Holder {{ ^r = Res(n: 2); }} }} \
        object Make {{ Make(); variant() is Item {{ Item.some(Res(n: 1)); }} \
        map() is %string:Res {{ [\"r\": Res(n: 1)]; }} \
        array() is @Res {{ (0..2).map() -> n {{ Res(n: n + 1); }}; }} }} main() is int {{ 0; }}"));
    for symbol in ["Make.variant", "Make.map", "Make.array"] {
        let found = function(&report, symbol);
        assert!(found.creations.iter().any(|site| matches!(site.owner,
            Owner::Aggregate { owner: AggregateOwner::Result, .. })), "{found:?}");
        assert_eq!(found.totals.unknown, 0, "{found:?}");
    }
    assert!(has_reason(function(&report, "Holder.next"), "other:objnext"));
}

#[test]
fn native_managed_results_are_unknown() {
    let mut module = module(&format!("{RES} main() is int {{ let r = Res(n: 1); 0; }}"));
    // A valid opaque OIR constructor, without loading or invoking a native.
    let ctor = module.functions.iter_mut().find(|function| function.symbol == "Res.new#0").unwrap();
    ctor.blocks.clear();
    ctor.attrs.push(ir::Attr::Extern("test_resource".to_string()));
    ir::verify(&module).expect("opaque constructor verifies");
    let report = ownership::analyze(&module);
    assert!(has_reason(function(&report, "main"), "native"));
    assert_eq!(function(&report, "main").totals.scope, 0);
}

#[test]
fn all_requested_region_kinds_are_reported() {
    let module = module(&format!("{RES} object Box {{ has r is Res; Box(:r); }} \
        main() is int {{ let r = Res(n: 1); let b = Box(r: r); 0; }}"));
    let report = ownership::analyze(&module);
    let expected = module.functions.iter().flat_map(|function| &function.blocks)
        .flat_map(|block| &block.insts).filter(|inst| matches!(inst.kind,
            InstKind::RegionAdopt { .. } | InstKind::RegionTrack { .. } | InstKind::RegionTransfer { .. }
            | InstKind::RegionLeave { keep: Some(_), .. } | InstKind::RegionSettle { keep: Some(_), .. }
        )).count();
    assert_eq!(report.totals.region_static + report.totals.region_dynamic, expected);
    assert!(report.totals.region_static > 0 && report.totals.region_dynamic > 0);
    assert!(report.to_json().contains("\"top_unknown\":"));
}

#[test]
fn leave_with_known_keep_still_checks_other_region_contents() {
    let module = linked("import \"async/atomic\" take Atomic;",
        "object Use { Use(); run() is Res { let risky = Res(n: 1); \
         let cell = Atomic(risky); Res(n: 2); } } main() is int { 0; }");
    let report = ownership::analyze(&module);
    let run = function(&report, "Use.run");
    assert!(has_reason(run, "atomic"));
    assert!(run.creations.iter().any(|site| site.owner == Owner::Result));
    assert!(run.region_ops.iter().any(|op| op.kind == "regionleave" && op.values.len() > 1 && !op.statically_owned));
}

#[test]
fn branch_arguments_preserve_all_possible_origins() {
    let report = census(&format!("{RES} object Choose {{ Choose(); \
        run(flag is bool) is Res {{ let r = if flag {{ Res(n: 1); }} else {{ Res(n: 2); }}; r; }} }} \
        main() is int {{ 0; }}"));
    let run = function(&report, "Choose.run");
    assert_eq!(run.creations.len(), 2);
    assert!(run.creations.iter().all(|site| site.owner == Owner::Result));
}
