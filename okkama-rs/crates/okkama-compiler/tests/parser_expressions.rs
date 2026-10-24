//! Expressions: precedence, associativity, postfix chains and every
//! expression form. Many cases are ported from okkama-pl/t/parser
//! (06, 12, 17, 18, 19, 20, 21-bitwise, 41).

mod common;

use common::{assert_rejects, body_sexpr, expr_sexpr};

#[test]
fn arithmetic_precedence_and_left_associativity() {
    assert_eq!(expr_sexpr("1 + 2 * 3"), "(+ 1 (* 2 3))");
    assert_eq!(expr_sexpr("1 * 2 + 3"), "(+ (* 1 2) 3)");
    assert_eq!(expr_sexpr("10 - 4 - 3"), "(- (- 10 4) 3)");
    assert_eq!(expr_sexpr("8 / 4 / 2"), "(/ (/ 8 4) 2)");
    assert_eq!(expr_sexpr("7 % 3 * 2"), "(* (% 7 3) 2)");
    assert_eq!(expr_sexpr("(1 + 2) * 3"), "(* (group (+ 1 2)) 3)");
}

#[test]
fn unary_operators_bind_tighter_than_binary() {
    assert_eq!(expr_sexpr("-1 + 2"), "(+ (- 1) 2)");
    assert_eq!(expr_sexpr("!a && b"), "(&& (! a) b)");
    assert_eq!(expr_sexpr("- - x"), "(- (- x))");
    assert_eq!(expr_sexpr("~1 & 3"), "(& (~ 1) 3)");
    assert_eq!(expr_sexpr("-x.y()"), "(- (call (. x y)))");
}

/// The approved bitwise grouping table from t/parser/21-bitwise.t.
#[test]
fn bitwise_and_shift_precedence() {
    for (source, expected) in [
        ("1 << 2 + 1", "(<< 1 (+ 2 1))"),
        ("4 | 1 == 5", "(== (| 4 1) 5)"),
        ("7 & 3 ^ 1 | 8", "(| (^ (& 7 3) 1) 8)"),
        ("8 >> 1 >> 1", "(>> (>> 8 1) 1)"),
        ("7 ^ 3 ^ 1", "(^ (^ 7 3) 1)"),
        ("1 & 2 << 3", "(& 1 (<< 2 3))"),
        (
            "true || false && 1 ^ 2 == 3",
            "(|| true (&& false (== (^ 1 2) 3)))",
        ),
        ("(12) & (10)", "(& (group 12) (group 10))"),
        ("1 >>> 2", "(>>> 1 2)"),
        ("a < b | c", "(< a (| b c))"),
    ] {
        assert_eq!(expr_sexpr(source), expected, "{source}");
    }
}

#[test]
fn comparison_equality_and_logic_levels() {
    assert_eq!(expr_sexpr("a < b == c > d"), "(== (< a b) (> c d))");
    assert_eq!(expr_sexpr("a == b != c"), "(!= (== a b) c)");
    assert_eq!(expr_sexpr("a || b || c"), "(|| (|| a b) c)");
    assert_eq!(expr_sexpr("a && b || c && d"), "(|| (&& a b) (&& c d))");
    // Comparisons chain left-associatively in the grammar.
    assert_eq!(expr_sexpr("a < b < c"), "(< (< a b) c)");
}

#[test]
fn ranges_sit_between_equality_and_comparison() {
    assert_eq!(expr_sexpr("1..10"), "(.. 1 10)");
    assert_eq!(expr_sexpr("1..=10"), "(..= 1 10)");
    assert_eq!(
        expr_sexpr("[0, 1 + 1..=limit - 1, 99]"),
        "(array 0 (..= (+ 1 1) (- limit 1)) 99)"
    );
    assert_eq!(expr_sexpr("a..b == c"), "(== (.. a b) c)");
    assert_eq!(expr_sexpr("a < b..c"), "(.. (< a b) c)");
    assert_rejects(
        "module app v1.0.0; main() is int { 1..2..3; }",
        "Range expressions cannot be chained.",
    );
}

