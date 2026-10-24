//! AST printer tests: node-kind coverage, snapshots of the `tree` and
//! `json` formats, JSON well-formedness and determinism.
//!
//! Snapshots live in `tests/snapshots/`. After an intentional printer
//! change, regenerate them with
//! `OKKAMA_UPDATE_SNAPSHOTS=1 cargo test -p okkama-compiler --test printer`
//! and review the diff.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use okkama_compiler::source::SourceFile;
use okkama_compiler::syntax::print::{
    Format, Node, PrintOptions, Value, print_program, program_node,
};
use okkama_compiler::syntax::{ParseMode, ParseOptions, parse};

const ALL_SYNTAX: &str = include_str!("fixtures/all-syntax.ok");

/// Every node kind the printers can emit.
const ALL_KINDS: &[&str] = &[
    "ActionDecl",
    "ActionSignature",
    "ActionType",
    "AnonymousAction",
    "Argument",
    "ArrayCast",
    "ArrayLiteral",
    "ArrayType",
    "Await",
    "Binary",
    "Block",
    "BlockStatement",
    "BoolLiteral",
    "Call",
    "CapDecl",
    "CatchClause",
    "CharLiteral",
    "ComputedField",
    "ConfirmPanic",
    "ConfirmStatement",
    "ConfirmThrow",
    "DeferStatement",
    "DestroyDecl",
    "EnumDecl",
    "ExpressionStatement",
    "FakeBody",
    "FakeDecl",
    "FieldBindStatement",
    "FloatLiteral",
    "ForBinding",
    "ForStatement",
    "FullConstructor",
    "Functional",
    "GenericParam",
    "Get",
    "HasDecl",
    "Grouping",
    "IfExpression",
    "IfStatement",
    "ImportDecl",
    "Index",
    "IntLiteral",
    "LastStatement",
    "LetStatement",
    "LoopExpression",
    "MapEntry",
    "MapLiteral",
    "MapType",
    "MatchCase",
    "MatchExpression",
    "MatchStatement",
    "ModuleDecl",
    "NameExpression",
    "NamedType",
    "NativeActionDecl",
    "NativeObjectDecl",
    "NestedType",
    "NextStatement",
    "ObjectDecl",
    "OwnerField",
    "Panic",
    "Param",
    "PrivateSection",
    "Program",
    "Projection",
    "ProjectionDirective",
    "Range",
    "ReturnStatement",
    "RoleDecl",
    "Select",
    "SelfExpression",
    "SelfFieldBindStatement",
    "ShorthandConstructor",
    "SkippedBody",
    "Spawn",
    "StreamWrite",
    "StringLiteral",
    "SuccessorAssignStatement",
    "SuccessorResult",
    "TestDecl",
    "ThrowStatement",
    "TryExpression",
    "TryStatement",
    "Unary",
    "Unpack",
    "VoidResult",
];

fn parse_source(source: &SourceFile, mode: ParseMode) -> okkama_compiler::syntax::ast::Program {
    let options = ParseOptions {
        mode,
        ..ParseOptions::default()
    };
    parse(source, &options).unwrap_or_else(|errors| panic!("{}: {errors:?}", source.name()))
}

fn collect_kinds(node: &Node, kinds: &mut BTreeSet<&'static str>) {
    kinds.insert(node.kind);
    for (_, value) in &node.fields {
        collect_value(value, kinds);
    }
}

fn collect_value(value: &Value, kinds: &mut BTreeSet<&'static str>) {
    match value {
        Value::Node(node) => collect_kinds(node, kinds),
        Value::List(items) => items.iter().for_each(|item| collect_value(item, kinds)),
        _ => {}
    }
}

#[test]
fn the_fixture_exercises_every_node_kind() {
    let source = SourceFile::new("all-syntax.ok", ALL_SYNTAX).unwrap();
    let mut kinds = BTreeSet::new();
    for mode in [ParseMode::Full, ParseMode::Production] {
        collect_kinds(&program_node(&parse_source(&source, mode)), &mut kinds);
    }
    let expected: BTreeSet<&str> = ALL_KINDS.iter().copied().collect();
    let missing: Vec<_> = expected.difference(&kinds).collect();
    let unexpected: Vec<_> = kinds.difference(&expected).collect();
    assert!(
        missing.is_empty(),
        "kinds not exercised by the fixture: {missing:?}"
    );
    assert!(
        unexpected.is_empty(),
        "kinds missing from ALL_KINDS: {unexpected:?}"
    );
}

