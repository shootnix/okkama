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
