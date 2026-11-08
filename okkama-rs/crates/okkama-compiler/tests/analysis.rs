//! Семантический анализ: проверка до исполнения, независимо от backend.

use okkama_compiler::SourceFile;
use okkama_compiler::analysis::thir::{Coercion, ExprKind, StmtKind};
use okkama_compiler::analysis::{self, SemanticError, SourceModule};
use okkama_compiler::syntax::{self, ParseOptions, prune};

/// Ошибки анализа одного модуля (пусто — программа корректна).
fn errors(text: &str) -> Vec<SemanticError> {
    let source = SourceFile::new("test.ok", text).expect("small source");
    let program = prune::strip_test_items(
        syntax::parse(&source, &ParseOptions::default()).expect("valid syntax"),
    );
    let module = SourceModule {
        path: "app".to_string(),
        program: &program,
        imports: Vec::new(),
    };
    match analysis::analyze(vec![module]) {
        Ok(_) => Vec::new(),
        Err(errors) => errors,
    }
}

fn messages(text: &str) -> Vec<String> {
    errors(&format!("module app v1.0.0; {text}"))
        .into_iter()
        .map(|error| error.message)
        .collect()
}

#[test]
fn every_independent_error_is_reported() {
    assert_eq!(
        messages("main() is int { let a = 1 + true; let b = \"x\" - 2; let c = missing; 0; }"),
        [
            "Operator + cannot be applied to int and bool",
            "Operator - cannot be applied to string and int",
            "Undefined variable \"missing\"",
        ]
    );
}

#[test]
fn errors_do_not_cascade_from_poisoned_values() {
    // `a` уже ошибочна: её использование не порождает новых сообщений.
    assert_eq!(
        messages("main() is int { let a = 1 + true; let b = a * 2; a; }"),
        ["Operator + cannot be applied to int and bool"]
    );
}

#[test]
fn generic_bodies_are_checked_without_a_specialization() {
    assert_eq!(
        messages(
            "object Box<T> { has v is T; Box(:v); bad() is int { \"oops\" + 1; } } \
             main() is int { 0; }"
        ),
        ["Operator + cannot be applied to string and int"]
    );
    // Параметр типа непрозрачен: действия — только через ограничение-роль.
    assert_eq!(
        messages(
            "object Box<T> { has v is T; Box(:v); show() is string { v.string(); } } \
             main() is int { 0; }"
        ),
        ["Type parameter \"T\" has no action \"string\"; declare a role bound"]
    );
    assert!(
        messages(
            "role Ranked { rank() is int; } \
             object Best<T: Ranked> { has v is T; Best(:v); score() is int { v.rank(); } } \
             main() is int { 0; }"
        )
        .is_empty()
    );
}

#[test]
fn constructs_without_a_backend_are_still_checked() {
    // `rat` и async lowering ещё не умеет, но их правила проверяются.
    assert_eq!(
        messages("main() is int { let r = rat(1, 2); let x = r + 1; 0; }"),
        ["Operator + cannot be applied to rat and int"]
    );
    assert!(messages("main() is int { let r = rat(1, 2) + 1.rat(); 0; }").is_empty());
    assert_eq!(
        messages(
            "object W { W(); async value() is int { 7; } } \
             main() is int { let v = await W().value(); v; }"
        ),
        ["await is available only inside an async action"]
    );
    assert!(
        messages(
            "object W { W(); async value() is int { 7; } } \
             async main() is int { let v = await W().value(); v; }"
        )
        .is_empty()
    );
}

