//! Lowering Okkama → OIR-1: канонические листинги и диагностика.

use okkama_compiler::SourceFile;
use okkama_compiler::ir;
use okkama_compiler::lowering::{LoweringError, lower_program};
use okkama_compiler::syntax::{self, ParseOptions, prune};

fn lower(text: &str) -> Result<ir::Module, LoweringError> {
    let source = SourceFile::new("test.ok", text).expect("small source");
    let program = syntax::parse(&source, &ParseOptions::default()).expect("valid syntax");
    lower_program(&prune::strip_test_items(program))
}

fn listing(body: &str) -> String {
    let module = lower(&format!("module app v1.0.0; main() is int {{ {body} }}"))
        .unwrap_or_else(|error| panic!("{body}: {error}"));
    ir::verify(&module).expect("lowering output verifies");
    module.to_string()
}

fn main_listing(body: &str) -> String {
    let text = listing(body);
    let start = text.find("func @main").expect("main is printed");
    // Только `main`: lowering по достижимости печатает её первой.
    let end = text[start..]
        .find("\n}\n")
        .map_or(text.len(), |end| start + end + 3);
    text[start..end].to_string()
}

#[test]
fn integer_addition_has_the_canonical_module_text() {
    assert_eq!(
        listing("1 + 1;"),
        concat!(
            "oir 1 high\n",
            "module app v1.0.0\n",
            "requires core\n",
            "entry @main\n",
            "\n",
            "func @main() -> int [entry] {\n",
            "^b0:\n",
            "    %0: int = const 1\n",
            "    %1: int = const 1\n",
            "    %2: int = int.add %0, %1\n",
            "    ret %2\n",
            "}\n",
        )
    );
}

#[test]
fn literals_are_exact_canonical_integers() {
    let text = main_listing(
        "let a = 0x2a; let b = 0b10_1010; let c = 00042; let d = -0; let e = -7; \
         let f = 340282366920938463463374607431768211456; a;",
    );
    for constant in [
        "const 42\n",
        "const 0\n",
        "const -7\n",
        "const 340282366920938463463374607431768211456\n",
    ] {
        assert!(text.contains(constant), "{constant} in\n{text}");
    }
    assert_eq!(text.matches("const 42\n").count(), 3, "{text}");
    // Унарный минус перед литералом сворачивается, перед выражением — нет.
    let negated = main_listing("let x = 5; -x;");
    assert!(negated.contains("int.neg %0"), "{negated}");
}

#[test]
fn let_bindings_reuse_values_and_check_declared_types() {
    assert_eq!(
        main_listing("let left = 40; let right is int = 2; let copy = left; copy + right;"),
        concat!(
            "func @main() -> int [entry] {\n",
            "^b0:\n",
            "    %0: int = const 40\n",
            "    %1: int = const 2\n",
            "    %2: int = int.add %0, %1\n",
            "    ret %2\n",
            "}\n",
        )
    );
    // Неявный результат — значение последнего оператора, включая `let`.
    assert!(main_listing("let x = 7;").contains("ret %0"));
}

#[test]
fn if_expression_joins_through_a_block_parameter() {
    assert_eq!(
        main_listing("let flag = true; let value = if flag { 1; } else { 2; }; value;"),
        concat!(
            "func @main() -> int [entry] {\n",
            "^b0:\n",
            "    %0: bool = const true\n",
            "    cond %0, ^b1, ^b2\n",
            "^b1:\n",
            "    %1: int = const 1\n",
            "    br ^b3(%1)\n",
            "^b2:\n",
            "    %2: int = const 2\n",
            "    br ^b3(%2)\n",
            "^b3(%3: int):\n",
            "    ret %3\n",
            "}\n",
        )
    );
}

#[test]
fn logical_operators_short_circuit() {
    assert_eq!(
        main_listing("let a = true; let b = false; let c = a && b; 0;"),
        concat!(
            "func @main() -> int [entry] {\n",
            "^b0:\n",
            "    %0: bool = const true\n",
            "    %1: bool = const false\n",
            "    cond %0, ^b1, ^b2\n",
            "^b1:\n",
            "    br ^b3(%1)\n",
            "^b2:\n",
            "    %2: bool = const false\n",
            "    br ^b3(%2)\n",
            "^b3(%3: bool):\n",
            "    %4: int = const 0\n",
            "    ret %4\n",
            "}\n",
        )
    );
    let or = main_listing("let a = true; let c = a || false; 0;");
    // `||`: истинная ветвь — короткое замыкание с `true`.
    assert!(or.contains("cond %0, ^b1, ^b2"), "{or}");
    assert!(or.contains("^b1:\n    %1: bool = const true\n"), "{or}");
}

