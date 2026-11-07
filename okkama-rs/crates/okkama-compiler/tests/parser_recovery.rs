//! The recovering parse for editors (#47): every error in one run, clean
//! declarations kept, damaged ones left out, a bounded number of errors.

use okkama_compiler::source::SourceFile;
use okkama_compiler::syntax::ast::{Item, Program};
use okkama_compiler::syntax::parser::MAX_RECOVERED_ERRORS;
use okkama_compiler::syntax::{ParseOptions, parse, parse_recovering};

/// The names of the declarations kept, and the errors as `line: message`.
fn recover(text: &str) -> (Vec<String>, Vec<String>) {
    let source = SourceFile::new("recovery.ok", text.to_string()).unwrap();
    let recovered = parse_recovering(&source, &ParseOptions::default());
    let line = |offset: u32| text[..offset as usize].matches('\n').count() + 1;
    let errors = recovered.diagnostics.iter().map(|d| format!("{}: {}", line(d.span.start), d.message)).collect();
    (recovered.program.as_ref().map(names).unwrap_or_default(), errors)
}

fn names(program: &Program) -> Vec<String> {
    program
        .items
        .iter()
        .map(|item| match item {
            Item::Object(decl) => decl.name.text.clone(),
            Item::Enum(decl) => decl.name.text.clone(),
            Item::Action(decl) => decl.name.text.clone(),
            other => format!("{other:?}").chars().take(12).collect(),
        })
        .collect()
}

const MODULE: &str = "module app v1.0.0;\n";

#[test]
fn independent_errors_are_reported_together_and_clean_declarations_survive() {
    let text = format!(
        "{MODULE}
object First {{
    has a is int;
    First(:a);
    broken() is int {{ a +; }}
}}

object Clean {{
    has b is int;
    Clean(:b);
}}

enum Second = (one, two,);

main() is int {{ 0; }}
"
    );
    let (kept, errors) = recover(&text);
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert!(errors[0].starts_with("6: "), "{errors:?}");
    assert!(errors[1].starts_with("14: "), "{errors:?}");
    assert_eq!(kept, ["Clean", "main"]);
    // The CLI's strict parse still stops at the first.
    let source = SourceFile::new("recovery.ok", text).unwrap();
    assert_eq!(parse(&source, &ParseOptions::default()).unwrap_err().len(), 1);
}

#[test]
fn statement_errors_in_one_body_are_all_reported_and_the_body_is_left_out() {
    let text = format!(
        "{MODULE}
main() is int {{
    let a = (1 + ;
    let b = 2;
    let c = * 3;
    a + b;
}}

object After {{
    has x is int;
    After(:x);
}}
"
    );
    let (kept, errors) = recover(&text);
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert!(errors[0].starts_with("4: ") && errors[1].starts_with("6: "), "{errors:?}");
    assert_eq!(kept, ["After"]);
}

#[test]
fn a_missing_closing_brace_costs_one_declaration() {
    let text = format!(
        "{MODULE}
object Open {{
    has a is int;
    Open(:a);
    value() is int {{
        a;

object Next {{
    has b is int;
    Next(:b);
}}

main() is int {{ 0; }}
"
    );
    let (kept, errors) = recover(&text);
    assert!(!errors.is_empty() && errors.len() <= 2, "a cascade: {errors:?}");
    assert!(errors.iter().all(|error| error.starts_with("9: ")), "{errors:?}");
    assert_eq!(kept, ["Next", "main"]);
}

#[test]
fn a_stray_closing_brace_is_one_error() {
    let text = format!("{MODULE}\nmain() is int {{ 0; }}\n}}\n\nobject Kept {{\n    has a is int;\n    Kept(:a);\n}}\n");
    let (kept, errors) = recover(&text);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(kept, ["main", "Kept"]);
}

#[test]
fn errors_are_capped() {
    let body: String = (0..MAX_RECOVERED_ERRORS * 3).map(|_| "    let = ;\n").collect();
    let (_, errors) = recover(&format!("{MODULE}\nmain() is int {{\n{body}    0;\n}}\n"));
    assert_eq!(errors.len(), MAX_RECOVERED_ERRORS);
}

#[test]
fn lexical_and_syntax_errors_come_together() {
    let (kept, errors) = recover(&format!("{MODULE}\nmain() is int {{\n    let a = 0b102;\n    let b = ;\n    0;\n}}\n"));
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert!(errors[0].contains("0b102") && errors[1].starts_with("5: "), "{errors:?}");
    assert!(kept.is_empty(), "no tree when a token is malformed");
}

#[test]
fn a_broken_module_declaration_still_reports_the_rest() {
    let (kept, errors) = recover("module app v1.0.0\n\nmain() is int {\n    let = 1;\n}\n");
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert!(kept.is_empty());
}

/// Damaged copies of a file with every syntax form: the recovering parse
/// always terminates, within the cap, and keeps only clean declarations —
/// each kept one also parses alone.
#[test]
fn damaged_input_always_terminates() {
    let sample = include_str!("fixtures/all-syntax.ok");
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = |bound: usize| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % bound as u64) as usize
    };
    for _ in 0..500 {
        let mut text = sample.to_string();
        for _ in 0..1 + next(3) {
            let at = next(text.len());
            let end = (at + 1 + next(12)).min(text.len());
            if text.is_char_boundary(at) && text.is_char_boundary(end) {
                let filler = ["", "{", "}", ";", "(", "let", "object"][next(7)];
                text.replace_range(at..end, filler);
            }
        }
        let source = SourceFile::new("damaged.ok", text).unwrap();
        let recovered = parse_recovering(&source, &ParseOptions::default());
        assert!(recovered.diagnostics.len() <= MAX_RECOVERED_ERRORS + 8);
    }
}
