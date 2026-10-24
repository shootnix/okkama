//! Module structure, imports, objects, members, roles, enums, natives, caps
//! and test/fake blocks. Many cases are ported from okkama-pl/t/parser
//! (01–04, 08, 10, 11, 14, 15, 17, 22–25).

mod common;

use common::{assert_rejects, parse_err_in, parse_in, parse_ok, program_sexpr};
use okkama_compiler::syntax::ParseMode;
use okkama_compiler::syntax::ast::{
    ConstructorForm, FakeBody, Item, ObjectMember, RoleMember, TestBody, Visibility,
};

#[test]
fn module_header_forms() {
    let program = parse_ok("module app/api v1.2.3; main() is int { 0; }");
    let path: Vec<_> = program
        .module
        .path
        .iter()
        .map(|n| n.text.as_str())
        .collect();
    assert_eq!(path, ["app", "api"]);
    assert_eq!(program.module.version.text(), "v1.2.3");
    // Keyword-shaped lower-case path segments are namespace data.
    parse_ok("module test/more v1.0.0; object Assertions { Assertions(); }");
    // `v1.2 .3` lexes differently from `v1.2.3` but means the same version.
    assert_eq!(
        parse_ok("module app v1.2 .3;").module.version.text(),
        "v1.2.3"
    );
    // Library modules may omit main.
    parse_ok("module money v1.0.0; object Cash {} role Payable { pay() is ^Payable; }");
    parse_ok("module empty v1.0.0;");
}

#[test]
fn module_header_errors() {
    for (source, expected) in [
        (
            "main() is int { 0; }",
            "Program must start with module declaration.",
        ),
        ("", "Program must start with module declaration."),
        ("module App v1.0.0;", "Expect module name."),
        (
            "module app/; main() is int {}",
            "Expect module name part after \"/\".",
        ),
        (
            "module app v1.0.0 main() is int {}",
            "Expect \";\" after module declaration.",
        ),
        (
            "module app v1; main() is int {}",
            "Expect \".\" in module version.",
        ),
        (
            "module app v1.0; main() is int {}",
            "Expect \".\" in module version.",
        ),
        (
            "module app 1.0.0; main() is int {}",
            "Expect module version.",
        ),
        (
            "module app version.0.0;",
            "Expect module version like v1.0.0.",
        ),
        (
            "module app v1.x.0; main() is int {}",
            "Expect minor version number.",
        ),
        ("module app v1.0.x;", "Expect patch version number."),
    ] {
        assert_rejects(source, expected);
    }
}

#[test]
fn imports() {
    assert_eq!(
        program_sexpr(
            "module app v1.0.0; import \"money\"; import \"sql/pool\" as db_pool; \
             import \"io/stdout\" as out take println, printf;"
        ),
        "(Program module=(ModuleDecl path=[app] version=v1.0.0) imports=[\
         (ImportDecl path=\"money\" alias=_ take=[]) \
         (ImportDecl path=\"sql/pool\" alias=db_pool take=[]) \
         (ImportDecl path=\"io/stdout\" alias=out take=[println printf])] items=[])"
    );
    // A take list may name types, lower-case natives or keyword-shaped words.
    parse_ok("module app v1.0.0; import \"runtime/time\" take monotonic_clock, Clock;");
}

#[test]
fn import_errors() {
    for (source, expected) in [
        (
            "module app v1.0.0; object Cash {} import \"late\";",
            "Imports must appear before module declarations.",
        ),
        (
            "module app v1.0.0; import money;",
            "Expect import path after \"import\".",
        ),
        (
            "module app v1.0.0; import \"money\" as;",
            "Expect import alias after \"as\".",
        ),
        (
            "module app v1.0.0; import \"money\" take;",
            "Expect imported name after \"take\".",
        ),
        (
            "module app v1.0.0; import \"money\" take Cash,;",
            "Expect imported name after \",\".",
        ),
        (
            "module app v1.0.0; import \"money\"",
            "Expect \";\" after import declaration.",
        ),
    ] {
        assert_rejects(source, expected);
    }
}

