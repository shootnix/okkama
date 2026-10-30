use std::collections::HashMap;

use super::*;
use crate::lowering::{self, LoadedModule, ModuleLoader};
use crate::syntax::{self, ParseOptions, prune};

/// Модули из памяти: путь импорта → текст.
struct Memory(HashMap<String, String>);

impl ModuleLoader for Memory {
    fn load(&mut self, path: &str, _importer: &str) -> Result<Option<LoadedModule>, String> {
        let Some(text) = self.0.get(path) else {
            return Ok(None);
        };
        let source = SourceFile::new(format!("/lib/{path}.ok"), text.clone()).unwrap();
        let program =
            syntax::parse(&source, &ParseOptions::default()).map_err(|_| "syntax".to_string())?;
        Ok(Some(LoadedModule {
            id: source.name().to_string(),
            program: prune::strip_test_items(program),
            source,
        }))
    }
}

const LIBRARY: &str = r#"module shop/money v1.0.0;

// Amount of money in cents.
object Money {
    has cents is int;
    Money(cents is int) { self.cents = cents; }

    // Sum of two amounts.
    plus(other is Money) is Money { Money(cents + other.cents()); }
    cents() is int { cents; }
}

enum Currency = (euro, dollar);
"#;

const MAIN: &str = r#"module app v1.0.0;
import "shop/money" take Money, Currency;

object Wallet {
    has total is Money;
    Wallet(total is Money) { self.total = total; }
    add(amount is Money) is Money {
        let sum = total.plus(amount);
        sum;
    }
}

main() is int {
    let wallet = Wallet(Money(5));
    let _kind = Currency.euro;
    return wallet.add(Money(7)).cents();
}
"#;

fn inspect(text: &str) -> Inspection {
    let source = SourceFile::new("/app/main.ok", text).unwrap();
    let mut loader = Memory(HashMap::from([(
        "shop/money".to_string(),
        LIBRARY.to_string(),
    )]));
    lowering::inspect(&source, &mut loader, false)
}

/// Символ на `occurrence`-м вхождении `needle` (смещение `shift` внутри).
fn at<'a>(
    inspection: &'a Inspection,
    text: &str,
    needle: &str,
    occurrence: usize,
    shift: u32,
) -> &'a Symbol {
    let start = text.match_indices(needle).nth(occurrence).unwrap().0 as u32;
    inspection
        .symbol_at(start + shift)
        .unwrap_or_else(|| panic!("no symbol at {needle:?}#{occurrence}"))
}

#[test]
fn line_index_counts_utf16_units() {
    let text = "ab\nжё🙂x\n";
    let lines = LineIndex::new(text);
    let x = text.find('x').unwrap() as u32;
    assert_eq!(
        lines.position(x),
        Position {
            line: 1,
            character: 4
        }
    );
    assert_eq!(
        lines.offset(Position {
            line: 1,
            character: 4
        }),
        x
    );
    assert_eq!(
        lines.offset(Position {
            line: 0,
            character: 99
        }),
        2
    );
    assert_eq!(
        lines.position(text.len() as u32),
        Position {
            line: 2,
            character: 0
        }
    );
}

#[test]
fn doc_comments_are_the_lines_right_above() {
    let source = SourceFile::new(
        "x.ok",
        "// one\n//   two\nobject A {}\n\n// far\n\nobject B {}\n",
    )
    .unwrap();
    assert_eq!(
        doc_comment(&source, source.text().find("object A").unwrap() as u32),
        "one\n  two"
    );
    assert_eq!(
        doc_comment(&source, source.text().find("object B").unwrap() as u32),
        ""
    );
}

#[test]
fn a_clean_program_has_no_diagnostics_and_an_outline() {
    let inspection = inspect(MAIN);
    assert!(inspection.parsed);
    assert!(
        inspection.diagnostics.is_empty(),
        "{:?}",
        inspection.diagnostics
    );
    let names: Vec<&str> = inspection
        .outline
        .iter()
        .map(|item| item.name.as_str())
        .collect();
    assert_eq!(names, ["Wallet", "main"]);
    let members: Vec<&str> = inspection.outline[0]
        .children
        .iter()
        .map(|item| item.name.as_str())
        .collect();
    assert_eq!(members, ["total", "Wallet", "add"]);
}

#[test]
fn locals_hover_with_their_type_and_lead_to_their_binding() {
    let inspection = inspect(MAIN);
    let symbol = at(&inspection, MAIN, "sum;", 0, 1);
    assert!(
        symbol.hover.contains("let sum is Money"),
        "{}",
        symbol.hover
    );
    let definition = symbol.definition.as_ref().unwrap();
    assert_eq!(definition.path, "/app/main.ok");
    assert_eq!(
        definition.range.start,
        Position {
            line: 7,
            character: 12
        }
    );
}