#[test]
fn objects_cross_role_parameters_and_overloads_pick_the_most_specific() {
    let shapes = "role Shape { area() is int; } \
        object Sq does Shape { has s is int; Sq(:s); area() is int { s * s; } }";
    assert!(
        messages(&format!(
            "{shapes} object Calc {{ Calc(); twice(shape is Shape) is int {{ shape.area() * 2; }} }} \
             main() is int {{ Calc().twice(Sq(s: 3)); }}"
        ))
        .is_empty()
    );
    // Точный тип специфичнее роли (§11.8).
    assert!(
        messages(&format!(
            "{shapes} object Calc {{ Calc(); f(shape is Shape) is int {{ 1; }} f(square is Sq) is int {{ 2; }} }} \
             main() is int {{ Calc().f(Sq(s: 3)); }}"
        ))
        .is_empty()
    );
}

#[test]
fn branch_values_cross_the_expected_type_one_by_one() {
    let shapes = "role Shape { area() is int; } \
        object Sq does Shape { has s is int; Sq(:s); area() is int { s * s; } } \
        object Dot does Shape { Dot(); area() is int { 0; } }";
    assert!(
        messages(&format!(
            "{shapes} object Pick {{ Pick(); one(big is bool) is Shape {{ if big {{ Sq(s: 2); }} else {{ Dot(); }} }} }} \
             main() is int {{ Pick().one(true).area(); }}"
        ))
        .is_empty()
    );
    assert_eq!(
        messages(&format!(
            "{shapes} main() is int {{ let x = if true {{ Sq(s: 2); }} else {{ Dot(); }}; 0; }}"
        )),
        Vec::<String>::new(),
        "a literal condition does not join the dead branch"
    );
    assert_eq!(
        messages(&format!(
            "{shapes} main() is int {{ let c = 1 > 0; let x = if c {{ Sq(s: 2); }} else {{ Dot(); }}; 0; }}"
        )),
        ["Branches produce different types: Sq and Dot"]
    );
}

#[test]
fn heterogeneous_literals_box_their_elements_into_any() {
    // R08: разнородный литерал — `@any`, каждый элемент упакован явно.
    assert!(messages("main() is int { let xs = [1, \"a\"]; let ys is @any = xs; 0; }").is_empty());
    assert_eq!(
        messages("main() is int { let xs = [1, \"a\"]; let ys is @int = xs; 0; }"),
        ["Cannot initialize variable \"ys\" of type @int with @any"]
    );
    // Объекты одной роли без цели — тоже `@any`: общая роль не выводится.
    assert!(messages("main() is int { let xs is @int = [1, 2..=3]; 0; }").is_empty());
}

#[test]
fn byte_values_are_not_map_keys() {
    assert_eq!(
        messages("main() is int { let m = [byte(1): 1]; 0; }"),
        ["Map key type byte is not a scalar key type"]
    );
}

/// Предупреждения анализа корректной программы.
fn warnings(text: &str) -> Vec<String> {
    let source =
        SourceFile::new("test.ok", format!("module app v1.0.0; {text}")).expect("small source");
    let program = prune::strip_test_items(
        syntax::parse(&source, &ParseOptions::default()).expect("valid syntax"),
    );
    let module = SourceModule {
        path: "app".to_string(),
        program: &program,
        imports: Vec::new(),
    };
    analysis::analyze(vec![module])
        .expect("valid program")
        .warnings
        .into_iter()
        .map(|warning| warning.message)
        .collect()
}

#[test]
fn a_for_loop_value_needs_a_body_that_runs() {
    // Ноль итераций известен статически — ошибка.
    for body in [
        "for 3..3 -> index { index; }",
        "for 5..=4 -> index { index; }",
        "let empty is @int = []; for [] -> index { index; }",
    ] {
        assert_eq!(
            messages(&format!("main() is int {{ {body} }}")),
            ["For loop has no value: its source is empty, so the body never runs"],
            "{body}"
        );
    }
    // Непустой источник известен — ни ошибки, ни предупреждения.
    for body in [
        "for 0..3 -> index { index; }",
        "for [4, 5] -> item { item; }",
    ] {
        assert!(
            messages(&format!("main() is int {{ {body} }}")).is_empty(),
            "{body}"
        );
        assert!(
            warnings(&format!("main() is int {{ {body} }}")).is_empty(),
            "{body}"
        );
    }
    // Число итераций неизвестно — предупреждение.
    assert_eq!(
        warnings(
            "main() is int { let xs = [1, 2]; let n = xs.size(); for 0..n -> index { index; } }"
        ),
        ["For loop value is undefined when its source is empty: the body may not run"]
    );
    // `for` как оператор значения не даёт: предупреждать не о чем.
    assert!(warnings("main() is int { let n = 2; for 0..n -> index { index; } 0; }").is_empty());
}

