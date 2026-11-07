//! The Book's table of builtin actions (chapter 10) names every builtin the
//! compiler knows (#224): the names come from the source of
//! `analysis/check/builtin.rs`, so a new builtin fails here until the Book
//! lists it.

use std::collections::BTreeSet;
use std::path::Path;

/// Quoted lower-case names inside `fn builtin_signature` and `BIT_ACTIONS`.
fn compiler_names() -> BTreeSet<String> {
    let source = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/analysis/check/builtin.rs")).unwrap();
    let start = source.find("fn builtin_signature(").expect("builtin table");
    let end = start + source[start..].find("\n    }\n").expect("end of the table");
    let bits = source.find("const BIT_ACTIONS").expect("bit actions");
    let bits = &source[bits..bits + source[bits..].find("];").unwrap()];
    let mut names = BTreeSet::new();
    for text in [&source[start..end], bits] {
        for part in text.split('"').skip(1).step_by(2) {
            if !part.is_empty() && part.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
                names.insert(part.to_string());
            }
        }
    }
    names
}

fn chapter(book: &str, heading: &str) -> String {
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs").join(book)).unwrap();
    let start = text.find(heading).unwrap_or_else(|| panic!("{heading} in {book}"));
    let end = start + text[start..].find("\n---\n").unwrap_or(text.len() - start);
    text[start..end].to_string()
}

#[test]
fn chapter_ten_lists_every_builtin() {
    let names = compiler_names();
    assert!(names.contains("ordering") && names.contains("swap_bytes") && names.len() > 30, "{names:?}");
    for (book, heading) in [("OkkamaBook.md", "## Chapter 10."), ("OkkamaBook-RU.md", "## Глава 10.")] {
        let text = chapter(book, heading);
        let missing: Vec<&String> = names.iter().filter(|name| !text.contains(&format!("`{name}")) && !text.contains(&format!(".{name}("))).collect();
        assert!(missing.is_empty(), "{book} chapter 10 does not list {missing:?}");
    }
}