#[test]
fn stream_write_chains_left_and_binds_below_equality() {
    assert_eq!(
        expr_sexpr("sink <- first <- second <- third"),
        "(<- (<- (<- sink first) second) third)"
    );
    assert_eq!(
        expr_sexpr("out <- \"value: \" + value.string() <- object.method().string()"),
        "(<- (<- out (+ \"value: \" (call (. value string)))) (call (. (call (. object method)) string)))"
    );
    assert_eq!(
        expr_sexpr("out <- (left + right) <- if condition { first; } else { second; }"),
        "(<- (<- out (group (+ left right))) (if condition {first} {second}))"
    );
    assert_eq!(expr_sexpr("sink <- -1"), "(<- sink (- 1))");
    assert_eq!(expr_sexpr("a <- b == c"), "(<- a (== b c))");
    assert_eq!(expr_sexpr("a <- b && c"), "(&& (<- a b) c)");
    assert_eq!(
        expr_sexpr("sink <- (value << 2).string()"),
        "(<- sink (call (. (group (<< value 2)) string)))"
    );
    // `a < -b` is a comparison with a negative operand.
    assert_eq!(expr_sexpr("a < -b"), "(< a (- b))");
}

#[test]
fn stream_write_with_endl_and_unpacked_array_cast() {
    assert_eq!(
        expr_sexpr("out <- @string(1, 2, 3)... <- endl"),
        "(<- (<- out (... (@string 1 2 3))) endl)"
    );
}

#[test]
fn malformed_operator_extensions_are_rejected() {
    for source in [
        "&thing",
        "1 xor 2",
        "sink <<< value",
        "1 <<=",
        "sink <-= value",
        "sink ~ value",
        "a &= 1",
        "a |= 1",
        "a ^= 1",
        "a >>= 1",
        "a >>>= 1",
        "a ^",
        "^a",
        "out <-",
        "<- \"hello\"",
        "2 ** 3",
    ] {
        let text = format!("module app v1.0.0; main() is int {{ {source}; }}");
        let message = common::parse_err(&text);
        assert!(!message.is_empty(), "{source}");
    }
}

#[test]
fn calls_arguments_and_trailing_commas() {
    assert_eq!(expr_sexpr("foo(1, 2,)"), "(call foo 1 2)");
    assert_eq!(expr_sexpr("foo()"), "(call foo)");
    assert_eq!(
        expr_sexpr("Cash(digit: 10, cents: 25)"),
        "(call Cash digit:10 cents:25)"
    );
    assert_eq!(expr_sexpr("f(a)(b)"), "(call (call f a) b)");
    assert_eq!(expr_sexpr("byte(112)"), "(call byte 112)");
    assert_rejects(
        "module app v1.0.0; main() is int { foo(1,,); }",
        "Expect expression.",
    );
}

#[test]
fn postfix_chains_left_to_right() {
    assert_eq!(
        expr_sexpr("a.b(1)[2].c::d..."),
        "(... (:: (. (index (call (. a b) 1) 2) c) d))"
    );
    assert_eq!(
        expr_sexpr("users[0] else fallback"),
        "(index users 0 else fallback)"
    );
    assert_eq!(
        expr_sexpr("m[k] else d + 1"),
        "(index m k else (+ d 1))",
        "the fallback is a full expression"
    );
    assert_eq!(expr_sexpr("test.ok(true)"), "(call (. test ok) true)");
    assert_eq!(expr_sexpr("value.last()"), "(call (. value last))");
    assert_eq!(expr_sexpr("Role.fake()"), "(call (. Role fake))");
    assert_eq!(expr_sexpr("x.endl"), "(. x endl)");
}

#[test]
fn unpack_applies_to_any_postfix_operand() {
    assert_eq!(
        body_sexpr("let copy = [1, 2]...;"),
        "(LetStatement name=copy type=_ value=(... (array 1 2)))"
    );
    assert_eq!(expr_sexpr("source()..."), "(... (call source))");
    assert_eq!(expr_sexpr("1..."), "(... 1)");
    assert_eq!(expr_sexpr("values... + 1"), "(+ (... values) 1)");
}

