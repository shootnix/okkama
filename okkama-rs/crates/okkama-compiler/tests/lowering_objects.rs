//! Lowering объектов: конструкторы, actions, successors, must-use (OIR-1 §8).

use okkama_compiler::SourceFile;
use okkama_compiler::ir;
use okkama_compiler::lowering::{LoweringError, lower_program};
use okkama_compiler::syntax::{self, ParseOptions};

const COUNTER: &str = "
object C {
    has n is int = 0;
    has m is int = 0;
    C() {}
    C(n is int) { n = n; confirm self.n >= 0; }
    inc() is ^C { ^n = n + 1; ^m = n; ^n = n + 10; }
    value() is int { n * 100 + m; }
    bump(by is int) is ^C { if by > 5 { ^n = n + by; } }
    add(value is int) is int { n + value; }
    add(value is string) is int { n; }
    twice() is int { self.helper() * 2; }
    private:
    helper() is int { n; }
}
";

fn lower(text: &str) -> Result<ir::Module, LoweringError> {
    let source = SourceFile::new("test.ok", text).expect("small source");
    let program = syntax::parse(&source, &ParseOptions::default()).expect("valid syntax");
    lower_program(&program)
}

fn module(main: &str) -> ir::Module {
    let text = format!("module app v1.0.0;{COUNTER} main() is int {{ {main} }}");
    let module = lower(&text).unwrap_or_else(|error| panic!("{main}: {error}"));
    ir::verify(&module).expect("lowering output verifies");
    module
}

/// Листинг одной функции из модуля.
fn function(module: &ir::Module, symbol: &str) -> String {
    let text = module.to_string();
    let start = text
        .find(&format!("func @{symbol}("))
        .unwrap_or_else(|| panic!("@{symbol} in\n{text}"));
    let end = text[start..].find("\n}\n").expect("function end") + start + 3;
    text[start..end].to_string()
}

fn error(main: &str) -> LoweringError {
    lower(&format!(
        "module app v1.0.0;{COUNTER} main() is int {{ {main} }}"
    ))
    .expect_err(main)
}

#[test]
fn objects_and_defaults_are_declared_in_source_order() {
    let module = module("0;");
    let text = module.to_string();
    assert!(
        text.contains(concat!(
            "object C {\n",
            "    field n: int = @C.default.n\n",
            "    field m: int = @C.default.m\n",
            "}\n",
        )),
        "{text}"
    );
    // Defaults вычисляются всегда и до тела конструктора (R09).
    assert_eq!(
        function(&module, "C.new#1"),
        concat!(
            "func @C.new#1(%0: int) -> C [ctor C] {\n",
            "^b0:\n",
            "    %1: int = call @C.default.n()\n",
            "    %2: int = call @C.default.m()\n",
            "    %3: int = const 0\n",
            "    %4: bool = int.ge %0, %3\n",
            "    cond %4, ^b1, ^b2\n",
            "^b1:\n",
            "    %5: C = obj.new C { n: %0, m: %2 }\n",
            "    ret %5\n",
            "^b2:\n",
            "    trap confirm \"confirm failed\"\n",
            "}\n",
        )
    );
}

#[test]
fn successors_read_original_fields_and_stage_the_last_value() {
    let module = module("0;");
    assert_eq!(
        function(&module, "C.inc"),
        concat!(
            "func @C.inc(%0: C) -> ^C [successor C] {\n",
            "^b0:\n",
            "    %1: int = obj.get %0, n\n",
            "    %2: int = const 1\n",
            "    %3: int = int.add %1, %2\n",
            "    %4: int = obj.get %0, n\n",
            "    %5: int = obj.get %0, n\n",
            "    %6: int = const 10\n",
            "    %7: int = int.add %5, %6\n",
            "    %8: C = obj.next %0 { n: %7, m: %4 }\n",
            "    ret %8\n",
            "}\n",
        )
    );
    // Ветвь без `^n` передаёт исходное поле в слияние.
    let bump = function(&module, "C.bump");
    assert!(
        bump.contains("^b3(%7: int):\n    %8: C = obj.next %0 { n: %7 }\n"),
        "{bump}"
    );
    assert!(
        bump.contains("^b2:\n    %6: int = obj.get %0, n\n    br ^b3(%6)\n"),
        "{bump}"
    );
}