#[test]
fn only_main_is_declared_at_module_scope() {
    assert_rejects(
        "module app v1.0.0; helper() is void {} main() is int {}",
        "Only main() may be declared at module scope.",
    );
    assert_rejects(
        "module app v1.0.0; main(args is @string) is int {}",
        "main() cannot declare parameters.",
    );
    assert_rejects(
        "module app v1.0.0; let x = 1;",
        "Expect \"(\" before action parameters.",
    );
    let program = parse_ok("module app v1.0.0; async main() is int { 0; }");
    let Item::Action(main) = &program.items[0] else {
        panic!()
    };
    assert!(main.is_async);
}

#[test]
fn object_members() {
    let source = "module app v1.0.0; object Cash { \
        has digit, cents is int; \
        has currency is string = \"USD\"; \
        has opened = false; \
        Cash(value is int) { digit = value; self.cents = 0; confirm currency == \"USD\"; } \
        destroy { digit; } \
        test \"currency\" { test.is(currency, \"USD\"); } \
        async test \"later\" { await x; } \
        is(actual is any, expected is any) is void {} \
        has(expected is string) is bool { false; } \
        withdrawal() is int { 1; } \
        private: helper() is void {} #projected(of) value() is int { 1; } \
        }";
    let program = parse_ok(source);
    let Item::Object(object) = &program.items[0] else {
        panic!()
    };
    let kinds: Vec<&str> = object
        .members
        .iter()
        .map(|member| match member {
            ObjectMember::Has(_) => "has",
            ObjectMember::Cap(_) => "cap",
            ObjectMember::Constructor(_) => "constructor",
            ObjectMember::Destroy(_) => "destroy",
            ObjectMember::Test(_) => "test",
            ObjectMember::Action(_) => "action",
            ObjectMember::Private(_) => "private",
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "has",
            "has",
            "has",
            "constructor",
            "destroy",
            "test",
            "test",
            "action",
            "action",
            "action",
            "private"
        ]
    );
    let ObjectMember::Private(section) = &object.members[10] else {
        panic!()
    };
    assert_eq!(section.actions.len(), 2);
    assert!(
        section
            .actions
            .iter()
            .all(|a| a.visibility == Visibility::Private)
    );
    assert_eq!(
        section.actions[1]
            .projection
            .as_ref()
            .unwrap()
            .alias
            .as_ref()
            .unwrap()
            .text,
        "of"
    );
    let ObjectMember::Test(test) = &object.members[6] else {
        panic!()
    };
    assert!(test.is_async);
}