#[test]
fn every_kind_appears_in_both_formats() {
    let source = SourceFile::new("all-syntax.ok", ALL_SYNTAX).unwrap();
    let mut tree = String::new();
    let mut json = String::new();
    for mode in [ParseMode::Full, ParseMode::Production] {
        let program = parse_source(&source, mode);
        for spans in [false, true] {
            let options = PrintOptions { spans };
            tree.push_str(&print_program(&program, &source, Format::Tree, options));
            json.push_str(&print_program(&program, &source, Format::Json, options));
        }
    }
    for kind in ALL_KINDS {
        assert!(
            tree.lines().any(|line| {
                let line = line.trim_start().trim_start_matches("- ");
                line == *kind
                    || line.starts_with(&format!("{kind} "))
                    || line.ends_with(&format!(": {kind}"))
                    || line.contains(&format!(": {kind} @"))
            }),
            "tree output never shows {kind}"
        );
        assert!(
            json.contains(&format!("\"node\": \"{kind}\"")),
            "json output never shows {kind}"
        );
    }
}

fn snapshot_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

fn check_snapshot(name: &str, actual: &str) {
    let path = snapshot_dir().join(name);
    if std::env::var_os("OKKAMA_UPDATE_SNAPSHOTS").is_some() {
        std::fs::create_dir_all(snapshot_dir()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing snapshot {}; run with OKKAMA_UPDATE_SNAPSHOTS=1 to create it",
            path.display()
        )
    });
    if expected != actual {
        let line = expected
            .lines()
            .zip(actual.lines())
            .position(|(a, b)| a != b)
            .map_or(expected.lines().count().min(actual.lines().count()), |n| n);
        panic!(
            "snapshot {name} differs at line {}:\n  expected: {:?}\n  actual:   {:?}\n\
             If the change is intended, run with OKKAMA_UPDATE_SNAPSHOTS=1 and review the diff.",
            line + 1,
            expected.lines().nth(line),
            actual.lines().nth(line)
        );
    }
}

fn snapshot_inputs() -> Vec<(String, String)> {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut inputs: Vec<(String, String)> = std::fs::read_dir(&examples)
        .expect("okkama-rs/examples")
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "ok"))
        .map(|path| {
            (
                path.file_stem().unwrap().to_string_lossy().into_owned(),
                std::fs::read_to_string(&path).unwrap(),
            )
        })
        .collect();
    inputs.sort();
    inputs.push(("all-syntax".to_string(), ALL_SYNTAX.to_string()));
    inputs
}

#[test]
fn tree_and_json_snapshots() {
    for (name, text) in snapshot_inputs() {
        let source = SourceFile::new(format!("{name}.ok"), text).unwrap();
        let program = parse_source(&source, ParseMode::Full);
        let options = PrintOptions::default();
        check_snapshot(
            &format!("{name}.tree"),
            &print_program(&program, &source, Format::Tree, options),
        );
        check_snapshot(
            &format!("{name}.json"),
            &print_program(&program, &source, Format::Json, options),
        );
    }
}

#[test]
fn span_snapshots() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello.ok");
    let source = SourceFile::new("hello.ok", std::fs::read_to_string(examples).unwrap()).unwrap();
    let program = parse_source(&source, ParseMode::Full);
    let options = PrintOptions { spans: true };
    check_snapshot(
        "hello.spans.tree",
        &print_program(&program, &source, Format::Tree, options),
    );
    check_snapshot(
        "hello.spans.json",
        &print_program(&program, &source, Format::Json, options),
    );
}

#[test]
fn production_snapshot_shows_skipped_bodies() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/tests-and-fakes.ok");
    let source =
        SourceFile::new("tests-and-fakes.ok", std::fs::read_to_string(path).unwrap()).unwrap();
    let program = parse_source(&source, ParseMode::Production);
    check_snapshot(
        "tests-and-fakes.production.tree",
        &print_program(
            &program,
            &source,
            Format::Tree,
            PrintOptions { spans: true },
        ),
    );
}

#[test]
fn printing_is_deterministic_and_json_is_well_formed() {
    for (name, text) in snapshot_inputs() {
        let source = SourceFile::new(format!("{name}.ok"), text).unwrap();
        for mode in [ParseMode::Full, ParseMode::Production] {
            for spans in [false, true] {
                let options = PrintOptions { spans };
                let first =
                    print_program(&parse_source(&source, mode), &source, Format::Json, options);
                let second =
                    print_program(&parse_source(&source, mode), &source, Format::Json, options);
                assert_eq!(first, second, "{name}");
                json_check::validate(&first).unwrap_or_else(|error| panic!("{name}: {error}"));
                let tree_a =
                    print_program(&parse_source(&source, mode), &source, Format::Tree, options);
                let tree_b =
                    print_program(&parse_source(&source, mode), &source, Format::Tree, options);
                assert_eq!(tree_a, tree_b, "{name}");
            }
        }
    }
}