#[test]
fn collection_literals() {
    assert_eq!(expr_sexpr("[]"), "(array)");
    assert_eq!(expr_sexpr("[:]"), "(map)");
    assert_eq!(expr_sexpr("[ : ]"), "(map)");
    assert_eq!(expr_sexpr("[1, 2,]"), "(array 1 2)");
    assert_eq!(expr_sexpr("[a: 1, b: 2,]"), "(map a:1 b:2)");
    assert_eq!(expr_sexpr("[[1], [:]]"), "(array (array 1) (map))");
    assert_eq!(
        expr_sexpr("[byte(0), byte(255)]"),
        "(array (call byte 0) (call byte 255))"
    );
    assert_rejects(
        "module app v1.0.0; main() is int { [ \"a\": ]; }",
        "Expect expression.",
    );
    assert_rejects(
        "module app v1.0.0; main() is int { [: 1]; }",
        "Expect \"]\" after empty map literal.",
    );
    assert_rejects(
        "module app v1.0.0; main() is int { [a: 1, b]; }",
        "Expect \":\" after map key.",
    );
    assert_rejects(
        "module app v1.0.0; main() is int { { \"a\": 1 }; }",
        "Expect \";\" after expression.",
    );
}

#[test]
fn literal_forms() {
    assert_eq!(expr_sexpr("0x2A"), "0x2A");
    assert_eq!(expr_sexpr("0b1010"), "0b1010");
    assert_eq!(expr_sexpr("3.14"), "3.14");
    assert_eq!(expr_sexpr("'x'"), "'x'");
    assert_eq!(expr_sexpr("\"a\\tb\""), "\"a\\tb\"");
    assert_eq!(
        expr_sexpr("```raw \"text\" \\n```"),
        "```raw \"text\" \\n```"
    );
    assert_eq!(expr_sexpr("endl"), "endl");
    assert_eq!(expr_sexpr("true"), "true");
    assert_eq!(expr_sexpr("self"), "self");
}

#[test]
fn array_casts_have_a_closed_target_set() {
    assert_eq!(expr_sexpr("@byte(1, 2,)"), "(@byte 1 2)");
    assert_eq!(expr_sexpr("@byte()"), "(@byte)");
    assert_eq!(expr_sexpr("@int(a..., b...)"), "(@int (... a) (... b))");
    for source in ["@bool(1)", "@rat(1, 2)", "@Widget()"] {
        assert_rejects(
            &format!("module app v1.0.0; main() is int {{ {source}; }}"),
            "Array casts support int, float, byte and string only.",
        );
    }
    assert_rejects(
        "module app v1.0.0; main() is int { @@byte(1); }",
        "Expect primitive cast type after \"@\".",
    );
}

#[test]
fn functional_suffixes() {
    assert_eq!(
        expr_sexpr("values.map() -> value { value * 2; }"),
        "(map values -> [value] {(* value 2)})"
    );
    assert_eq!(
        expr_sexpr("values.reduce(0) -> sum, value { sum + value; }"),
        "(reduce values 0 -> [sum value] {(+ sum value)})"
    );
    assert_eq!(
        expr_sexpr("(1..=5).map() -> number { number * number; }"),
        "(map (group (..= 1 5)) -> [number] {(* number number)})"
    );
    assert_eq!(
        expr_sexpr("mapping.filter() -> key, value { value.more(0); }"),
        "(filter mapping -> [key value] {(call (. value more) 0)})"
    );
    // Chains nest through the receiver.
    assert_eq!(
        expr_sexpr("v.filter() -> x { x; }.map() -> x { x; }.reduce(0) -> s, x { s; }"),
        "(reduce (map (filter v -> [x] {x}) -> [x] {x}) 0 -> [s x] {s})"
    );
    // Without an arrow, `map()` is an ordinary call.
    assert_eq!(expr_sexpr("mapper.map()"), "(call (. mapper map))");
}