#[test]
fn map_keys_have_exactly_one_type() {
    // D-047: ключ `any` невозможен, поэтому разнородные ключи — ошибка.
    assert_eq!(
        messages("main() is int { let m = [1: \"int\", \"1\": \"string\"]; 0; }"),
        ["Map keys have different types: int and string"]
    );
    // Разнородные значения — `%K:any` (R08).
    assert!(
        messages(
            "main() is int { let m = [\"a\": 1, \"b\": \"two\"]; let n is %string:any = m; 0; }"
        )
        .is_empty()
    );
}

#[test]
fn a_for_loop_produces_the_value_of_its_last_iteration() {
    assert!(
        messages("main() is int { for 0..3 -> index { last index if index == 2; -1; } }")
            .is_empty()
    );
    assert_eq!(
        messages("main() is int { for 0..3 -> index { \"x\"; } }"),
        ["Action result must be int, got string"]
    );
}

#[test]
fn boundaries_are_explicit_coercions_in_thir() {
    let source = SourceFile::new(
        "test.ok",
        "module app v1.0.0; role Shape { area() is int; } \
         object Sq does Shape { has s is int; Sq(:s); area() is int { s * s; } } \
         main() is int { let shape is Shape = Sq(s: 2); let xs is @int = []; shape.area(); }",
    )
    .expect("small source");
    let program = syntax::parse(&source, &ParseOptions::default()).expect("valid syntax");
    let analysis = analysis::analyze(vec![SourceModule {
        path: "app".to_string(),
        program: &program,
        imports: Vec::new(),
    }])
    .expect("valid program");
    let main = analysis
        .bodies
        .iter()
        .find(|body| body.owner == analysis::thir::BodyOwner::Entry)
        .expect("entry body");
    let coercions: Vec<Coercion> = main
        .block
        .stmts
        .iter()
        .filter_map(|stmt| match &stmt.kind {
            StmtKind::Let { value, .. } => match &value.kind {
                ExprKind::Coerce { coercion, .. } => Some(coercion.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    // `[]` с целевым типом сразу типизирован: `@int`, без отдельного тега.
    assert_eq!(coercions, [Coercion::RoleWrap]);
    let empty = main
        .block
        .stmts
        .iter()
        .find_map(|stmt| match &stmt.kind {
            StmtKind::Let { value, .. } if matches!(value.kind, ExprKind::Array(_)) => {
                value.ty.clone()
            }
            _ => None,
        })
        .expect("the empty array literal");
    assert_eq!(
        empty,
        okkama_compiler::analysis::ty::Ty::array(okkama_compiler::analysis::ty::Ty::Int)
    );
}

const COUNTER: &str =
    "object C { has n is int = 0; C(); inc() is ^C { ^n = n + 1; } value() is int { n; } }";

#[test]
fn successor_obligations_are_checked_before_lowering() {
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
            "Successor action result must be used; binding \"d\" can leave scope without consumption",
        ),
        // Тело цикла может не выполниться: обязательство переживает цикл.
        (
            "let c = C(); let d = c.inc(); for 1..=2 -> i { d = d.inc(); } 0;",
            "Successor action result must be used; binding \"d\" can leave scope without consumption",
        ),
        (
            "let c = C(); let d = c.inc(); return 0;",
            "Successor action result must be used; binding \"d\" can leave scope without consumption",
        ),
        (
            "let c = C(); for 1..=2 -> i { let d = c.inc(); next; } 0;",
            "Successor action result must be used; binding \"d\" can leave scope without consumption",
        ),
        (
            "let c = C(); defer c.inc(); 0;",
            "Successor action result must be used",
        ),
    ] {
        assert_eq!(
            messages(&format!("{COUNTER} main() is int {{ {body} }}")),
            [expected],
            "{body}"
        );
    }
    // Использованные результаты: receiver, аргумент, возврат, rebind, агрегат.
    for body in [
        "let c = C(); let d = c.inc(); d.value();",
        "let c = C(); let d = c.inc(); let xs = [d]; 0;",
        "let c = C(); let d = c.inc(); let k = d.inc(); k.value();",
        "let c = C(); c = c.inc(); 0;",
        "let c = C(); for 1..=2 -> i { c = c.inc(); } c.value();",
        "let c = C(); let d = if 1 > 0 { c.inc(); } else { c.inc(); }; d.value();",
    ] {
        assert!(
            messages(&format!("{COUNTER} main() is int {{ {body} }}")).is_empty(),
            "{body}"
        );
    }
}

#[test]
fn constructor_fields_are_checked_before_lowering() {
    for (object, expected) in [
        (
            "object Q { has v is int; has w is int; Q(v is int) { v = v; } }",
            "Object \"Q\" field \"w\" was not initialized",
        ),
        (
            "object Q { has v is int; Q(x is int) { confirm self.v > 0; v = x; } }",
            "Object \"Q\" field \"v\" is read before it is bound",
        ),
        (
            "object Q { has v is int; Q(x is int) { for 1..=2 -> i { v = x; } } }",
            "Field \"v\" must be bound before a loop that rebinds it",
        ),
    ] {
        assert_eq!(
            messages(&format!("{object} main() is int {{ 0; }}")),
            [expected],
            "{object}"
        );
    }
    // Связанное не на всех путях — проверка во время исполнения, не ошибка.
    assert!(
        messages(
            "object Q { has v is int; Q(x is int) { if x > 0 { v = x; } } } main() is int { 0; }"
        )
        .is_empty()
    );
}

#[test]
fn values_holding_any_are_never_compared() {
    // `any` не распаковывается в программе: сравнение `any` и коллекций из
    // него отвергается до запуска (Perl падает всегда, во время исполнения).
    assert_eq!(
        messages(
            "main() is int { let a is any = 1; let xs is @any = [1, \"x\"]; \
             let m is %string:any = [\"k\": 1]; \
             if a == a || xs == xs || m == m { 1; } else { 0; } }"
        ),
        [
            "Cannot compare any and any",
            "Cannot compare @any and @any",
            "Cannot compare %string:any and %string:any",
        ]
    );
}

#[test]
fn bound_actions_are_called_only_after_an_action_type_boundary() {
    let object = "object C { C(); bump(n is int) is int { n; } }";
    assert_eq!(
        messages(&format!(
            "{object} main() is int {{ let c = C(); let b = c.bump; b(1); }}"
        )),
        ["Bound action requires an exact action-type boundary before it can be called"]
    );
    assert!(
        messages(&format!(
            "{object} main() is int {{ let c = C(); let b is (int) is int = c.bump; b(1); }}"
        ))
        .is_empty()
    );
}

#[test]
fn for_await_checks_async_next_and_maybe_payload() {
    let valid = "object Stream { Stream(); async next() is Maybe<int> { Maybe.none; } } ";
    assert!(messages(&format!("{valid} async main() is int {{ for await Stream() -> item {{ let n = item + 1; next; }} 0; }}")).is_empty());
    assert!(messages(&format!("{valid} main() is int {{ for await Stream() -> item {{}} 0; }}")).iter().any(|m| m.contains("only inside an async")));
    assert!(!messages("object Stream { Stream(); next() is Maybe<int> { Maybe.none; } } async main() is int { for await Stream() -> item {} 0; }").is_empty());
    assert!(!messages("object Stream { Stream(); async next() is int { 1; } } async main() is int { for await Stream() -> item {} 0; }").is_empty());
    assert!(!messages("object Stream { Stream(); async next() is Presence<int> { Presence.none; } } async main() is int { for await Stream() -> item {} 0; }").is_empty());
    assert!(messages(&format!("{valid} async main() is int {{ for await Stream() -> a, b {{}} 0; }}")).iter().any(|m| m.contains("exactly one binding")));
    assert!(messages(&format!("{valid} async main() is int {{ for await Stream() -> a[2] {{}} 0; }}")).iter().any(|m| m.contains("batched")));
    assert!(messages(&format!("{valid} async main() is int {{ for await Stream() -> item {{}} item; }}")).iter().any(|m| m.contains("Undefined variable")));
}

#[test]
fn calls_through_a_cap_name_the_cap() {
    // Issue #8: a cap value has computed fields read with `::`, no actions.
    assert_eq!(
        messages(
            "object Customer { has given, family is string; Customer(:given, :family); \
             greet() is void {} \
             cap Public { full_name is string { given + \" \" + family; } } } \
             main() is int { let owner = Customer(given: \"J\", family: \"D\"); let c = owner::Public; \
             c.greet(); c.full_name(); c.other(); 0; }"
        ),
        [
            "Action \"greet\" of \"Customer\" is not available through cap \"Customer::Public\": \
             a cap has no actions; read its computed fields with \"::\" (full_name)",
            "\"full_name\" is a computed field of cap \"Customer::Public\", not an action: \
             read it with \"::full_name\"",
            "Cap \"Customer::Public\" has no actions; read its computed fields with \"::\" (full_name)",
        ]
    );
}

/// A chain of `let`s that deepens a generic type stops at the type depth
/// limit with one error (#49): lowering would keep a full copy of every
/// value's type, memory growing with the square of the depth.
#[test]
fn types_deeper_than_the_limit_are_refused_once() {
    let lets: String = (0..100).map(|i| format!("    let b{} = b{i}.deeper();\n", i + 1)).collect();
    let text = format!(
        "object Box<T> {{\n    has value is T;\n    Box(:value);\n    deeper() is Box<@T> {{ Box(value: [value]); }}\n}}\nmain() is int {{\n    let b0 = Box(value: 1);\n{lets}    0;\n}}\n"
    );
    let found = messages(&text);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("is nested deeper than 64 levels"), "{found:?}");
}

/// Diagnostics that say what the rule is (#226, #229, #233).
#[test]
fn diagnostics_name_the_rule_and_the_fix() {
    let assign = messages("object Bottle {\n has opened is bool;\n Bottle(:opened);\n close() is void { opened = false; }\n}\nmain() is int { 0; }");
    assert!(assign.iter().any(|m| m.contains("Field \"opened\" cannot be assigned after construction") && m.contains("^opened")), "{assign:?}");
    let matched = messages("enum Color = (red, green, blue);\nmain() is int {\n let c = Color.red;\n let name = match c { case Color.red { \"r\"; } case Color.green { \"g\"; } };\n 0;\n}");
    assert!(matched.iter().any(|m| m.contains("does not cover blue")), "{matched:?}");
    let ordering = messages("main() is int {\n let sorted = [9, 0, 2, 1].ordering();\n 0;\n}");
    assert!(ordering.iter().any(|m| m.contains("import \"compare\" take Comparing")), "{ordering:?}");
}

/// Exponent literals are floats (#57); one that cannot be a finite float, or
/// that is not zero but would round to 0.0, is an error.
#[test]
fn exponent_literals_are_floats_in_range() {
    assert!(messages("main() is int { let a is float = 1e5; let b is float = 1.5E-3; let c is float = -2e+10; let d is float = 4.9e-324; let e is float = 0e9; let f is float = 0.0e-999; 0; }").is_empty());
    assert_eq!(
        messages("main() is int { let a is int = 1e5; let b = 1e400; let c = 1e-400; let d = -1e-400; let e = 0.000_1e-330; 0; }"),
        [
            "Cannot initialize variable \"a\" of type int with float",
            "Float value must contain a finite number",
            "Float literal is too small: it would round to 0.0",
            "Float literal is too small: it would round to 0.0",
            "Float literal is too small: it would round to 0.0",
        ]
    );
    assert_eq!(messages("main() is int { let a = 1e99999999999999999999999; 0; }"), ["Float value must contain a finite number"]);
}

/// Value objects are map keys (#212): every field a scalar (also `byte` and
/// `rat`), an enum or such an object; a resource, a collection field or a
/// generic argument that is not a key is named in the error.
#[test]
fn value_objects_are_map_keys() {
    let declarations = "enum Suit = (hearts, spades); \
        object Card { has rank is int; has suit is Suit; has weight is rat; has mark is byte; Card(rank is int, suit is Suit, weight is rat, mark is byte) { self.rank = rank; self.suit = suit; self.weight = weight; self.mark = mark; } } \
        object Hand { has best is Card; Hand(best is Card) { self.best = best; } } \
        object Box<T> { has value is T; Box(value is T) { self.value = value; } } \
        object Tagged { has tags is @string; Tagged(tags is @string) { self.tags = tags; } } \
        object Outer { has inner is Tagged; Outer(inner is Tagged) { self.inner = inner; } } \
        object File { has path is string; File(path is string) { self.path = path; } destroy { path; } } \
        object Early { has index is %Tagged:int; Early(index is %Tagged:int) { self.index = index; } }";
    assert_eq!(
        messages(&format!("{declarations} main() is int {{ let a is %Card:int = [:]; let b is %Hand:int = [:]; let c is %Box<string>:int = [Box(\"x\"): 1]; let d is %File:int = [:]; let e is %Box<@int>:int = [:]; let f is %Outer:int = [:]; 0; }}")),
        [
            "Map key type Tagged cannot be a key: field \"tags\" is @string",
            "Map key type Tagged cannot be a key: field \"tags\" is @string",
            "Map key type File cannot be a key: File has a destroy block",
            "Map key type Box<@int> cannot be a key: field \"value\" is @int",
            "Map key type Outer cannot be a key: field \"inner\" (Tagged): field \"tags\" is @string",
        ]
    );
    assert_eq!(messages("main() is int { let a is %@int:int = [:]; 0; }"), ["Map key type @int is not a scalar key type"]);
}

/// A generic parameter used as a map key takes only key types (#242): the
/// specialization is rejected at compile time, also through a parameter
/// passed on to another generic object and through a key used only in a body.
#[test]
fn generic_map_keys_take_key_types() {
    let declarations = "object Card { has rank is int; Card(rank is int) { self.rank = rank; } } \
        object File { has path is string; File(path is string) { self.path = path; } destroy { path; } } \
        object Index<T> { has entries is %T:int; Index() { let empty is %T:int = [:]; entries = empty; } size() is int { entries.size(); } } \
        object Wrap<U> { has inner is Index<U>; Wrap() { inner = Index(); } } \
        object Late<T> { has items is @T; Late(items is @T) { self.items = items; } distinct() is int { let seen is %T:bool = [:]; seen.size(); } } \
        object Plain<T> { has items is @T; Plain(items is @T) { self.items = items; } } \
        object Literal<T> { has item is T; Literal(item is T) { self.item = item; } count() is int { [item: 1].size(); } }";
    assert!(messages(&format!("{declarations} main() is int {{ let a is Index<string> = Index(); let b is Index<Card> = Index(); let c is Wrap<int> = Wrap(); let d = Plain(items: [[1]]); let e = Late(items: [1]); let f = Literal(item: \"x\"); a.size() + b.size(); }}")).is_empty());
    assert_eq!(
        messages(&format!("{declarations} main() is int {{ let a is Index<@int> = Index(); let b is Index<File> = Index(); let c is Wrap<@string> = Wrap(); let d = Late(items: [[1]]); let e = Literal(item: [1]); 0; }}")),
        [
            "Generic parameter \"T\" for object \"Index\" is a map key: Map key type @int is not a scalar key type",
            "Generic parameter \"T\" for object \"Index\" is a map key: Map key type File cannot be a key: File has a destroy block",
            "Generic parameter \"U\" for object \"Wrap\" is a map key: Map key type @string is not a scalar key type",
            "Generic parameter \"T\" for object \"Late\" is a map key: Map key type @int is not a scalar key type",
            "Generic parameter \"T\" for object \"Literal\" is a map key: Map key type @int is not a scalar key type",
        ]
    );
}

/// An anonymous action's declared result fixes its type and supplies generic
/// arguments (#214): `Keyed(origin:, key: (job is Job) is int { … })` infers
/// `Keyed<Job, int>` without an annotation at the binding.
#[test]
fn anonymous_action_results_supply_generic_arguments() {
    let declarations = "object Job { has weight is int; Job(weight is int) { self.weight = weight; } weight() is int { weight; } } \
        object Keyed<T, K> { has origin is @T; has key is (T) is K; Keyed(origin is @T, key is (T) is K) { self.origin = origin; self.key = key; } first() is K { key(origin[0]); } }";
    assert!(messages(&format!("{declarations} main() is int {{ let keyed = Keyed(origin: [Job(3)], key: (job is Job) is int {{ job.weight(); }}); let k is int = keyed.first(); k; }}")).is_empty());
    assert_eq!(
        messages(&format!("{declarations} main() is int {{ let keyed = Keyed(origin: [Job(3)], key: (job is Job) is int {{ job.weight(); }}); let k is string = keyed.first(); 0; }}")),
        ["Cannot initialize variable \"k\" of type string with int"]
    );
}

/// Default values of trailing parameters (#149): a call may leave them out,
/// positionally or by name; the value is checked in the declaring scope,
/// without the caller's locals, and reported once at the declaration.
#[test]
fn trailing_parameters_take_default_values() {
    let greeter = "object Greeter { has greeting is string; Greeter(greeting is string = \"hello\") { self.greeting = greeting; } \
        greet(name is string, mark is string = \"!\", times is int = 1) is string { greeting + name + mark + times.string(); } }";
    assert!(messages(&format!("{greeter} main() is int {{ let a = Greeter().greet(\"x\"); let b = Greeter(\"hi\").greet(\"y\", \"?\"); let c = Greeter().greet(name: \"z\", times: 2); 0; }}")).is_empty());
    assert_eq!(
        messages("object Bad { Bad(); first(a is int = 1, b is int) is int { a + b; } typed(a is int = \"one\") is int { a; } hidden(a is int = secret) is int { a; } } main() is int { let secret = 1; Bad().typed(); Bad().typed(); 0; }"),
        [
            "Parameter \"b\" follows a parameter with a default value, so it needs one too",
            "Default value must be int, got string",
            "Undefined variable \"secret\"",
        ]
    );
    assert_eq!(messages("role Named { name(prefix is string = \"x\") is string; } main() is int { 0; }"), ["Role actions cannot declare default values"]);
}

#[test]
fn break_and_continue_point_to_last_and_next() {
    let found = messages("main() is int { loop { break; } 0; }");
    assert!(found.iter().any(|m| m.ends_with("Okkama leaves a loop with `last`")), "{found:?}");
    let found = messages("main() is int { for [1] -> n { continue; } 0; }");
    assert!(found.iter().any(|m| m.contains("next iteration with `next`")), "{found:?}");
}