#[test]
fn rebinding_in_loops_and_branches_threads_values_through_parameters() {
    let module = module(
        "let c = C(); for 1..=3 -> i { c = c.inc(); } \
         if c.value() > 0 { c = c.bump(by: 7); } else { c = c.bump(1); } c.value();",
    );
    let main = function(&module, "main");
    assert!(main.contains("^b2(%4: int, %5: C):\n"), "{main}");
    assert!(
        main.contains("%7: ^C = call @C.inc(%5)\n    %8: C = succ.take %7\n"),
        "{main}"
    );
    assert!(main.contains("br ^b2(%11, %9)"), "{main}");
    // Обе ветви `if` перепривязывают `c`: значение приходит параметром.
    assert!(main.matches("succ.take").count() >= 3, "{main}");
    assert!(main.contains("(%"), "{main}");
}

#[test]
fn overloads_and_named_arguments_resolve_statically() {
    let module = module("let c = C(n: 4); c.add(1) + c.add(\"x\") + c.add(value: 2);");
    let main = function(&module, "main");
    assert!(main.contains("call @C.add#0(%"), "{main}");
    assert!(main.contains("call @C.add#1(%"), "{main}");
    assert!(main.contains("call @C.new#1(%"), "{main}");
}

#[test]
fn shorthand_constructors_are_specialized_by_given_fields() {
    let text = "module app v1.0.0;
object P { has a is int; has b is int = 2; P(:a, :b); sum() is int { a + b; } }
main() is int { P(a: 1).sum() + P(b: 3, a: 4).sum(); }";
    let module = lower(text).unwrap();
    ir::verify(&module).unwrap();
    assert_eq!(
        function(&module, "P.new#0"),
        concat!(
            "func @P.new#0(%0: int, %1: int) -> P [ctor P] {\n",
            "^b0:\n",
            "    %2: int = call @P.default.b()\n",
            "    %3: P = obj.new P { a: %0, b: %1 }\n",
            "    ret %3\n",
            "}\n",
        )
    );
    assert_eq!(
        function(&module, "P.new#0.a"),
        concat!(
            "func @P.new#0.a(%0: int) -> P [ctor P] {\n",
            "^b0:\n",
            "    %1: int = call @P.default.b()\n",
            "    %2: P = obj.new P { a: %0, b: %1 }\n",
            "    ret %2\n",
            "}\n",
        )
    );
}

#[test]
fn successor_results_follow_the_perl_must_use_rules() {
    for (body, expected) in [
        (
            "let c = C(); c.inc(); 0;",
            "Successor action result must be used",
        ),
        (
            "let c = C(); let d = c.inc(); let e = d; 0;",
            "Successor action result must be used; binding \"e\" can leave scope without consumption",
        ),
        (
            "let c = C(); { let d = c.inc(); } 0;",
            "Successor action result must be used; binding \"d\" can leave scope without consumption",
        ),
        (
            "let c = C(); let d = c.inc(); let f = 1 > 0; if f { d.value(); } 0;",
            "binding \"d\" can leave scope without consumption",
        ),
        // Тело цикла может не выполниться: обязательство переживает цикл.
        (
            "let c = C(); let d = c.inc(); for 1..=2 -> i { d = d.inc(); } 0;",
            "binding \"d\" can leave scope without consumption",
        ),
        (
            "let c = C(); let d = c.inc(); return 0;",
            "binding \"d\" can leave scope without consumption",
        ),
        (
            "let c = C(); for 1..=2 -> i { let d = c.inc(); next; } 0;",
            "binding \"d\" can leave scope without consumption",
        ),
    ] {
        let error = error(body);
        assert!(matches!(error, LoweringError::Invalid { .. }), "{body}");
        assert!(
            error.message().contains(expected),
            "{body}: {}",
            error.message()
        );
    }
    // Потребление: receiver, аргумент, привязка поля, возврат, хвост, rebind.
    for body in [
        "let c = C(); let d = c.inc(); d.value();",
        "let c = C(); let d = c.inc(); let e = d; e.value();",
        "let c = C(); c.inc().inc().value();",
        "let c = C(); let d = c.inc(); if true { d.value(); } else { d.value(); }",
        "let c = C(); c = c.inc(); c.value();",
        // Литеральное условие известно, как в анализаторе Perl.
        "let c = C(); let d = c.inc(); if true { d.value(); } 0;",
        // Rebind гасит и прежнее, и новое значение.
        "let c = C(); let d = c.inc(); d = d.inc(); 0;",
        "let c = C(); let d = c.inc(); for 1..=2 -> i { d = d.inc(); } d.value();",
        "let c = C(); let d = c.inc(); loop { d = d.inc(); last; } d.value();",
        "let c = C(); let d = c.inc(); if 1 > 0 { d = d.inc(); } d.value();",
        "let c = C(); for 1..=2 -> i { let d = c.inc(); d = d.inc(); } 0;",
    ] {
        module(body);
    }
}