#[test]
fn functional_suffix_errors() {
    let wrap = |body: &str| format!("module app v1.0.0; main() is int {{ {body} }}");
    for (body, expected) in [
        (
            "values.map(1) -> value { value; };",
            "Functional operation \"map\" accepts no arguments.",
        ),
        (
            "values.filter(limit: 1) -> value { true; };",
            "Functional operation \"filter\" does not accept named arguments.",
        ),
        (
            "values.reduce() -> sum, value { sum; };",
            "Functional operation \"reduce\" requires exactly one initial value.",
        ),
        (
            "values.reduce(0, 1) -> sum, value { sum; };",
            "Functional operation \"reduce\" requires exactly one initial value.",
        ),
        (
            "values.map() -> { 1; };",
            "Expect functional binding name after \"->\".",
        ),
        (
            "values.map() -> value, value { value; };",
            "Functional binding \"value\" is declared more than once.",
        ),
        ("values.map() -> value;", "Expect \"{\" before block."),
        (
            "values.fold(0) -> sum, value { sum; };",
            "Unsupported functional operation \"fold\".",
        ),
        (
            "runner.apply -> value { value; };",
            "Expect \";\" after expression.",
        ),
    ] {
        assert_rejects(&wrap(body), expected);
    }
}

#[test]
fn anonymous_actions() {
    assert_eq!(expr_sexpr("() is void {}"), "(fn [] void {})");
    assert_eq!(
        expr_sexpr("(left is int, right is int) is int { left + right; }"),
        "(fn [left int right int] int {(+ left right)})"
    );
    assert_eq!(
        expr_sexpr("consume(10, (value is int) is int { value * 2; })"),
        "(call consume 10 (fn [value int] int {(* value 2)}))"
    );
    assert_eq!(
        expr_sexpr("((value is int) is int { value; })(5)"),
        "(call (group (fn [value int] int {value})) 5)"
    );
    assert_eq!(
        expr_sexpr("(first is int) is (int) is int { first; }"),
        "(fn [first int] (int) is int {first})"
    );
}

#[test]
fn anonymous_action_errors() {
    let wrap = |body: &str| format!("module app v1.0.0; main() is int {{ {body} }}");
    for (body, expected) in [
        ("(, value is int) is void {};", "Expect expression."),
        (
            "(value is int, ) is void {};",
            "Expect anonymous action parameter after \",\".",
        ),
        (
            "(value is int, value is int) is int { value; };",
            "Anonymous action parameter \"value\" is declared more than once.",
        ),
        (
            "(value) is int { value; };",
            "Expect \";\" after expression.",
        ),
        (
            "(values is ...int) is int { values; };",
            "Variadic anonymous action parameters are not supported.",
        ),
        ("(value is int) is int;", "Expect \"{\" before block."),
        (
            "(value is int) is ^int { value; };",
            "Anonymous actions cannot declare successor results.",
        ),
        ("() {};", "Anonymous action requires an explicit result"),
    ] {
        assert_rejects(&wrap(body), expected);
    }
}

#[test]
fn async_await_and_spawn() {
    assert_eq!(expr_sexpr("await pending"), "(await pending)");
    assert_eq!(
        expr_sexpr("(await pending).string()"),
        "(call (. (group (await pending)) string))"
    );
    assert_eq!(
        expr_sexpr("await service.load() + 1"),
        "(+ (await (call (. service load))) 1)"
    );
    assert_eq!(expr_sexpr("await await nested"), "(await (await nested))");
    assert_eq!(
        expr_sexpr("spawn worker.build(input)"),
        "(spawn (call (. worker build) input))"
    );
    assert_eq!(expr_sexpr("spawn run(1) + 2"), "(+ (spawn (call run 1)) 2)");
    // `spawn(` is an ordinary call of an action labelled `spawn`.
    assert_eq!(expr_sexpr("spawn(1, 2)"), "(call spawn 1 2)");
    assert_rejects(
        "module app v1.0.0; main() is int { spawn value; }",
        "spawn requires an action call",
    );
    assert_rejects(
        "module app v1.0.0; main() is int { await; }",
        "Expect expression.",
    );
}