#[test]
fn methods_of_imported_objects_show_signature_and_doc() {
    let inspection = inspect(MAIN);
    let symbol = at(&inspection, MAIN, "plus", 0, 2);
    assert!(
        symbol.hover.contains("Money.plus(other is Money) is Money"),
        "{}",
        symbol.hover
    );
    assert!(
        symbol.hover.contains("Sum of two amounts."),
        "{}",
        symbol.hover
    );
    let definition = symbol.definition.as_ref().unwrap();
    assert_eq!(definition.path, "/lib/shop/money.ok");
    assert_eq!(
        definition.range.start,
        Position {
            line: 8,
            character: 4
        }
    );
}

#[test]
fn constructors_fields_enum_cases_and_types_resolve() {
    let inspection = inspect(MAIN);
    let ctor = at(&inspection, MAIN, "Money(5)", 0, 1);
    assert!(ctor.hover.contains("Money(cents is int)"), "{}", ctor.hover);
    assert_eq!(ctor.definition.as_ref().unwrap().range.start.line, 5);

    let field = at(&inspection, MAIN, "total.plus", 0, 1);
    assert!(
        field.hover.contains("has total is Money"),
        "{}",
        field.hover
    );
    assert_eq!(
        field.definition.as_ref().unwrap().range.start,
        Position {
            line: 4,
            character: 8
        }
    );

    let case = at(&inspection, MAIN, "euro", 0, 1);
    assert!(case.hover.contains("Currency.euro"), "{}", case.hover);
    assert_eq!(
        case.definition.as_ref().unwrap().range.start,
        Position {
            line: 12,
            character: 17
        }
    );

    let taken = at(&inspection, MAIN, "Money, Currency", 0, 1);
    assert!(taken.hover.contains("object Money"), "{}", taken.hover);
    assert!(
        taken.hover.contains("plus(other is Money) is Money"),
        "{}",
        taken.hover
    );
    assert!(
        taken.hover.contains("Amount of money in cents."),
        "{}",
        taken.hover
    );
    assert_eq!(
        taken.definition.as_ref().unwrap().path,
        "/lib/shop/money.ok"
    );

    let chained = at(&inspection, MAIN, "cents()", 0, 1);
    assert!(
        chained.hover.contains("Money.cents() is int"),
        "{}",
        chained.hover
    );
}

#[test]
fn errors_keep_the_index_and_point_at_their_span() {
    let text = MAIN.replace(
        "let _kind = Currency.euro;",
        "let _kind is string = Currency.euro;",
    );
    let inspection = inspect(&text);
    assert_eq!(
        inspection.diagnostics.len(),
        1,
        "{:?}",
        inspection.diagnostics
    );
    let error = &inspection.diagnostics[0];
    assert_eq!(error.severity, Severity::Error);
    assert_eq!(error.range.start.line, 14);
    assert!(
        at(&inspection, &text, "plus", 0, 1)
            .hover
            .contains("Money.plus")
    );
}

#[test]
fn syntax_errors_are_reported_without_symbols() {
    let inspection = inspect("module app v1.0.0;\nmain() is int { let = 1; }\n");
    assert!(!inspection.parsed);
    assert_eq!(inspection.diagnostics.len(), 1);
    assert_eq!(inspection.diagnostics[0].range.start.line, 1);
    assert!(inspection.symbols.is_empty());
}

#[test]
fn missing_imports_are_reported_on_the_import() {
    let inspection =
        inspect("module app v1.0.0;\nimport \"nowhere\" take X;\nmain() is int { return 0; }\n");
    assert_eq!(
        inspection.diagnostics.len(),
        1,
        "{:?}",
        inspection.diagnostics
    );
    assert_eq!(inspection.diagnostics[0].range.start.line, 1);
}

fn lints(text: &str) -> Vec<(String, u32)> {
    inspect(text)
        .diagnostics
        .iter()
        .filter_map(|found| {
            found
                .code
                .clone()
                .map(|code| (code, found.range.start.line))
        })
        .collect()
}

#[test]
fn lint_reports_unused_imports_lets_privates_and_unreachable_code() {
    let text = r#"module app v1.0.0;
import "shop/money" take Money, Currency;

object Box {
    has value is int;
    Box(value is int) { self.value = value; }
    get() is int {
        let spare = 1;
        let _kept = 2;
        return value;
        value;
    }
    private:
    helper() is int { 1; }
}

main() is int {
    return Box(1).get();
}
"#;
    let found = lints(text);
    assert_eq!(
        found,
        [
            ("unused-import".to_string(), 1),
            ("unused-import".to_string(), 1),
            ("unused-let".to_string(), 7),
            ("unreachable".to_string(), 10),
            ("unused-private".to_string(), 13),
        ],
        "{found:?}"
    );
    let diagnostics = inspect(text).diagnostics;
    assert!(
        diagnostics
            .iter()
            .filter(|found| found.code.is_some())
            .all(|found| found.unnecessary)
    );
}

