//! Statements. Many cases are ported from okkama-pl/t/parser (05, 09, 13,
//! 16, 21-return).

mod common;

use common::{assert_rejects, body_sexpr, parse_ok};

fn wrap(body: &str) -> String {
    format!("module app v1.0.0; main() is int {{ {body} }}")
}

#[test]
fn let_defer_confirm_and_bindings() {
    assert_eq!(
        body_sexpr(
            "let x is int = 1 + 2; defer log.close(); confirm x > 0 else panic InvalidX(x); confirm ok else throw Problem();"
        ),
        "(LetStatement name=x type=int value=(+ 1 2)); \
         (DeferStatement value=(call (. log close))); \
         (ConfirmStatement condition=(> x 0) fallback=(ConfirmPanic value=(call InvalidX x))); \
         (ConfirmStatement condition=ok fallback=(ConfirmThrow value=(call Problem)))"
    );
    // Binding names may use any case.
    assert_eq!(
        body_sexpr("let USER_NAME = 1; let UserName = 2;"),
        "(LetStatement name=USER_NAME type=_ value=1); (LetStatement name=UserName type=_ value=2)"
    );
    assert_eq!(
        body_sexpr("total = total + 1; self.value = input; ^opened = true;"),
        "(FieldBindStatement name=total value=(+ total 1)); \
         (SelfFieldBindStatement name=value value=input); \
         (SuccessorAssignStatement name=opened value=true)"
    );
}

#[test]
fn statement_errors() {
    for (body, expected) in [
        ("let x is int 1;", "Expect \"=\" after variable name."),
        ("let x = 1", "Expect \";\" after let declaration."),
        (
            "confirm false else 42;",
            "Expect \"throw\" or \"panic\" after confirm else.",
        ),
        ("confirm false else throw;", "Expect expression."),
        (
            "confirm false else panic;",
            "\"panic;\" is not supported; use \"throw;\" to rethrow.",
        ),
        (
            "other.value = 2;",
            "Only \"self.field\" may be assigned; object fields are not mutable.",
        ),
        ("self.value = ;", "Expect expression."),
        ("Value = 2;", "Expect field name."),
        (
            "^Opened = true;",
            "Expect successor field name after \"^\".",
        ),
        ("^opened true;", "Expect \"=\" after successor field name."),
        ("defer close()", "Expect \";\" after defer statement."),
        ("1 2;", "Expect \";\" after expression."),
        (
            "let value = throw Problem();",
            "Expect \";\" after let declaration.",
        ),
    ] {
        assert_rejects(&wrap(body), expected);
    }
}

#[test]
fn if_statements_chain_else_if() {
    assert_eq!(
        body_sexpr("if a { 1; } else if b { 2; } else { 3; }"),
        "(IfStatement condition=a then={1} else=(IfStatement condition=b then={2} else={3}))"
    );
    assert_eq!(
        body_sexpr("if a { 1; }"),
        "(IfStatement condition=a then={1} else=_)"
    );
    for (body, expected) in [
        ("if a 1;", "Expect \"{\" before block."),
        ("else { 1; }", "Expect \";\" after expression."),
        ("if a { 1; } else 2;", "Expect \"{\" before block."),
    ] {
        assert_rejects(&wrap(body), expected);
    }
}

#[test]
fn match_statements() {
    assert_eq!(
        body_sexpr("match x { case 1, 2 { small(); } else { big(); } }"),
        "(MatchStatement subject=x cases=[(MatchCase patterns=[1 2] body={(call small)})] else={(call big)})"
    );
    for (body, expected) in [
        (
            "match x { else {} else {} }",
            "Match cannot have multiple else branches.",
        ),
        ("match x { 1 {} }", "Expect \"case\" or \"else\" in match."),
        ("match x case 1 {}", "Expect \"{\" after match subject."),
    ] {
        assert_rejects(&wrap(body), expected);
    }
    assert_rejects(
        "module app v1.0.0; main() is int { match x { case 1 {}",
        "Expect \"}\" after match.",
    );
}

#[test]
fn for_statements_own_the_top_level_arrow() {
    assert_eq!(
        body_sexpr("for users -> user { user.name; }"),
        "(ForStatement source=users bindings=[(ForBinding name=user batch=_)] body={(. user name)})"
    );
    assert_eq!(
        body_sexpr("for pairs -> key, value { key; }"),
        "(ForStatement source=pairs bindings=[(ForBinding name=key batch=_) (ForBinding name=value batch=_)] body={key})"
    );
    assert_eq!(
        body_sexpr("for rows -> batch[10] { batch; }"),
        "(ForStatement source=rows bindings=[(ForBinding name=batch batch=10)] body={batch})"
    );
    // A member call before the arrow is the source, not a functional suffix.
    assert_eq!(
        body_sexpr("for mapper.map() -> value { value; }"),
        "(ForStatement source=(call (. mapper map)) bindings=[(ForBinding name=value batch=_)] body={value})"
    );
    assert_eq!(
        body_sexpr("for (values.map() -> item { item; }) -> value { value; }"),
        "(ForStatement source=(group (map values -> [item] {item})) bindings=[(ForBinding name=value batch=_)] body={value})"
    );
    // Only the top level of the source gives the arrow to the `for`; nested
    // expressions such as arguments complete their functional calls.
    assert_eq!(
        body_sexpr("for mapper.items(values.map() -> item { item; }) -> value { value; }"),
        "(ForStatement source=(call (. mapper items) (map values -> [item] {item})) bindings=[(ForBinding name=value batch=_)] body={value})"
    );
    assert_eq!(
        body_sexpr("for a + mapper.map() -> value { value; }"),
        "(ForStatement source=(+ a (call (. mapper map))) bindings=[(ForBinding name=value batch=_)] body={value})"
    );
    for (body, expected) in [
        (
            "for users user { user; }",
            "Expect \"->\" and a binding after for source.",
        ),
        (
            "for { last; }",
            "Source-less \"for\" is not supported; use \"loop { ... }\".",
        ),
        (
            "for [1, 2] { 1; }",
            "Expect \"->\" and a binding after for source.",
        ),
        ("for rows -> batch[n] { }", "Expect batch size integer."),
        ("for rows -> batch[1 { }", "Expect \"]\" after batch size."),
    ] {
        assert_rejects(&wrap(body), expected);
    }
}

