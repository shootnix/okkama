//! Successor создаёт новое значение, не меняя исходник и его алиасы.
use okkama_compiler::{SourceFile, ir, lowering, syntax};
use okkama_runtime::{RuntimeError, execute};

const COUNTER: &str = r#"
object Counter {
    has n is int;
    has previous is int = 0;
    Counter(:n);
    inc() is ^Counter { ^n = n + 1; ^previous = n; }
    staged() is ^Counter { ^n = n + 1; ^n = n + 10; ^previous = n; }
    maybe(flag is bool) is ^Counter { if flag { ^n = n + 5; } }
    value() is int { n; }
    before() is int { previous; }
}
"#;

fn lower(decls: &str, body: &str) -> ir::Module {
    let source = SourceFile::new(
        "successors.ok",
        format!("module app v1.0.0; {decls} main() is int {{ {body} }}"),
    )
    .unwrap();
    let ast = syntax::parse(&source, &syntax::ParseOptions::default()).unwrap();
    lowering::lower_program(&ast).unwrap()
}

fn result(module: &okkama_compiler::ir::Module) -> okkama_compiler::ir::IntValue {
    let result = execute(module).unwrap();
    let program = okkama_compiler::codegen::sparrow::generate(module).unwrap();
    let bytes = okkama_sparrow::encode(&program).unwrap();
    assert_eq!(
        okkama_runtime::sparrow::execute(&okkama_sparrow::decode(&bytes).unwrap()).unwrap(),
        result
    );
    result
}

#[test]
fn old_aliases_and_chained_successors_keep_distinct_values() {
    let module = lower(
        COUNTER,
        r#"
        let a = Counter(n: 10);
        let alias = a;
        a = a.inc();
        let b = a.inc().inc();
        let bn = b.value();
        if alias.value() == 10 && a.value() == 11 && bn == 13 {
            42;
        } else { 0; }
    "#,
    );
    assert_eq!(result(&module).to_string(), "42");
}

#[test]
fn staged_updates_read_original_fields_and_keep_the_last_assignment() {
    let module = lower(
        COUNTER,
        r#"
        let a = Counter(n: 3);
        let b = a.staged();
        let bn = b.value();
        if a.value() == 3 && bn == 13 && b.before() == 3 { 42; } else { 0; }
    "#,
    );
    assert_eq!(result(&module).to_string(), "42");
}

#[test]
fn rebinding_across_loops_and_branches_preserves_unmodified_fields() {
    let module = lower(
        COUNTER,
        r#"
        let c = Counter(n: 0);
        for 1..=5 -> i { c = c.inc(); }
        c = c.maybe(false);
        if c.value() == 5 { c = c.maybe(true); }
        if c.value() == 10 && c.before() == 4 { 42; } else { 0; }
    "#,
    );
    assert_eq!(result(&module).to_string(), "42");
}

#[test]
fn successors_preserve_and_replace_nested_object_references() {
    let decls = format!(
        r#"{COUNTER}
        object Box {{
            has inner is Counter;
            has version is int = 0;
            Box(:inner);
            bump() is ^Box {{ ^version = version + 1; }}
            replace(replacement is Counter) is ^Box {{ ^inner = replacement; }}
            get() is Counter {{ inner; }}
            revision() is int {{ version; }}
        }}
    "#
    );
    let module = lower(
        &decls,
        r#"
        let old = Box(inner: Counter(n: 7));
        let bumped = old.bump();
        let changed = bumped.replace(Counter(n: 9));
        let changed_value = changed.get().value();
        if old.get().value() == 7 && bumped.get().value() == 7
            && changed_value == 9 && changed.revision() == 1 {
            42;
        } else { 0; }
    "#,
    );
    assert_eq!(result(&module).to_string(), "42");
}

#[test]
fn verifier_rejects_invalid_successor_updates_before_execution() {
    let mut module = lower(COUNTER, "Counter(n: 1).inc().value();");
    let update = module
        .functions
        .iter_mut()
        .flat_map(|f| &mut f.blocks)
        .flat_map(|b| &mut b.insts)
        .find_map(|i| match &mut i.kind {
            ir::InstKind::ObjNext { updates, .. } if !updates.is_empty() => Some(updates),
            _ => None,
        })
        .unwrap();
    update[0].0 = 999;
    assert!(matches!(execute(&module), Err(RuntimeError::InvalidIr(_))));
}