#[test]
fn construction_and_access_errors_use_okkama_messages() {
    for (body, expected) in [
        (
            "C(1, 2).value();",
            "No constructor of \"C\" accepts (int, int)",
        ),
        ("C().missing();", "Object \"C\" has no action \"missing\""),
        (
            "C().add(true);",
            "No action \"add\" on object \"C\" accepts (bool)",
        ),
        (
            "C().helper();",
            "Action \"helper\" on object \"C\" is private",
        ),
        (
            "let c = C(); c.n;",
            "Field \"n\" of object \"C\" is private; call an action instead",
        ),
        (
            "let x = 1; x = 2; x;",
            "Cannot rebind variable \"x\": type \"int\" has no successor actions",
        ),
        ("Missing();", "Undefined action \"Missing\""),
    ] {
        let error = error(body);
        assert!(
            matches!(error, LoweringError::Invalid { .. }),
            "{body}: {error:?}"
        );
        assert_eq!(error.message(), expected, "{body}");
    }
}

#[test]
fn constructor_bodies_follow_the_perl_scope_rules() {
    for (object, expected) in [
        (
            "object Q { has v is int; Q(v is int) { v = v; self.twice(); } twice() is int { v; } }",
            "Cannot call actions on self before construction completes",
        ),
        (
            "object Q { has v is int; has w is int; Q(v is int) { v = v; } }",
            "Object \"Q\" field \"w\" was not initialized",
        ),
        (
            "object Q { has v is int; Q(x is int) { confirm self.v > 0; v = x; } }",
            "Object \"Q\" field \"v\" is read before it is bound",
        ),
        (
            "object Q { has v is int; has w is int = 1; Q(x is int) { v = w; } }",
            "Undefined variable \"w\"",
        ),
        (
            "object Q { has v is int; Q(x is int) { v = x; return; } }",
            "`return` is not allowed in a constructor",
        ),
        (
            "object Q { has v is int = 0; Q(); next() is ^Q { return self; } }",
            "A successor action cannot return a replacement value",
        ),
        (
            "object Q { has v is int = 0; Q(); touch() is int { ^v = 1; 0; } }",
            "`^field = value` is allowed only in successor actions",
        ),
    ] {
        let text = format!("module app v1.0.0; {object} main() is int {{ 0; }}");
        let error = lower(&text).expect_err(object);
        assert!(
            error.message().contains(expected),
            "{object}: {}",
            error.message()
        );
    }
}

#[test]
fn a_field_bound_on_some_paths_is_checked_after_the_body() {
    // Как в Perl: полноту проверяет конец конструктора, а не слияние ветвей.
    let module = lower(
        "module app v1.0.0; object Q { has v is int; Q(x is int) { if x > 0 { v = 1; } } } \
         main() is int { 0; }",
    )
    .expect("lowering succeeds");
    ir::verify(&module).expect("valid IR");
    let text = module.to_string();
    assert!(text.contains("= undef\n"), "{text}");
    assert!(
        text.contains("trap unbound \"Object \\\"Q\\\" field \\\"v\\\" was not initialized\""),
        "{text}"
    );
}