#[test]
fn constructor_forms_stay_distinct() {
    let program = parse_ok(
        "module app v1.0.0; object Foo { has foo is int; has bar is string; \
         Foo(:bar, :foo); Foo(); Foo() {} Foo(:bar) { self.foo = 1; } Foo(first is int, rest is ...int) {} }",
    );
    let Item::Object(object) = &program.items[0] else {
        panic!()
    };
    let forms: Vec<String> = object
        .members
        .iter()
        .filter_map(|member| match member {
            ObjectMember::Constructor(ctor) => Some(match &ctor.form {
                ConstructorForm::Shorthand { fields, body } => format!(
                    "shorthand[{}]{}",
                    fields
                        .iter()
                        .map(|f| f.text.as_str())
                        .collect::<Vec<_>>()
                        .join(","),
                    if body.is_some() { "+body" } else { ";" }
                ),
                ConstructorForm::Full { params, .. } => format!(
                    "full({})",
                    params
                        .iter()
                        .map(|p| format!("{}{}", if p.variadic { "..." } else { "" }, p.name.text))
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            }),
            _ => None,
        })
        .collect();
    assert_eq!(
        forms,
        [
            "shorthand[bar,foo];",
            "shorthand[];",
            "full()",
            "shorthand[bar]+body",
            "full(first,...rest)"
        ]
    );
}

#[test]
fn shorthand_accepts_the_grammar_trailing_comma() {
    // field_param_list = field_param ("," field_param)* trailing_comma?
    // (conformance L01.gram.constructor.shorthand-trailing-comma; the Perl
    // reference rejects it, recorded as conformance defect DEF-001).
    parse_ok("module app v1.0.0; object Pair { has a is int; has b is int; Pair(:a, :b,); }");
    assert_rejects(
        "module app v1.0.0; object Pair { has a is int; Pair(:a,,); }",
        "Expect \":\" before constructor shorthand field.",
    );
}

#[test]
fn constructor_and_member_errors() {
    for (source, expected) in [
        ("object Foo { Foo(:); }", "Expect field name after \":\"."),
        (
            "object Foo { has foo is int; Foo(:foo, bar is string); }",
            "Constructor shorthand cannot mix field entries and typed parameters.",
        ),
        (
            "object Foo { has bar is string; Foo(bar is string, :bar); }",
            "Constructor shorthand cannot mix field entries and typed parameters.",
        ),
        (
            "object Foo { has foo is int; Foo(:foo) }",
            "Expect \";\" after constructor shorthand.",
        ),
        (
            "object User { Other(value is int) { other = value; } }",
            "Constructor name must match enclosing object.",
        ),
        (
            "object User(id is int) {}",
            "Object headers do not accept parameters in v1.0.",
        ),
        (
            "object Dog isa Animal {}",
            "Inheritance is not part of Okkama v1.0 syntax.",
        ),
        (
            "object User { has ; }",
            "Expect attribute name after \"has\".",
        ),
        (
            "object Box { has next is int; Box(); }",
            "Expect attribute name after \"has\".",
        ),
        (
            "object Box { Box(); echo(next is int) is int { 1; } }",
            "Expect parameter name.",
        ),
        (
            "object Box { Box(); value() { 1; } }",
            "Action requires an explicit result",
        ),
        (
            "object Box { Box(); value() is void; }",
            "Expect \"{\" before block.",
        ),
        (
            "object Box { Box(); &value() is int { 1; } }",
            "Object projection declarations use \"#projected\", not \"&\".",
        ),
        (
            "object Box { Box(); #projectd(of) value() is int { 1; } }",
            "Unknown object action directive \"projectd\".",
        ),
        (
            "object Box { Box(); #projected() value() is int { 1; } }",
            "Projection alias must be a lowercase identifier.",
        ),
        (
            "object Box { Box(); #projected #projected value() is int { 1; } }",
            "Expect action name.",
        ),
        (
            "object Box { Box(); private helper() is void {} }",
            "Expect \":\" after \"private\".",
        ),
        (
            "object Box { Box(); private: has x is int; }",
            "Expect \"(\" before action parameters.",
        ),
        ("object Box { 42; }", "Expect object member declaration."),
    ] {
        assert_rejects(&format!("module app v1.0.0; {source}"), expected);
    }
}

#[test]
fn with_names_are_forbidden_in_action_positions() {
    let diagnostic = "Action names \"with\" and \"with_*\" are forbidden";
    for source in [
        "object Box { Box(); with_value(value is int) is void {} }",
        "object Box { Box(); with() is void {} }",
        "role Boxed { with_value(value is int) is void; }",
        "object Box { Box(); private: with_value(value is int) is void {} }",
        "object Box { Box(); #projected with_value() is int { 1; } }",
        "native with_value(value is int) is int;",
        "object Box { Box(); } main() is int { Box().with_value(1); }",
    ] {
        assert_rejects(&format!("module app v1.0.0; {source}"), diagnostic);
    }
    assert!(
        parse_err_in(
            "module app v1.0.0; role Boxed { value() is int; fake { with_value(value is int) is void {} } }",
            ParseMode::Full
        )
        .contains(diagnostic)
    );
    parse_ok("module app v1.0.0; object Account { Account(); withdrawal() is int { 1; } }");
}

#[test]
fn variadic_parameters() {
    parse_ok(
        "module app v1.0.0; object Format { format(pattern is string, values is ...int) is string { \"ok\"; } \
         collect(groups is ...@int) is int { 0; } }",
    );
    for (source, expected) in [
        (
            "bad(values is ...int, suffix is string) is string { suffix; }",
            "Variadic parameter \"values\" must be the last parameter.",
        ),
        (
            "bad(left is ...int, right is ...int) is int { 0; }",
            "Action may declare only one variadic parameter.",
        ),
        (
            "bad(values is ...) is int { 0; }",
            "Expect element type after \"...\".",
        ),
        (
            "bad(values is ...int = 1) is int { 0; }",
            "Variadic parameter \"values\" cannot have a default value.",
        ),
        (
            "bad(value is int = 1) is int { 0; }",
            "Expect \")\" after action parameters.",
        ),
    ] {
        assert_rejects(
            &format!("module app v1.0.0; object Format {{ {source} }}"),
            expected,
        );
    }
}

#[test]
fn generic_declarations() {
    let program = parse_ok(
        "module app v1.0.0; role Comparing<T> { order(other is T) is int; } \
         object Box<T: Comparing<T>> { Box(); } object Pair<K, V,> { Pair(); }",
    );
    let Item::Object(boxed) = &program.items[1] else {
        panic!()
    };
    assert_eq!(boxed.generics.len(), 1);
    assert!(boxed.generics[0].bound.is_some());
    let Item::Object(pair) = &program.items[2] else {
        panic!()
    };
    assert_eq!(pair.generics.len(), 2);
    for (source, expected) in [
        (
            "object Box<> { Box(); }",
            "Generic object parameter list requires at least one type parameter.",
        ),
        (
            "object Box<T, T> { Box(); }",
            "Generic type parameter \"T\" is declared more than once.",
        ),
        (
            "object Box<int> { Box(); }",
            "Generic type parameter must be a type name.",
        ),
        (
            "object Box<Box> { Box(); }",
            "Generic type parameter \"Box\" cannot shadow object \"Box\".",
        ),
        (
            "role Mapping<> { put() is int; }",
            "Generic role parameter list requires at least one type parameter.",
        ),
        (
            "role Mapping<Mapping> { put() is int; }",
            "Generic type parameter \"Mapping\" cannot shadow role \"Mapping\".",
        ),
        (
            "role Bounded<T: Comparing<T>> { value() is T; }",
            "Generic bounds are currently supported only on object declarations.",
        ),
    ] {
        assert_rejects(&format!("module app v1.0.0; {source}"), expected);
    }
}

#[test]
fn type_references() {
    let source = "module app v1.0.0; role Store { \
        save(items is @@Item, index is Map<string, Item>, m is %string:@int) is Result<Item>; \
        apply(op is (int, string) is bool, f is () is void, g is async (Request) is Response) is ^Store; \
        card(c is Customer::Public, q is io.Package) is any; }";
    assert_eq!(
        program_sexpr(source),
        "(Program module=(ModuleDecl path=[app] version=v1.0.0) imports=[] items=[(RoleDecl name=Store generics=[] members=[\
         (ActionSignature async=false name=save params=[items @@Item index Map<string, Item> m %string:@int] result=Result<Item>) \
         (ActionSignature async=false name=apply params=[op (int, string) is bool f () is void g async (Request) is Response] result=^Store) \
         (ActionSignature async=false name=card params=[c Customer::Public q io.Package] result=any)])])"
    );
    for (source, expected) in [
        ("save(item is Result<>) is int;", "Expect type argument."),
        ("save(item is Result<Item,>) is int;", "Expect name."),
        (
            "save(item is %string) is int;",
            "Expect \":\" between map key and value types.",
        ),
        (
            "apply(op is (int) int) is int;",
            "Expect \"is\" after action parameter types.",
        ),
        (
            "apply(op is (int,) is int) is int;",
            "Expect action parameter type after \",\".",
        ),
        ("factory() is () is;", "Expect name."),
        (
            "value(v is void) is int;",
            "void is only allowed as an action result marker.",
        ),
        (
            "value(v is async int) is int;",
            "Expect \"(\" after \"async\" in an async action type.",
        ),
        (
            "value() is User?;",
            "Optional types are not part of Okkama v1.0 syntax.",
        ),
    ] {
        assert_rejects(
            &format!("module app v1.0.0; role R {{ {source} }}"),
            expected,
        );
    }
}

#[test]
fn roles_fakes_and_tests() {
    let source = "module app v1.0.0; role Package { open() is ^Package; async load() is int; \
        has(expected is string) is bool; \
        fake { has opened = false; Package(); open() is ^Package { ^opened = true; } destroy {} } \
        test \"fake\" { test.ok(true); } }";
    let program = parse_ok(source);
    let Item::Role(role) = &program.items[0] else {
        panic!()
    };
    assert_eq!(role.members.len(), 5);
    let RoleMember::Fake(fake) = &role.members[3] else {
        panic!()
    };
    let FakeBody::Parsed(members) = &fake.body else {
        panic!()
    };
    assert_eq!(members.len(), 4);
    for (source, expected) in [
        (
            "role Package { has opened = false; }",
            "Expect \"(\" before action signature parameters.",
        ),
        (
            "role Value { #projected value() is int; }",
            "Expect role member declaration.",
        ),
        (
            "role Named { cap Card {} }",
            "Caps are only allowed inside objects.",
        ),
        (
            "role Named { fake { cap Card {} } }",
            "Caps are only allowed inside objects.",
        ),
        (
            "role Named { value() is int { 1; } }",
            "Expect \";\" after action signature.",
        ),
    ] {
        assert_rejects(&format!("module app v1.0.0; {source}"), expected);
    }
}

#[test]
fn production_mode_skips_test_and_fake_bodies_but_keeps_the_nodes() {
    let source = "module app v1.0.0;\n\
        test \"ignored grammar\" { if true { let = ; } }\n\
        role Readable { read() is int; fake { if true { let = ; } } }\n\
        main() is int { 7; }\n";
    let program = parse_in(source, ParseMode::Production).expect("bodies are not parsed");
    let Item::Test(test) = &program.items[0] else {
        panic!()
    };
    assert!(matches!(test.body, TestBody::Skipped(_)));
    let Item::Role(role) = &program.items[1] else {
        panic!()
    };
    let RoleMember::Fake(fake) = &role.members[1] else {
        panic!()
    };
    assert!(matches!(fake.body, FakeBody::Skipped(_)));
    // Full mode parses the same bodies and reports the inner error.
    assert_eq!(
        parse_err_in(source, ParseMode::Full),
        "Expect variable name after \"let\"."
    );
    // Production still needs balanced braces and a string test name.
    assert_eq!(
        parse_err_in(
            "module app v1.0.0; test \"open\" { {",
            ParseMode::Production
        ),
        "Expect \"}\" after test block."
    );
    assert_eq!(
        parse_err_in("module app v1.0.0; test unquoted {}", ParseMode::Production),
        "Expect test name after \"test\"."
    );
    // Lexical errors inside skipped bodies still count.
    assert!(
        parse_in(
            "module app v1.0.0; test \"x\" { \"unterminated }",
            ParseMode::Production
        )
        .is_err()
    );
}

#[test]
fn enums() {
    let program = parse_ok("module app v1.0.0; enum State = (open, closed); enum Empty = ();");
    let Item::Enum(state) = &program.items[0] else {
        panic!()
    };
    assert_eq!(state.cases.len(), 2);
    for (source, expected) in [
        ("enum state = (a);", "Expect enum name after \"enum\"."),
        ("enum State (a);", "Expect \"=\" after enum name."),
        ("enum State = (a,);", "Expect enum case name."),
        ("enum State = (Open);", "Expect enum case name."),
        ("enum State = (a)", "Expect \";\" after enum declaration."),
    ] {
        assert_rejects(&format!("module app v1.0.0; {source}"), expected);
    }
}

#[test]
fn native_declarations() {
    let program = parse_ok(
        "module runtime/time v1.0.0; native monotonic_clock() is int; \
         native async sleep_async(duration is int) is bool; native spawn(program is @byte) is int; \
         native object FileHandle; native close(handle is FileHandle) is void;",
    );
    assert_eq!(program.items.len(), 5);
    let Item::NativeAction(sleep) = &program.items[1] else {
        panic!()
    };
    assert!(sleep.is_async);
    for (source, expected) in [
        (
            "native monotonic_clock();",
            "Native declaration \"monotonic_clock\" requires a return type.",
        ),
        (
            "native monotonic_clock() is int",
            "Expect \";\" after native declaration.",
        ),
        (
            "native monotonic_clock() is int { 1; }",
            "Native declaration requires \";\" and cannot have a body.",
        ),
        (
            "native main() is int;",
            "Native declaration cannot be named \"main\".",
        ),
        (
            "async native monotonic_clock() is int;",
            "Native declarations cannot be async.",
        ),
        (
            "native collect(values is ...int) is int;",
            "Variadic native declarations are not supported in Phase 01.",
        ),
        (
            "native clock(unit is string = \"ns\") is int;",
            "Native parameters cannot have default values.",
        ),
        (
            "native object file_handle;",
            "Expect native object type name.",
        ),
        (
            "native object FileHandle<T>;",
            "Native object declarations cannot have type parameters.",
        ),
        (
            "native object FileHandle {}",
            "Native object declaration requires \";\" and cannot have a body.",
        ),
        (
            "native object FileHandle",
            "Expect \";\" after native object declaration.",
        ),
        (
            "object Clock { native tick() is int; }",
            "Native declarations are allowed only at module scope.",
        ),
        (
            "role Clock { native tick() is int; }",
            "Native declarations are allowed only at module scope.",
        ),
        (
            "main() is int { native tick() is int; }",
            "Native declarations are allowed only at module scope.",
        ),
        (
            "object Wrapper { Wrapper(); native object FileHandle; }",
            "Native declarations are allowed only at module scope.",
        ),
    ] {
        assert_rejects(&format!("module app v1.0.0; {source}"), expected);
    }
}

#[test]
fn caps_and_owner_field_reads() {
    let source = "module app v1.0.0;\n\
        object Customer { has email is string; Customer(:email);\n\
          cap Public { email is string { email; } } }\n\
        cap Customer::Card { fullName is string { Customer::name; } }\n\
        main() is int { let card is Customer::Public = Customer(email: \"m\")::Public; card::email; 0; }\n";
    let sexpr = program_sexpr(source);
    assert!(sexpr.contains("(CapDecl owner=_ name=Public"), "{sexpr}");
    assert!(sexpr.contains("(CapDecl owner=Customer name=Card fields=[(ComputedField name=fullName type=string body={(owner:: Customer name)})])"), "{sexpr}");
    assert!(
        sexpr.contains("(:: (call Customer email:\"m\") Public)"),
        "{sexpr}"
    );
    assert!(sexpr.contains("(:: card email)"), "{sexpr}");
    // Outside an external cap, `Owner::x` is an ordinary selection.
    assert!(
        program_sexpr("module app v1.0.0; main() is int { Customer::name; }")
            .contains("(:: Customer name)")
    );
    for member in [
        "does Named {}",
        "{ has state is int; }",
        "{ method() is int { 1; } }",
        "{ async value is int { 1; } }",
        "{ Card() {} }",
        "{ change() is ^Owner { ^n=2; } }",
        "{ destroy {} }",
        "{ cap Nested {} }",
        "{ test \"field\" {} }",
        "{ value is ^int { 1; } }",
    ] {
        let message = common::parse_err(&format!(
            "module app v1.0.0; object Owner {{ Owner(); cap Card {member} }}"
        ));
        assert!(
            message.contains("cap") || message.contains("computed field"),
            "{member}: {message}"
        );
    }
    assert_rejects(
        "module app v1.0.0; cap Card {}",
        "A module-level cap requires Owner::Name.",
    );
    // `cap` and `view` stay ordinary identifiers elsewhere.
    parse_ok(
        "module app v1.0.0; object Values { has mask is int; Values(:mask); cap() is int { mask; } }",
    );
    parse_ok(
        "module app v1.0.0; object C { has view is int; C(:view); view() is int { view; } } main() is int { let view = C(1); view.view(); 0; }",
    );
}

#[test]
fn removed_object_views() {
    for declaration in ["view Presentation(email);", "view All(*);", "view Empty();"] {
        assert_rejects(
            &format!(
                "module app v1.0.0; object Customer {{ has email is string; Customer(:email); {declaration} }}"
            ),
            "Object views have been removed.",
        );
    }
    assert_rejects(
        "module app v1.0.0; view Public(email);",
        "Object views have been removed.",
    );
    assert_rejects(
        "module app v1.0.0; role R { view Public(email); }",
        "Object views have been removed.",
    );
}

#[test]
fn removed_allow_and_while_forms_stay_invalid() {
    // `allow` grants were removed (docs/allow-removal.md); `allow` is an
    // ordinary identifier now, so the old declarations no longer parse.
    for member in ["allow B;", "allow B.read;", "allow Reader, Writer;"] {
        assert_rejects(
            &format!("module app v1.0.0; object A {{ has x is int; A(:x); {member} }}"),
            "Expect \"(\" before action parameters.",
        );
    }
    assert_rejects(
        "module app v1.0.0; main() is int { while true { 1; } }",
        "Expect \";\" after expression.",
    );
    // As ordinary names they remain usable.
    parse_ok("module app v1.0.0; main() is int { let allow = 1; let while = 2; allow + while; }");
}