#[test]
fn statement_if_chains_and_early_returns_leave_no_dead_blocks() {
    let text =
        main_listing("let x = 5; if x < 0 { return 1; } else if x == 0 { return 2; } return 3;");
    assert_eq!(text.matches("ret ").count(), 3, "{text}");
    assert!(!text.contains("unreachable"), "{text}");
    // Код после `return` проверяется, но не попадает в IR.
    let dead = main_listing("return 1; 2 + 3;");
    assert_eq!(
        dead,
        "func @main() -> int [entry] {\n^b0:\n    %0: int = const 1\n    ret %0\n}\n"
    );
    let error = lower("module app v1.0.0; main() is int { return 1; missing; }").unwrap_err();
    assert_eq!(error.message(), "Undefined variable \"missing\"");
}

#[test]
fn loops_carry_last_values_and_ranges_check_their_bounds() {
    // Единственный выход цикла не требует блока слияния.
    assert_eq!(
        main_listing("let n = loop { last 7; }; n;"),
        "func @main() -> int [entry] {\n^b0:\n    br ^b1\n^b1:\n    %0: int = const 7\n    ret %0\n}\n"
    );

    let range = main_listing("for 1..=3 -> i { next if i == 2; } 0;");
    assert!(
        range.contains("trap range \"Range start must be less than or equal to range end\""),
        "{range}"
    );
    assert!(range.contains("int.le %"), "{range}");
    // Счётчик цикла — точное `int.add` (R01: int — bigint, переполнения нет),
    // без промежуточного диапазона на каждой итерации.
    assert!(range.contains("int.add"), "{range}");
    assert!(!range.contains("range.new"), "{range}");
    let exclusive = main_listing("for 0..3 -> i { last if i == 1; } 0;");
    assert!(exclusive.contains("int.lt %"), "{exclusive}");

    // Бесконечный цикл без `last`: продолжение недостижимо, хвост не нужен.
    let forever = main_listing("loop { 1; }");
    assert!(!forever.contains("ret"), "{forever}");
}

#[test]
fn confirm_and_panic_become_terminators() {
    let text = main_listing("confirm 1 == 1; confirm true else panic \"boom\"; 0;");
    assert!(text.contains("trap confirm \"confirm failed\""), "{text}");
    assert!(text.contains("%5: str = const \"boom\""), "{text}");
    assert!(text.contains("panic %5"), "{text}");
}

#[test]
fn strings_concatenate_and_compare_and_print_escaped() {
    let text = main_listing("let s = \"a\\\"b\" + \"\\n\"; confirm s != \"x\"; 0;");
    assert!(text.contains("const \"a\\\"b\""), "{text}");
    assert!(text.contains("const \"\\n\""), "{text}");
    assert!(text.contains("str.concat %0, %1"), "{text}");
    assert!(text.contains("str.ne %2, %3"), "{text}");
}

#[test]
fn every_operator_maps_to_one_instruction_family() {
    let ops = main_listing(
        "let a = 6; let b = 3; let s = a - b; let m = a * b; let r = a % b; let x = a & b; \
         let o = a | b; let y = a ^ b; let l = a << b; let h = a >> b; let u = a >>> b; \
         let n = ~a; let lt = a < b; let le = a <= b; let gt = a > b; let ge = a >= b; \
         let ne = a != b; let t = !(true == false); 0;",
    );
    for mnemonic in [
        "int.sub", "int.mul", "int.mod", "int.and", "int.or", "int.xor", "int.shl", "int.shr",
        "int.ushr", "int.not", "int.lt", "int.le", "int.gt", "int.ge", "int.ne", "bool.eq",
        "bool.not",
    ] {
        assert!(
            ops.contains(&format!("= {mnemonic} ")),
            "{mnemonic} in\n{ops}"
        );
    }
}