#[test]
fn loop_next_and_last() {
    assert_eq!(
        body_sexpr(
            "loop { next; next if retry; last; last if done; last answer; last answer if done; }"
        ),
        "(loop {(NextStatement condition=_); (NextStatement condition=retry); \
         (LastStatement value=_ condition=_); (LastStatement value=_ condition=done); \
         (LastStatement value=answer condition=_); (LastStatement value=answer condition=done)})"
    );
    // A statement-level loop needs no semicolon, but may have one.
    assert_eq!(
        body_sexpr("loop { last 1; } loop { last 2; };"),
        "(loop {(LastStatement value=1 condition=_)}); (loop {(LastStatement value=2 condition=_)})"
    );
    assert_eq!(
        body_sexpr("loop { last (if ready { 1; } else { 2; }); }"),
        "(loop {(LastStatement value=(group (if ready {1} {2})) condition=_)})"
    );
    for (body, expected) in [
        ("loop 1;", "Expect \"{\" after \"loop\"."),
        ("loop { next 1; }", "\"next\" does not accept a value."),
        ("loop { last 1 if; }", "Expect expression."),
        ("next if;", "Expect expression."),
    ] {
        assert_rejects(&wrap(body), expected);
    }
}

#[test]
fn postfix_if_on_expression_statements() {
    assert_eq!(body_sexpr("log(x) if ready;"), "(call log x) if ready");
}

#[test]
fn try_throw_and_return() {
    assert_eq!(
        body_sexpr(
            "try { panic \"x\"; } catch text is string { text; } catch problem is Problem {} throw Problem(42); throw; return 1; return;"
        ),
        "(TryStatement body={(panic \"x\")} catches=[(CatchClause name=text type=string body={text}) \
         (CatchClause name=problem type=Problem body={})]); (ThrowStatement value=(call Problem 42)); \
         (ThrowStatement value=_); (ReturnStatement value=1); (ReturnStatement value=_)"
    );
    parse_ok(&wrap(
        "try { x; } catch a is @int {} catch b is %string:int {} catch c is State {}",
    ));
    for (body, expected) in [
        ("try {}", "Expect \"catch\" after try block."),
        (
            "try {} catch is Problem {}",
            "Expect error name after \"catch\".",
        ),
        (
            "try {} catch error Problem {}",
            "Expect \"is\" after catch error name.",
        ),
        (
            "try {} catch error is {}",
            "Expect error type after \"is\".",
        ),
        (
            "try {} catch error is Problem error;",
            "Expect \"{\" before catch body.",
        ),
        (
            "catch error is Problem {}",
            "\"catch\" must follow a try block.",
        ),
        (
            "try {} catch error is Problem {} finally {}",
            "finally is not supported.",
        ),
        ("finally {}", "finally is not supported."),
        (
            "return 1, 2;",
            "Expect \";\" after return; only one value is allowed.",
        ),
        ("throw 1, 2;", "Expect \";\" after throw statement."),
    ] {
        assert_rejects(&wrap(body), expected);
    }
}

#[test]
fn nested_block_statements() {
    assert_eq!(
        body_sexpr("{ let a = 1; { a; } }"),
        "(BlockStatement statements=[(LetStatement name=a type=_ value=1) (BlockStatement statements=[a])])"
    );
    // A statement-level `try`/`if` is not an expression: a following `;`
    // is an empty expression statement, which is invalid.
    assert_rejects(&wrap("if a { 1; } ;"), "Expect expression.");
}

#[test]
fn reserved_words_cannot_be_bindings() {
    for keyword in [
        "else", "false", "for", "if", "true", "let", "object", "is", "as", "does", "role",
        "import", "has", "confirm", "private", "enum", "match", "case", "defer", "try", "catch",
        "finally", "next", "last", "async", "await", "panic", "self", "module", "take", "native",
        "test", "fake", "destroy", "with",
    ] {
        let message = common::parse_err(&wrap(&format!("let {keyword} = 1;")));
        assert!(
            message.contains("Expect variable name after \"let\"")
                || message.contains("is reserved"),
            "{keyword}: {message}"
        );
    }
    for (word, expected) in [
        (
            "none",
            "Null/none/nil values are not part of Okkama v1.0 syntax.",
        ),
        (
            "null",
            "Null/none/nil values are not part of Okkama v1.0 syntax.",
        ),
        (
            "nil",
            "Null/none/nil values are not part of Okkama v1.0 syntax.",
        ),
        (
            "super.action()",
            "Inheritance is not part of Okkama v1.0 syntax.",
        ),
        (
            "user?.name",
            "Optional chaining is not part of Okkama v1.0 syntax.",
        ),
        (
            "abstract",
            "\"abstract\" is reserved and not part of Okkama v1.0 syntax.",
        ),
        (
            "protected",
            "\"protected\" is reserved and not part of Okkama v1.0 syntax.",
        ),
    ] {
        assert_rejects(&wrap(&format!("{word};")), expected);
    }
    // Words that are not reserved (owner decision D-005).
    parse_ok(&wrap(
        "let var = 1; let optional = 2; let mutable = 3; let setters = 4;",
    ));
}
