//! Corpus checks against the repository the crate lives in:
//!
//! * every `.ok` file in `std/`, `examples/` and `okkama-rs/examples/`
//!   must parse in both modes (none of them is an intentionally invalid
//!   fixture; `tests/corpus/expected-failures.txt` would list any, with a
//!   reason);
//! * every conformance case (`conformance/cases/**/case.json`) must be
//!   accepted or rejected by the parser as its expectation requires.
//!
//! These tests need the surrounding Okkama checkout. They fail loudly when
//! it is missing; set `OKKAMA_SKIP_CORPUS=1` to skip them on purpose (for
//! example when building the crate outside the repository).

mod common;

use std::path::{Path, PathBuf};

use common::json::{self, Json};
use okkama_compiler::source::SourceFile;
use okkama_compiler::syntax::{ParseMode, ParseOptions, parse, prune};

const EXPECTED_FAILURES: &str = include_str!("corpus/expected-failures.txt");

fn repo_root() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let root = root.canonicalize().ok()?;
    if root.join("std").is_dir() && root.join("examples").is_dir() {
        return Some(root);
    }
    None
}

fn require_repo() -> Option<PathBuf> {
    if std::env::var_os("OKKAMA_SKIP_CORPUS").is_some() {
        eprintln!("corpus checks skipped: OKKAMA_SKIP_CORPUS is set");
        return None;
    }
    Some(repo_root().expect(
        "the Okkama checkout (std/ and examples/) was not found next to okkama-rs; \
         set OKKAMA_SKIP_CORPUS=1 to skip the corpus checks explicitly",
    ))
}

fn ok_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            ok_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "ok") {
            out.push(path);
        }
    }
}

fn parses(path: &Path, mode: ParseMode) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let source = SourceFile::new(path.display().to_string(), text).map_err(|e| e.to_string())?;
    let options = ParseOptions {
        mode,
        ..ParseOptions::default()
    };
    match parse(&source, &options) {
        Ok(program) => {
            if mode == ParseMode::Production {
                let _ = prune::strip_test_items(program);
            }
            Ok(())
        }
        Err(diagnostics) => Err(diagnostics[0].headline(&source)),
    }
}

#[test]
fn std_and_examples_parse_in_both_modes() {
    let Some(root) = require_repo() else { return };
    let expected: Vec<&str> = EXPECTED_FAILURES
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|line| line.split('\t').next().unwrap())
        .collect();
    let mut files = Vec::new();
    for dir in ["std", "examples", "okkama-rs/examples"] {
        ok_files(&root.join(dir), &mut files);
    }
    assert!(
        files.len() > 900,
        "suspiciously small corpus: {}",
        files.len()
    );
    let mut unexpected = Vec::new();
    let mut failed_as_expected = Vec::new();
    for path in &files {
        let relative = path.strip_prefix(&root).unwrap().display().to_string();
        for mode in [ParseMode::Full, ParseMode::Production] {
            match parses(path, mode) {
                Ok(()) if expected.contains(&relative.as_str()) => unexpected.push(format!(
                    "{relative} ({mode:?}): expected to fail but parsed"
                )),
                Ok(()) => {}
                Err(_) if expected.contains(&relative.as_str()) => {
                    failed_as_expected.push(relative.clone())
                }
                Err(error) => unexpected.push(format!("{relative} ({mode:?}): {error}")),
            }
        }
    }
    eprintln!(
        "corpus: {} files parsed in full and production mode; {} expected failures",
        files.len(),
        failed_as_expected.len() / 2
    );
    assert!(unexpected.is_empty(), "{}", unexpected.join("\n"));
}

/// What a conformance case requires from the parser.
#[derive(Debug, PartialEq)]
enum Requirement {
    /// Every source must parse.
    Parse,
    /// At least one source must be rejected by the lexer or parser.
    Reject,
    /// The case allows the rejection at parse or at a later stage.
    Either,
}

fn requirement(case: &Json) -> Requirement {
    let expect = case.get("expect").expect("case.expect");
    let outcome = expect.get("outcome").and_then(Json::as_str).unwrap_or("");
    let stages_of = |key: &str| -> Vec<String> {
        expect
            .get(key)
            .and_then(|value| value.get("stages"))
            .map(|stages| {
                stages
                    .as_array()
                    .iter()
                    .filter_map(|s| s.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    let stages = match outcome {
        "reject" => stages_of("rejection"),
        // Five L04.projection.* cases declare `outcome: error` with a
        // `rejection` block instead of `failure` (a metadata slip in the
        // conformance suite, see docs/compatibility.md); read either.
        "error" => {
            let failure = stages_of("failure");
            if failure.is_empty() {
                stages_of("rejection")
            } else {
                failure
            }
        }
        _ => Vec::new(),
    };
    if !stages.iter().any(|stage| stage == "parse") {
        Requirement::Parse
    } else if stages.len() == 1 {
        Requirement::Reject
    } else {
        Requirement::Either
    }
}

#[test]
fn conformance_sources_meet_their_parse_expectations() {
    let Some(root) = require_repo() else { return };
    let cases_root = root.join("conformance/cases");
    let mut files = Vec::new();
    ok_files(&cases_root, &mut files);
    let mut case_dirs: Vec<PathBuf> = files
        .iter()
        .filter_map(|file| file.ancestors().find(|dir| dir.join("case.json").is_file()))
        .map(Path::to_path_buf)
        .collect();
    case_dirs.sort();
    case_dirs.dedup();
    assert!(
        case_dirs.len() > 700,
        "suspiciously few cases: {}",
        case_dirs.len()
    );
    let mut mismatches = Vec::new();
    let mut counts = [0usize; 3];
    for dir in &case_dirs {
        let case = json::parse(&std::fs::read_to_string(dir.join("case.json")).unwrap())
            .unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
        let id = case
            .get("id")
            .and_then(Json::as_str)
            .unwrap_or("?")
            .to_string();
        let mode = match case.get("mode").and_then(Json::as_str) {
            Some("test") => ParseMode::Full,
            _ => ParseMode::Production,
        };
        let results: Vec<Result<(), String>> = case
            .get("sources")
            .map(Json::as_array)
            .unwrap_or_default()
            .iter()
            .filter_map(Json::as_str)
            .map(|source| parses(&dir.join(source), mode))
            .collect();
        let all_parse = results.iter().all(Result::is_ok);
        let requirement = requirement(&case);
        match requirement {
            Requirement::Parse => {
                counts[0] += 1;
                if !all_parse {
                    let error = results.iter().find_map(|r| r.clone().err()).unwrap();
                    mismatches.push(format!("{id}: must parse, got {error}"));
                }
            }
            Requirement::Reject => {
                counts[1] += 1;
                if all_parse {
                    mismatches.push(format!("{id}: must be rejected by the lexer/parser"));
                }
            }
            Requirement::Either => counts[2] += 1,
        }
    }
    eprintln!(
        "conformance: {} cases must parse, {} must be rejected while parsing, {} allow either",
        counts[0], counts[1], counts[2]
    );
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