#[test]
fn lint_accepts_used_names_closures_and_snake_case() {
    let text = r#"module app v1.0.0;
import "shop/money" take Money;

object Box {
    has total is Money;
    Box(total is Money) { self.total = total; }
    sum(amounts is @int) is int {
        let base = 1;
        let all is @int = amounts.map() -> amount { amount + base + self.helper(); };
        all.size();
    }
    private:
    helper() is int { 1; }
}

main() is int {
    let camelName = 1;
    return Box(Money(camelName)).sum([1]);
}
"#;
    assert_eq!(lints(text), [("snake-case".to_string(), 16)]);
}

/// Дополнения в позиции после `marker` в `edited` по проверке `MAIN`.
fn complete_after(edited: &str, marker: &str) -> Vec<String> {
    let inspection = inspect(MAIN);
    let offset = (edited.find(marker).unwrap() + marker.len()) as u32;
    let mut labels: Vec<String> = inspection
        .complete(MAIN, edited, offset)
        .into_iter()
        .map(|completion| completion.label)
        .collect();
    labels.sort();
    labels.dedup();
    labels
}

#[test]
fn members_of_locals_and_call_results_complete_while_the_text_does_not_parse() {
    let edited = MAIN.replace("        sum;\n", "        sum.\n");
    assert_eq!(complete_after(&edited, "        sum."), ["cents", "plus"]);
    let chained = MAIN.replace("total.plus(amount);", "total.plus(amount).");
    assert_eq!(
        complete_after(&chained, "total.plus(amount)."),
        ["cents", "plus"]
    );
}

#[test]
fn self_enum_strings_and_take_lists_complete() {
    let with_self = MAIN.replace("        sum;\n", "        self.\n");
    assert_eq!(complete_after(&with_self, "self."), ["add", "total"]);
    let with_enum = MAIN.replace("Currency.euro;", "Currency.;");
    assert_eq!(complete_after(&with_enum, "Currency."), ["dollar", "euro"]);
    let with_string = MAIN.replace("        sum;\n", "        \"x\".\n");
    let labels = complete_after(&with_string, "\"x\".");
    assert!(
        labels.contains(&"split".to_string()) && labels.contains(&"length".to_string()),
        "{labels:?}"
    );
    let take = MAIN.replace("take Money, Currency;", "take ");
    assert_eq!(complete_after(&take, "take "), ["Currency", "Money"]);
}

#[test]
fn plain_names_are_locals_fields_types_and_keywords_in_scope() {
    let edited = MAIN.replace("        sum;\n", "        s\n");
    let labels = complete_after(&edited, "        s");
    for expected in ["amount", "sum", "total", "self", "Money", "Wallet", "let"] {
        assert!(
            labels.contains(&expected.to_string()),
            "{expected} missing in {labels:?}"
        );
    }
    assert!(
        !labels.contains(&"wallet".to_string()),
        "main's local leaked: {labels:?}"
    );
}

#[test]
fn signature_help_names_the_active_parameter() {
    let inspection = inspect(MAIN);
    let edited = MAIN.replace("Wallet(Money(5))", "Wallet(Money(5), ");
    let offset = (edited.find("Wallet(Money(5), ").unwrap() + "Wallet(Money(5), ".len()) as u32;
    let (label, params, active) = inspection.signature(MAIN, &edited, offset).unwrap();
    assert_eq!(label, "Wallet(total is Money)");
    assert_eq!(params, ["total is Money"]);
    assert_eq!(active, 1);
}

#[test]
fn freshly_typed_receivers_resolve_by_name_and_through_calls() {
    let typed = MAIN.replace(
        "        sum;\n",
        "        let fresh = amount;\n        amount.\n",
    );
    assert_eq!(complete_after(&typed, "        amount."), ["cents", "plus"]);
    let chain = MAIN.replace("        sum;\n", "        Money(1).plus(amount).\n");
    assert_eq!(complete_after(&chain, "plus(amount)."), ["cents", "plus"]);
    let field = MAIN.replace("        sum;\n", "        total.\n");
    assert_eq!(complete_after(&field, "        total."), ["cents", "plus"]);
}

#[test]
fn std_paths_are_not_unused_imports() {
    // Загрузчик из памяти ищет `std.shop.money` там же, где обычный импорт.
    let text = r#"module app v1.0.0;

main() is int {
    let wallet = std.shop.money.Money(7);
    let kind is std.shop.money.Currency = std.shop.money.Currency.euro;
    return wallet.cents();
}
"#;
    let found = lints(text);
    assert!(!found.iter().any(|(code, _)| code == "unused-import"), "{found:?}");
    let errors: Vec<_> = inspect(text).diagnostics.into_iter().filter(|found| found.code.is_none()).collect();
    assert!(errors.is_empty(), "{errors:?}");
}