#[test]
fn panic_takes_a_whole_expression() {
    assert_eq!(expr_sexpr("panic \"stop\""), "(panic \"stop\")");
    assert_eq!(expr_sexpr("panic a || b"), "(panic (|| a b))");
    assert_eq!(expr_sexpr("x + panic b"), "(+ x (panic b))");
    assert_rejects(
        "module app v1.0.0; main() is int { panic; }",
        "\"panic;\" is not supported; use \"throw;\" to rethrow.",
    );
}

#[test]
fn named_constructor_projections() {
    assert_eq!(
        expr_sexpr("value ParsedInt(answer)"),
        "(projection value (call ParsedInt answer))"
    );
    assert_eq!(
        expr_sexpr("value strings.ParsedInt(answer).less(100)"),
        "(call (. (projection value (call (. strings ParsedInt) answer)) less) 100)"
    );
    assert_eq!(
        expr_sexpr("of Outer(item: of Inner(source))"),
        "(projection of (call Outer item:(projection of (call Inner source))))"
    );
    // `of(Max(1))` is an ordinary call; `of Max(1)` is a projection.
    assert_eq!(expr_sexpr("of(Max(1))"), "(call of (call Max 1))");
    // A lowercase juxtaposition is not a projection.
    assert_rejects(
        "module app v1.0.0; main() is int { of value; }",
        "Expect \";\" after expression.",
    );
    assert_rejects(
        "module app v1.0.0; main() is int { of factory.build(); }",
        "Expect \";\" after expression.",
    );
    // Binary AND, not the removed `&` projection.
    assert_eq!(
        expr_sexpr("ParsedInt&(answer)"),
        "(& ParsedInt (group answer))"
    );
}

#[test]
fn removed_projection_spellings_have_migration_errors() {
    assert_rejects(
        "module app v1.0.0; main() is int { &parser(answer); }",
        "Constructor projections use a declared prefix, not \"&\".",
    );
    assert_rejects(
        "module app v1.0.0; main() is int { ParsedInt:(answer); }",
        "Constructor projections use a declared prefix, not \":\".",
    );
    assert_rejects(
        "module app v1.0.0; main() is int { f(Parsed:(x)); }",
        "Constructor projections use a declared prefix, not \":\".",
    );
}

#[test]
fn if_match_loop_and_try_expressions() {
    assert_eq!(
        body_sexpr("let v = if c { 1; } else { 2; };"),
        "(LetStatement name=v type=_ value=(if c {1} {2}))"
    );
    assert_rejects(
        "module app v1.0.0; main() is int { let x = if c { 1; }; }",
        "If expression requires else branch.",
    );
    assert_rejects(
        "module app v1.0.0; main() is int { let x = if a { 1; } else if b { 2; } else { 3; }; }",
        "Expect \"{\" before block.",
    );
    assert_eq!(
        body_sexpr("let v = match x { case 1, 2 { a; } else { b; } };"),
        "(LetStatement name=v type=_ value=(MatchExpression subject=x cases=[(MatchCase patterns=[1 2] body={a})] else={b}))"
    );
    assert_eq!(
        body_sexpr("let value = loop { last 2; };"),
        "(LetStatement name=value type=_ value=(loop {(LastStatement value=2 condition=_)}))"
    );
    assert_eq!(
        body_sexpr("let value = try { 1; } catch error is int { 2; };"),
        "(LetStatement name=value type=_ value=(TryExpression body={1} catches=[(CatchClause name=error type=int body={2})]))"
    );
}

#[test]
fn generic_closers_split_compound_tokens() {
    for body in [
        "let value is Box<Box<int>> = Box();",
        "let value is Box<Box<int>>=Box();",
        "let value is Box<Box<Box<int>>>=Box();",
        "let value is Box<int>= Box();",
        "let value is Box<Box<int> > = Box(); value > other;",
        "let value is %string:Box<@int> = [:];",
    ] {
        common::parse_ok(&format!("module app v1.0.0; main() is int {{ {body} }}"));
    }
    // Expressions keep `>>` as a shift.
    assert_eq!(expr_sexpr("a >> b"), "(>> a b)");
}
