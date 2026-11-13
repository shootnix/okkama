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
    assert!(has_reason(function(&report, "Escape.invoke"), "indirect-call"));
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
fn roles_union_implementations_with_different_summaries() {
    let report = census(&format!("{RES} role Choose {{ pick(r is Res) is Res; }} \
        object Alias does Choose {{ Alias(); pick(r is Res) is Res {{ r; }} }} \
        object Fresh does Choose {{ Fresh(); pick(r is Res) is Res {{ Res(n: 2); }} }} \
        object Use {{ Use(); run(c is Choose) is Res {{ c.pick(Res(n: 1)); }} }} \
        main() is int {{ 0; }}"));
    let run = function(&report, "Use.run");
    assert_eq!(run.totals.unknown, 0, "{run:?}");
    assert_eq!(run.creations.len(), 2);
    assert!(run.creations.iter().all(|site| site.owner == Owner::Result));
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
    assert!(has_reason(function(&report, "Use.run"), "child"));
    assert_eq!(function(&report, "Use.run").totals.result, 0);
}

#[test]
fn async_frames_and_settle_keep_use_ordinary_scoping() {
    let report = census("module app v1.0.0; object Conn { Conn(); async destroy {} } \
        object Job { Job(); async run() is Conn { Conn(); } } main() is int { 0; }");
    let run = function(&report, "Job.run");
    assert_eq!(run.totals.unknown, 0, "{run:?}");
    assert_eq!(run.creations[0].owner, Owner::Result);
    assert!(run.region_ops.iter().any(|op| op.kind == "regionsettle" && op.statically_owned));
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
    let next = function(&report, "Holder.next");
    assert_eq!(next.totals.unknown, 0, "{next:?}");
    assert!(next.parameters[0].result_part);
    assert!(next.creations.iter().any(|site| site.kind == "objnext" && site.owner == Owner::Result));
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
    assert!(has_reason(function(&report, "main"), "native:test_resource"));
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

#[test]
fn local_field_reads_borrow_without_escaping_the_container() {
    let report = census(&format!("{RES} object Box {{ has item is Res; Box(:item); \
        inspect() is int {{ item.read(); }} item() is Res {{ item; }} }} \
        main() is int {{ let b = Box(item: Res(n: 1)); b.item().read(); b.inspect(); }}"));
    let main = function(&report, "main");
    assert_eq!(main.totals.unknown, 0, "{main:?}");
    assert_eq!(main.creations.last().unwrap().owner, Owner::Scope);
    assert!(!function(&report, "Box.inspect").parameters[0].escapes());
    assert!(function(&report, "Box.item").parameters[0].result_part);
    assert!(!function(&report, "Box.item").parameters[0].result);
}

#[test]
fn collection_copies_borrow_contents_and_preserve_source_owners() {
    let report = census(&format!("{RES} object Holder {{ has r is Res; Holder(:r); \
        next() is ^Holder {{ ^r = r; }} read() is int {{ r.read(); }} }} main() is int {{ \
        let a = [Res(n: 1)]; let slice = a.slice(0, 1); let joined = a + slice; \
        let m = [\"x\": Res(n: 2)]; let merged = m + m; \
        let h = Holder(r: Res(n: 3)); let successor = h.next(); successor.read(); 0; }}"));
    let main = function(&report, "main");
    assert_eq!(main.totals.unknown, 0, "{main:?}");
    for kind in ["arraynew", "arrayslice", "arrayconcat", "mapnew", "mapmerge"] {
        let site = main.creations.iter().find(|site| site.kind == kind).unwrap();
        assert_eq!(site.owner, Owner::Scope, "{site:?}");
        if matches!(kind, "arrayslice" | "arrayconcat" | "mapmerge") {
            assert!(!site.borrowed_contents.is_empty(), "{site:?}");
        }
    }
    let copies: Vec<_> = main.creations.iter().filter(|site| matches!(site.kind.as_str(),
        "arrayslice" | "arrayconcat" | "mapmerge")).map(|site| site.value).collect();
    assert!(main.creations.iter().all(|site| match &site.owner {
        Owner::Aggregate { containers, .. } => containers.iter().all(|id| !copies.contains(id)),
        _ => true,
    }));
    let next = function(&report, "Holder.next");
    let successor = next.creations.iter().find(|site| site.kind == "objnext").unwrap();
    assert_eq!(successor.owner, Owner::Result);
    assert!(!successor.borrowed_contents.is_empty());
}

#[test]
fn caps_apply_the_computed_fields_owner_summary() {
    let report = census(&format!("{RES} object Box {{ has item is Res; Box(:item); \
        cap View {{ item is Res {{ item; }} count is int {{ item.read(); }} }} }} \
        main() is int {{ let b = Box(item: Res(n: 1)); let cap = b::View; \
        cap::item.read(); cap::count; }}"));
    let main = function(&report, "main");
    assert_eq!(main.totals.unknown, 0, "{main:?}");
    assert_eq!(main.creations.last().unwrap().owner, Owner::Scope);
}

#[test]
fn direct_async_await_applies_the_callee_summary() {
    let report = census(&format!("{RES} object Job {{ Job(); \
        async read(r is Res) is int {{ r.read(); }} \
        async alias(r is Res) is Res {{ r; }} \
        async make() is Res {{ Res(n: 2); }} \
        async local() is int {{ let r = Res(n: 1); await self.read(r); }} \
        async returned() is Res {{ await self.alias(Res(n: 3)); }} \
        async fresh() is Res {{ await self.make(); }} }} main() is int {{ 0; }}"));
    for symbol in ["Job.local", "Job.returned", "Job.fresh"] {
        let found = function(&report, symbol);
        assert_eq!(found.totals.unknown, 0, "{found:?}");
    }
    assert!(function(&report, "Job.local").creations.iter().all(|site| !matches!(site.owner, Owner::Result)));
    assert!(function(&report, "Job.returned").creations.iter().any(|site| site.owner == Owner::Result));
    assert!(function(&report, "Job.fresh").creations.iter().any(|site| site.kind == "await.result" && site.owner == Owner::Result));
}

#[test]
fn local_closures_borrow_captures_and_resolve_direct_calls() {
    let report = census(&format!("{RES} object Use {{ Use(); \
        ignore(f is () is int) is int {{ 0; }} }} main() is int {{ \
        let r = Res(n: 1); let f = () is int {{ r.read(); }}; \
        f(); Use().ignore(f); 0; }}"));
    let main = function(&report, "main");
    assert_eq!(main.totals.unknown, 0, "{main:?}");
    assert!(main.creations.iter().any(|site| site.kind == "fnclosure" && site.owner == Owner::Scope));
    assert!(main.creations.iter().all(|site| site.owner == Owner::Scope), "{main:?}");
}

#[test]
fn escaped_read_results_stay_child_through_stores_and_sinks() {
    let report = census(&format!("{RES} object Box {{ has r is Res; Box(:r); item() is Res {{ r; }} }} \
        object Use {{ Use(); stored() is @Res {{ let b = Box(r: Res(n: 1)); [b.item()]; }} \
        sunk() is int {{ let b = Box(r: Res(n: 2)); let boxed is any = b.item(); 0; }} \
        copied() is @Res {{ let a = [Res(n: 3)]; a.slice(0, 1); }} }} main() is int {{ 0; }}"));
    for symbol in ["Use.stored", "Use.sunk", "Use.copied"] {
        let found = function(&report, symbol);
        assert!(has_reason(found, "child"), "{found:?}");
        assert!(found.creations.iter().filter(|site| site.kind == "call.result")
            .all(|site| matches!(&site.owner, Owner::Unknown(reasons) if reasons.contains("child"))));
    }
}

#[test]
fn lookup_terminators_borrow_on_both_local_and_returned_paths() {
    let report = census(&format!("{RES} object Use {{ Use(); \
        array() is Res {{ let a = [Res(n: 1)]; a.first(); }} \
        map() is Res {{ let m = [\"x\": Res(n: 2)]; m[\"x\"] else Res(n: 3); }} }} \
        main() is int {{ let a = [Res(n: 1)]; a.first().read(); \
        let m = [\"x\": Res(n: 2)]; let r = m[\"x\"] else Res(n: 3); r.read(); }}"));
    assert_eq!(function(&report, "main").totals.unknown, 0);
    for symbol in ["Use.array", "Use.map"] {
        assert!(has_reason(function(&report, symbol), "child"));
    }
}

#[test]
fn future_storage_return_and_spawn_keep_retained_resources_unknown() {
    let report = census(&format!("{RES} object Job {{ Job(); \
        async read(r is Res) is int {{ r.read(); }} \
        returned() is Future<int> {{ self.read(Res(n: 1)); }} \
        stored() is int {{ let futures = [self.read(Res(n: 2))]; 0; }} \
        passed() is int {{ self.ignore(self.read(Res(n: 3))); }} \
        ignore(f is Future<int>) is int {{ 0; }} \
        async spawned() is int {{ let f = spawn Job().read(Res(n: 4)); await f; }} }} \
        main() is int {{ 0; }}"));
    for symbol in ["Job.returned", "Job.stored", "Job.passed"] {
        assert!(has_reason(function(&report, symbol), "future"));
    }
    assert!(has_reason(function(&report, "Job.spawned"), "spawn"));
    assert!(!report.totals.reasons.contains_key("async"));
}

#[test]
fn resolved_closures_apply_capture_and_argument_effects() {
    let report = census(&format!("{RES} object Use {{ Use(); \
        alias() is Res {{ let r = Res(n: 1); let f = (v is Res) is Res {{ r.read(); v; }}; f(Res(n: 2)); }} \
        sink() is int {{ let r = Res(n: 3); let f = () is any {{ r; }}; f(); 0; }} }} \
        main() is int {{ 0; }}"));
    let alias = function(&report, "Use.alias");
    assert_eq!(alias.totals.unknown, 0, "{alias:?}");
    assert_eq!(alias.totals.result, 1);
    assert!(has_reason(function(&report, "Use.sink"), "any"));
}

#[test]
fn json_splits_genuine_reasons_from_named_model_gaps() {
    let mut module = module(&format!("{RES} object Use {{ Use(); boxed() is any {{ Res(n: 1); }} }} \
        main() is int {{ let r = Res(n: 2); 0; }}"));
    let ctor = module.functions.iter_mut().find(|function| function.symbol == "Res.new#0").unwrap();
    ctor.blocks.clear();
    ctor.attrs.push(ir::Attr::Extern("test_resource".to_string()));
    ir::verify(&module).unwrap();
    let report = ownership::analyze(&module);
    let json = report.to_json();
    assert!(json.contains(&format!("\"reasons_genuine\":{{\"any\":{}}}", report.totals.reasons["any"])));
    assert!(json.contains(&format!("\"reasons_model\":{{\"native:test_resource\":{}}}", report.totals.reasons["native:test_resource"])));
    assert!(json.contains("\"schema\":1"));
    assert!(json.contains("\"borrowed_contents\":"));
    assert!(json.contains("\"result_part\":"));
}

#[test]
fn role_summary_joins_sinks_and_requires_visible_implementations() {
    let text = format!("{RES} role Read {{ read(r is Res) is int; }} \
        object First does Read {{ First(); read(r is Res) is int {{ r.read(); }} }} \
        object Second does Read {{ Second(); read(r is Res) is int {{ let box is any = r; 0; }} }} \
        object Use {{ Use(); run(c is Read) is int {{ c.read(Res(n: 1)); }} }} main() is int {{ 0; }}");
    let mut module = module(&text);
    let report = ownership::analyze(&module);
    assert!(has_reason(function(&report, "Use.run"), "any"));
    assert!(!has_reason(function(&report, "Use.run"), "role-call"));
    for object in &mut module.objects { object.does.clear(); }
    ir::verify(&module).unwrap();
    let report = ownership::analyze(&module);
    assert!(has_reason(function(&report, "Use.run"), "role-call"));
}

#[test]
fn intrinsic_model_gaps_include_the_exact_intrinsic_name() {
    let module = linked("import \"compare\" take Comparing, Order;",
        "object Ranked does Comparing<Ranked> { Ranked(); \
         order(other is Ranked) is Order { Order.equal; } destroy {} } \
         main() is int { let sorted = [Ranked(), Ranked()].ordering(); 0; }");
    let report = ownership::analyze(&module);
    assert!(has_reason(function(&report, "main"), "intrinsic:okkama.array.ordering"));
    assert!(!report.totals.reasons.contains_key("native"));
}

#[test]
fn replacement_removal_difference_and_builder_copies_do_not_sink_sources() {
    let report = census(&format!("{RES} main() is int {{ \
        let r = Res(n: 1); let a = [r]; let b = a.replaced(0, Res(n: 2)); \
        let c = a.removed_at(0); let d = a - [r]; \
        let m = [\"r\": r]; let n = m - [\"r\": r]; let values = m.values(); \
        let reversed = a.reversed(); let combined is @Res = (0..2).reduce([]) -> acc, i {{ acc + a; }}; 0; }}"));
    let main = function(&report, "main");
    assert_eq!(main.totals.unknown, 0, "{main:?}");
    for kind in ["arrayreplace", "arrayremove", "arraydifference", "mapdifference", "mapvalues"] {
        assert!(main.creations.iter().any(|site| site.kind == kind && !site.borrowed_contents.is_empty()), "{kind}: {main:?}");
    }
}

#[test]
fn observing_a_future_and_spawning_workers_stays_dynamic() {
    let report = census(&format!("{RES} object Job {{ Job(); async read(r is Res) is int {{ r.read(); }} \
        observe() is int {{ let f = self.read(Res(n: 1)); f.done(); 0; }} }} \
        async main() is int {{ await spawn Res(n: 2).read(); }}"));
    assert!(has_reason(function(&report, "Job.observe"), "future"));
    assert!(has_reason(function(&report, "main"), "spawn"));
}

#[test]
fn repeated_awaits_share_identity_and_escaped_futures_taint_results() {
    let report = census(&format!("{RES} object Job {{ Job(); async make() is Res {{ Res(n: 1); }} \
        async repeated() is Res {{ let f = self.make(); let a = await f; a.read(); await f; }} \
        async stored() is Res {{ let f = self.make(); let futures = [f]; await f; }} }} \
        main() is int {{ 0; }}"));
    let repeated = function(&report, "Job.repeated");
    assert_eq!(repeated.totals.unknown, 0, "{repeated:?}");
    let results: Vec<_> = repeated.creations.iter().filter(|site| site.kind == "await.result").collect();
    assert_eq!(results.len(), 1, "{repeated:?}");
    assert_eq!(results[0].owner, Owner::Result);
    let stored = function(&report, "Job.stored");
    assert!(stored.creations.iter().filter(|site| site.kind == "await.result")
        .all(|site| matches!(&site.owner, Owner::Unknown(reasons) if reasons.contains("future"))), "{stored:?}");
}
