//! Lowering ролей и модели отказов: `does`, ограниченные ссылки, `throw`, `try`/`catch`.

use okkama_compiler::SourceFile;
use okkama_compiler::ir;
use okkama_compiler::lowering::{LoweringError, lower_program};
use okkama_compiler::syntax::{self, ParseOptions};

const DECLS: &str = "
object Boom does Error { has code is int; Boom(:code); text() is string { \"boom\"; } code_value() is int { code; } }
role Wallet { total() is int; add(extra is int) is int; }
object Purse does Wallet {
    has digit is int;
    Purse(:digit);
    total() is int { digit; }
    add(extra is int) is int { digit + extra; }
    secret() is int { 0; }
}
object C { has n is int = 0; C() {} inc() is ^C { ^n = n + 1; } value() is int { n; } }
";

fn lower(text: &str) -> Result<ir::Module, LoweringError> {
    let source = SourceFile::new("test.ok", text).expect("small source");
    let program = syntax::parse(&source, &ParseOptions::default()).expect("valid syntax");
    lower_program(&program)
}

fn module(main: &str) -> ir::Module {
    let module = lower(&format!(
        "module app v1.0.0;{DECLS} main() is int {{ {main} }}"
    ))
    .unwrap_or_else(|error| panic!("{main}: {error}"));
    ir::verify(&module).expect("lowering output verifies");
    module
}

fn main_listing(main: &str) -> String {
    let text = module(main).to_string();
    let start = text.find("func @main").expect("main is printed");
    // Только `main`: lowering по достижимости печатает её первой.
    let end = text[start..]
        .find("\n}\n")
        .map_or(text.len(), |end| start + end + 3);
    text[start..end].to_string()
}

fn error(main: &str) -> String {
    lower(&format!(
        "module app v1.0.0;{DECLS} main() is int {{ {main} }}"
    ))
    .expect_err(main)
    .message()
    .to_string()
}

#[test]
fn roles_are_declared_with_witness_tables() {
    let text = module("0;").to_string();
    assert!(
        text.contains("role Wallet {\n    total() -> int\n    add(int) -> int\n}\n"),
        "{text}"
    );
    assert!(
        text.contains("role Error [builtin] {\n    text() -> str\n}\n"),
        "{text}"
    );
    assert!(
        text.contains("    does Error { text = @Boom.text }\n"),
        "{text}"
    );
    assert!(
        text.contains("    does Wallet { total = @Purse.total, add = @Purse.add }\n"),
        "{text}"
    );
}

#[test]
fn role_boundaries_wrap_and_role_calls_use_slots() {
    let main = main_listing("let w is Wallet = Purse(digit: 3); w.total() + w.add(extra: 1);");
    assert!(main.contains("%2: Wallet = role.wrap %1\n"), "{main}");
    assert!(main.contains("= call.role %2, Wallet.total()\n"), "{main}");
    assert!(main.contains("= call.role %2, Wallet.add(%"), "{main}");
    // Ограниченная ссылка не открывает остальные actions объекта.
    assert_eq!(
        error("let w is Wallet = Purse(digit: 3); w.secret();"),
        "Role \"Wallet\" has no action \"secret\""
    );
}

#[test]
fn try_catch_lowers_to_dispatch_handlers_and_causes() {
    assert_eq!(
        main_listing(
            "let c = C(); let r = try { c = c.inc(); throw Boom(code: 1); } \
             catch e is Boom { e.code_value(); }; r + c.value();"
        ),
        concat!(
            "func @main() -> int [entry] {\n",
            "^b0:\n",
            "    %0: C = call @C.new#0()\n",
            "    br ^b1\n",
            "^b1 unwind ^b3(%0):\n",
            "    %1: ^C = call @C.inc(%0)\n",
            "    %2: C = succ.take %1\n",
            "    br ^b2\n",
            "^b2 unwind ^b3(%2):\n",
            "    %3: int = const 1\n",
            "    %4: Boom = call @Boom.new#0(%3)\n",
            "    throw %4\n",
            "^b3(%5: C, %6: failure):\n",
            "    catch %6 is Boom, ^b4(%5, %6), ^b7(%5, %6)\n",
            "^b4(%7: C, %8: failure, %9: Boom) unwind ^b6(%7, %8):\n",
            "    %10: int = call @Boom.code_value(%9)\n",
            "    br ^b5\n",
            "^b5:\n",
            "    %11: int = call @C.value(%7)\n",
            "    %12: int = int.add %10, %11\n",
            "    ret %12\n",
            "^b6(%13: C, %14: failure, %15: failure):\n",
            "    %16: failure = fail.caused %15, %14\n",
            "    raise %16\n",
            "^b7(%17: C, %18: failure):\n",
            "    raise %18\n",
            "}\n",
        )
    );
}

#[test]
fn rethrow_and_confirm_else_throw() {
    let main = main_listing(
        "let a = try { try { 1; } catch e is Boom { throw; } } catch e is Error { 2; }; \
         confirm a > 0 else throw Boom(code: a); a;",
    );
    assert!(main.contains("raise %"), "{main}");
    assert!(main.contains("catch %"), "{main}");
    assert!(main.contains(" is Error, "), "{main}");
    assert!(main.contains("throw %"), "{main}");
}

#[test]
fn failure_rules_use_okkama_messages() {
    for (main, expected) in [
        (
            "throw C(); 0;",
            "Type \"C\" cannot be thrown; only objects that do Error are catchable",
        ),
        (
            "throw 5; 0;",
            "Type \"int\" cannot be thrown; only objects that do Error are catchable",
        ),
        (
            "try { 1; } catch e is C { 2; }",
            "Catch type \"C\" does not satisfy Error",
        ),
        (
            "try { 1; } catch e is Wallet { 2; }",
            "Catch type \"Wallet\" does not satisfy Error",
        ),
        (
            "throw; 0;",
            "\"throw;\" is available only inside a catch body",
        ),
    ] {
        assert_eq!(error(main), expected, "{main}");
    }
    for (decls, expected) in [
        (
            "object Q does Error { Q(); }",
            "Object \"Q\" does role \"Error\" but does not implement action \"text\"",
        ),
        (
            "role R { go(x is int) is int; } object Q does R { Q(); go(x is string) is int { 0; } }",
            "Action \"go\" parameter 1 on object \"Q\" must be int for role \"R\", got string",
        ),
        (
            "role R { go() is int; } object Q does R { Q(); go() is string { \"\"; } }",
            "Action \"go\" on object \"Q\" must return int for role \"R\", got string",
        ),
        (
            "role R { go(x is int) is int; } object Q does R { Q(); go(x is int, y is int) is int { 0; } }",
            "Object \"Q\" does role \"R\" but action \"go\" does not accept 1 parameter(s)",
        ),
        (
            "role R { go() is int; } object Q does R { Q(); private: go() is int { 0; } }",
            "Role \"R\" has no public implementation of action \"go\"",
        ),
        (
            "object Error { Error(); }",
            "\"Error\" is the name of the system role",
        ),
    ] {
        let text = format!("module app v1.0.0; {decls} main() is int {{ 0; }}");
        let message = lower(&text).expect_err(decls).message().to_string();
        assert_eq!(message, expected, "{decls}");
    }
}