#[test]
fn semantic_errors_are_reported_with_okkama_messages() {
    for (body, expected) in [
        ("missing;", "Undefined variable \"missing\""),
        (
            "let x = 1; let x = 2; x;",
            "Variable \"x\" is already defined in this scope",
        ),
        (
            "let x is bool = 1; 0;",
            "Cannot initialize variable \"x\" of type bool with int",
        ),
        (
            "let x is @int = 1; 0;",
            "Cannot initialize variable \"x\" of type @int with int",
        ),
        (
            "let x = 1; x = 2; x;",
            "Cannot rebind variable \"x\": type \"int\" has no successor actions",
        ),
        ("next; 0;", "`next` used outside of a loop"),
        ("last; 0;", "`last` used outside of a loop"),
        (
            "let v = loop { last; }; 0;",
            "Bare `last` cannot produce the value of this loop",
        ),
        (
            "let c = 1 > 0; let v = if c { 1; } else { \"a\"; }; 0;",
            "Branches produce different types: int and string",
        ),
        ("if 1 { 0; } 0;", "If condition must be bool, got int"),
        ("1 == true;", "Cannot compare int and bool"),
        (
            "1 + \"a\";",
            "Operator + cannot be applied to int and string",
        ),
        ("!1;", "Operator ! cannot be applied to int"),
        ("confirm 1;", "Confirm condition must be bool, got int"),
        ("panic 1;", "Panic message must be string, got int"),
        ("let b = true; b;", "Action result must be int, got bool"),
        (
            "{}",
            "Action \"main\" must produce a return value of type int.",
        ),
        ("return;", "`return;` requires a value of type int"),
    ] {
        let error = lower(&format!("module app v1.0.0; main() is int {{ {body} }}")).unwrap_err();
        assert!(
            matches!(error, LoweringError::Invalid { .. }),
            "{body}: {error:?}"
        );
        assert_eq!(error.message(), expected, "{body}");
    }
}

#[test]
fn unsupported_constructs_are_errors_not_silent_drops() {
    let text = "main() is int { let xs is @any = [1..3, \"a\"]; 0; }";
    let error = lower(&format!("module app v1.0.0; {text}")).unwrap_err();
    assert!(
        matches!(error, LoweringError::Unsupported { .. }),
        "{text}: {error:?}"
    );
    assert!(
        error.message().contains("ranges inside `@any` literals"),
        "{text}: {}",
        error.message()
    );
    for (text, expected) in [
        (
            "main() is string { \"a\"; }",
            "must be declared `main() is int`",
        ),
        (
            "main() is int { 1; } main() is int { 2; }",
            "Duplicate action name \"main\"",
        ),
    ] {
        let error = lower(&format!("module app v1.0.0; {text}")).unwrap_err();
        assert!(
            error.message().contains(expected),
            "{text}: {}",
            error.message()
        );
    }
}

#[test]
fn test_declarations_are_pruned_before_lowering() {
    let module = lower("module app v1.0.0; main() is int { 0; } test \"t\" { test.ok(true); }")
        .expect("tests are removed by the explicit pruning pass");
    assert_eq!(module.functions.len(), 1);
}

/// Bound action роли, `spawn` синхронного действия (вход воркера, §18.1) и
/// корневой `native object` понижаются в проверенный IR.
#[test]
fn role_bound_actions_worker_spawn_and_root_native_objects_lower() {
    for (text, needle) in [
        (
            "role R { f() is int; } object O does R { O(); f() is int { 1; } } main() is int { let r is R = O(); let g is () is int = r.f; g(); }",
            "fn.closure @member.f",
        ),
        (
            "object W { has n is int; W(:n); f(x is int) is int { n + x; } } async main() is int { await spawn W(n: 1).f(x: 2); }",
            "worker.spawn.direct",
        ),
        (
            "object W { has n is int; W(:n); f(x is int) is int { n + x; } } async main() is int { let g is (int) is int = W(n: 1).f; await spawn g(2); }",
            "worker.bind",
        ),
        ("main() is int { 1; } native object Handle;", "@main"),
    ] {
        let module = lower(&format!("module app v1.0.0; {text}"))
            .unwrap_or_else(|error| panic!("{text}: {error:?}"));
        let printed = module.to_string();
        assert!(printed.contains(needle), "{text}:\n{printed}");
    }
}