#[test]
fn spans_are_byte_ranges_with_line_columns() {
    let source =
        SourceFile::new("t.ok", "module app v1.0.0;\nmain() is int {\n  \"é\";\n}\n").unwrap();
    let program = parse_source(&source, ParseMode::Full);
    // The literal "é" occupies bytes 37..41 (é is two bytes in UTF-8) and
    // columns 3..6 of line 3 (columns count characters; the end is
    // exclusive).
    let tree = print_program(
        &program,
        &source,
        Format::Tree,
        PrintOptions { spans: true },
    );
    assert!(
        tree.contains("expression: StringLiteral @3:3-3:6"),
        "{tree}"
    );
    let json = print_program(
        &program,
        &source,
        Format::Json,
        PrintOptions { spans: true },
    );
    let literal = &json[json.find("\"node\": \"StringLiteral\"").unwrap()..];
    let span: String = literal
        .lines()
        .skip(2)
        .take(6)
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(
        span,
        "\"start\": 37, \"end\": 41, \"start_line\": 3, \"start_column\": 3, \"end_line\": 3, \"end_column\": 6"
    );
}

/// A minimal JSON validator (the crate has no dependencies).
mod json_check {
    pub fn validate(text: &str) -> Result<(), String> {
        let bytes = text.as_bytes();
        let mut pos = 0;
        value(bytes, &mut pos)?;
        skip_ws(bytes, &mut pos);
        if pos != bytes.len() {
            return Err(format!("trailing data at byte {pos}"));
        }
        Ok(())
    }

    fn skip_ws(b: &[u8], pos: &mut usize) {
        while *pos < b.len() && matches!(b[*pos], b' ' | b'\n' | b'\r' | b'\t') {
            *pos += 1;
        }
    }

    fn expect(b: &[u8], pos: &mut usize, byte: u8) -> Result<(), String> {
        skip_ws(b, pos);
        if b.get(*pos) == Some(&byte) {
            *pos += 1;
            Ok(())
        } else {
            Err(format!("expected {:?} at byte {}", byte as char, *pos))
        }
    }

    fn value(b: &[u8], pos: &mut usize) -> Result<(), String> {
        skip_ws(b, pos);
        match b.get(*pos) {
            Some(b'{') => {
                *pos += 1;
                skip_ws(b, pos);
                if b.get(*pos) == Some(&b'}') {
                    *pos += 1;
                    return Ok(());
                }
                loop {
                    skip_ws(b, pos);
                    string(b, pos)?;
                    expect(b, pos, b':')?;
                    value(b, pos)?;
                    skip_ws(b, pos);
                    match b.get(*pos) {
                        Some(b',') => *pos += 1,
                        Some(b'}') => {
                            *pos += 1;
                            return Ok(());
                        }
                        _ => return Err(format!("bad object at byte {}", *pos)),
                    }
                }
            }
            Some(b'[') => {
                *pos += 1;
                skip_ws(b, pos);
                if b.get(*pos) == Some(&b']') {
                    *pos += 1;
                    return Ok(());
                }
                loop {
                    value(b, pos)?;
                    skip_ws(b, pos);
                    match b.get(*pos) {
                        Some(b',') => *pos += 1,
                        Some(b']') => {
                            *pos += 1;
                            return Ok(());
                        }
                        _ => return Err(format!("bad array at byte {}", *pos)),
                    }
                }
            }
            Some(b'"') => string(b, pos),
            Some(b't') => literal(b, pos, b"true"),
            Some(b'f') => literal(b, pos, b"false"),
            Some(b'n') => literal(b, pos, b"null"),
            Some(c) if c.is_ascii_digit() || *c == b'-' => {
                *pos += 1;
                while *pos < b.len() && (b[*pos].is_ascii_digit() || b".eE+-".contains(&b[*pos])) {
                    *pos += 1;
                }
                Ok(())
            }
            _ => Err(format!("unexpected byte at {}", *pos)),
        }
    }

    fn literal(b: &[u8], pos: &mut usize, word: &[u8]) -> Result<(), String> {
        if b[*pos..].starts_with(word) {
            *pos += word.len();
            Ok(())
        } else {
            Err(format!("bad literal at byte {}", *pos))
        }
    }

    fn string(b: &[u8], pos: &mut usize) -> Result<(), String> {
        if b.get(*pos) != Some(&b'"') {
            return Err(format!("expected string at byte {}", *pos));
        }
        *pos += 1;
        while *pos < b.len() {
            match b[*pos] {
                b'"' => {
                    *pos += 1;
                    return Ok(());
                }
                b'\\' => match b.get(*pos + 1) {
                    Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => *pos += 2,
                    Some(b'u')
                        if b.len() >= *pos + 6
                            && b[*pos + 2..*pos + 6].iter().all(u8::is_ascii_hexdigit) =>
                    {
                        *pos += 6
                    }
                    _ => return Err(format!("bad escape at byte {}", *pos)),
                },
                c if c < 0x20 => return Err(format!("raw control character at byte {}", *pos)),
                _ => *pos += 1,
            }
        }
        Err("unterminated string".to_string())
    }
}
