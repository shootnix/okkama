//! Limited role references retain the object and dispatch through checked witnesses.
use okkama_compiler::{SourceFile, codegen, ir, lowering, syntax};
use okkama_runtime::{execute, sparrow};

fn lower(declarations: &str, body: &str) -> ir::Module {
    let source = SourceFile::new(
        "roles.ok",
        format!("module app v1.0.0; {declarations} main() is int {{ {body} }}"),
    )
    .unwrap();
    lowering::lower_program(&syntax::parse(&source, &syntax::ParseOptions::default()).unwrap())
        .unwrap()
}

fn check(module: &ir::Module, expected: &str) {
    assert_eq!(execute(module).unwrap().to_string(), expected);
    let program = codegen::sparrow::generate(module).unwrap();
    let bytes = okkama_sparrow::encode(&program).unwrap();
    assert_eq!(&bytes[4..6], &okkama_sparrow::VERSION.to_be_bytes());
    let loaded = okkama_sparrow::decode(&bytes).unwrap();
    assert_eq!(loaded, program);
    assert_eq!(sparrow::execute(&loaded).unwrap().to_string(), expected);
}

#[test]
fn narrowing_checks_nominal_witness_and_keeps_the_failure_lazy() {
    let declarations = "role Wide { read() is int; extra() is int; } role Narrow { read() is int; } object Both does Wide, Narrow { Both(); read() is int { 42; } extra() is int { 1; } } object OnlyWide does Wide { OnlyWide(); read() is int { 7; } extra() is int { 2; } }";
    check(
        &lower(
            declarations,
            "let wide is Wide = Both(); let narrow is Narrow = wide; narrow.read();",
        ),
        "42",
    );
    check(
        &lower(
            declarations,
            "let wide is Wide = OnlyWide(); if false { let narrow is Narrow = wide; narrow.read(); } else { 42; }",
        ),
        "42",
    );
    let module = lower(
        declarations,
        "let wide is Wide = OnlyWide(); let narrow is Narrow = wide; narrow.read();",
    );
    let Err(okkama_runtime::RuntimeError::Trap { message, .. }) = execute(&module) else {
        panic!("expected missing role witness trap");
    };
    assert!(message.contains("Narrow"));
    let bytes = okkama_sparrow::encode(&codegen::sparrow::generate(&module).unwrap()).unwrap();
    assert!(
        matches!(sparrow::execute(&okkama_sparrow::decode(&bytes).unwrap()), Err(sparrow::Error::Operation { error: okkama_runtime::RuntimeError::Trap { message: actual, .. }, .. }) if actual == message)
    );
}

#[test]
fn role_dispatch_uses_actual_object_and_specialized_fields() {
    let declarations = "role Reading { read() is int; } object A does Reading { A(); read() is int { 20; } } object B does Reading { B(); read() is int { 22; } } object Box<T> { has value is T; Box(:value); get() is T { value; } }";
    check(
        &lower(
            declarations,
            "let a is Reading = A(); let b is Reading = B(); a.read() + b.read();",
        ),
        "42",
    );
    check(
        &lower(
            declarations,
            "let xs is @Reading = [A(), B()]; xs[0].read() + xs[1].read();",
        ),
        "42",
    );
    check(
        &lower(
            declarations,
            "let a is Reading = A(); let boxed = Box(value: a); boxed.get().read();",
        ),
        "20",
    );
}

#[test]
fn role_calls_support_parameters_recursion_and_void_results() {
    check(
        &lower(
            "role Compute { calc(n is int) is int; touch() is void; } object Counter does Compute { Counter(); touch() is void {} calc(n is int) is int { if n == 0 { 0; } else { let r is Compute = self; r.calc(n - 1) + 1; } } }",
            "let c is Compute = Counter(); c.touch(); c.calc(42);",
        ),
        "42",
    );
    check(
        &lower(
            "role Echo<T> { echo(value is T) is T; } object Echoer<T> does Echo<T> { has seed is T; Echoer(:seed); echo(value is T) is T { value; } }",
            "let e is Echo<int> = Echoer(seed: 0); e.echo(42);",
        ),
        "42",
    );
}

#[test]
fn successor_does_not_retarget_existing_role_references() {
    check(
        &lower(
            "role Reading { read() is int; } object Counter does Reading { has value is int; Counter(:value); read() is int { value; } plus(n is int) is ^Counter { ^value = value + n; } }",
            "let c = Counter(value: 20); let old is Reading = c; let updated = c.plus(2); let next_value = updated.read(); old.read() + next_value;",
        ),
        "42",
    );
}
