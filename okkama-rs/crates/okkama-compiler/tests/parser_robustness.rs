//! Robustness: the front end must terminate with a diagnostic (never panic,
//! hang or overflow the stack) on arbitrary input, and must reject valid
//! prefixes followed by garbage.

mod common;

use common::parse_in;
use okkama_compiler::source::SourceFile;
use okkama_compiler::syntax::{ParseMode, ParseOptions, lexer, parse};
use okkama_compiler::syntax::ast::{BinaryOp, Expr, ExprKind, Name};
use okkama_compiler::source::Span;

const SAMPLE: &str = include_str!("fixtures/all-syntax.ok");

/// Runs `f` on a thread with the default 2 MiB stack used for spawned
/// threads, so stack exhaustion would show up here as a crash.
fn on_small_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(f)
        .expect("spawn test thread")
        .join()
        .expect("parser thread panicked")
}

fn nested(shape: &str, depth: usize) -> String {
    let (open, inner, close) = match shape {
        "parens" => ("(", "1", ")"),
        "brackets" => ("[", "1", "]"),
        "blocks" => ("{ ", "1;", " }"),
        "ifs" => ("if a { ", "1;", " }"),
        "unary" => ("-", "1", ""),
        "await" => ("await ", "x", ""),
        "calls" => ("f(", "1", ")"),
        "lambdas" => ("() is int { ", "1", "; }"),
        "types" => ("@", "int", ""),
        "generics" => ("Box<", "int", ">"),
        other => panic!("unknown shape {other}"),
    };
    let body = format!("{}{}{}", open.repeat(depth), inner, close.repeat(depth));
    match shape {
        "blocks" | "ifs" => format!("module app v1.0.0; main() is int {{ {body} }}"),
        "types" | "generics" => {
            format!("module app v1.0.0; main() is int {{ let x is {body} = y; }}")
        }
        _ => format!("module app v1.0.0; main() is int {{ let x = {body}; }}"),
    }
}

const SHAPES: [&str; 10] = [
    "parens", "brackets", "blocks", "ifs", "unary", "await", "calls", "lambdas", "types",
    "generics",
];

#[test]
fn deep_nesting_is_reported_not_overflowed() {
    on_small_stack(|| {
        for shape in SHAPES {
            let text = nested(shape, 100_000);
            let message = parse_in(&text, ParseMode::Full).expect_err(shape).0;
            assert!(
                message.starts_with("Nesting is too deep"),
                "{shape}: {message}"
            );
        }
    });
}

#[test]
fn nesting_up_to_the_limit_fits_a_small_stack() {
    on_small_stack(|| {
        for shape in SHAPES {
            // Find the deepest input of this shape that is still accepted
            // and parse it on the 2 MiB stack.
            let mut depth = ParseOptions::DEFAULT_MAX_NESTING;
            while parse_in(&nested(shape, depth), ParseMode::Full).is_err() {
                depth -= 1;
                assert!(depth > 0, "{shape}: even depth 1 fails");
            }
            assert!(depth >= 20, "{shape}: only {depth} levels are accepted");
        }
    });
}

#[test]
fn nesting_limit_is_configurable() {
    let text = nested("parens", 300);
    let source = SourceFile::new("deep.ok", text).unwrap();
    let options = ParseOptions {
        max_nesting: 1000,
        max_tree_depth: 2000,
        ..ParseOptions::default()
    };
    // A larger limit needs a larger stack; the caller owns that decision.
    let program = std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(move || parse(&source, &options).is_ok())
        .unwrap()
        .join()
        .unwrap();
    assert!(program);
}

#[test]
fn trailing_garbage_is_rejected() {
    let valid = "module app v1.0.0; main() is int { 0; }";
    assert!(parse_in(valid, ParseMode::Full).is_ok());
    for garbage in [
        "}",
        "x",
        ";",
        "0;",
        "main() is int { 0; } }",
        "import \"x\";",
        "let a = 1;",
    ] {
        let text = format!("{valid} {garbage}");
        assert!(parse_in(&text, ParseMode::Full).is_err(), "{garbage}");
    }
}

#[test]
fn incomplete_inputs_end_with_a_diagnostic() {
    for text in [
        "module",
        "module app",
        "module app v1",
        "module app v1.0.0",
        "module app v1.0.0; object",
        "module app v1.0.0; object Box",
        "module app v1.0.0; object Box {",
        "module app v1.0.0; object Box { has",
        "module app v1.0.0; object Box { Box(",
        "module app v1.0.0; role R { f(",
        "module app v1.0.0; main() is int {",
        "module app v1.0.0; main() is int { let x = ",
        "module app v1.0.0; main() is int { f(1, ",
        "module app v1.0.0; main() is int { [1, ",
        "module app v1.0.0; main() is int { a.",
        "module app v1.0.0; main() is int { match x { case",
        "module app v1.0.0; main() is int { try {} catch e is",
        "module app v1.0.0; main() is int { \"unterminated",
        "module app v1.0.0; main() is int { /* unterminated",
        "module app v1.0.0; main() is int { ```unterminated",
        "module app v1.0.0; test \"t\" {",
    ] {
        for mode in [ParseMode::Full, ParseMode::Production] {
            assert!(parse_in(text, mode).is_err(), "{text}");
        }
    }
}

#[test]
fn every_prefix_of_a_program_terminates() {
    // Every prefix that ends on a character boundary; most are errors.
    let mut accepted = 0;
    for (index, _) in SAMPLE.char_indices() {
        let prefix = &SAMPLE[..index];
        for mode in [ParseMode::Full, ParseMode::Production] {
            if parse_in(prefix, mode).is_ok() {
                accepted += 1;
            }
        }
    }
    assert!(parse_in(SAMPLE, ParseMode::Full).is_ok());
    // Prefixes ending after a complete declaration are valid programs.
    assert!(accepted > 0);
}

#[test]
fn deleting_any_single_token_terminates() {
    let tokens = lexer::lex(SAMPLE).tokens;
    for token in &tokens {
        if token.span.is_empty() {
            continue;
        }
        let text = format!(
            "{}{}",
            &SAMPLE[..token.span.start as usize],
            &SAMPLE[token.span.end as usize..]
        );
        let _ = parse_in(&text, ParseMode::Full);
        let _ = parse_in(&text, ParseMode::Production);
    }
}

#[test]
fn random_token_soup_terminates() {
    let words = [
        "module",
        "app",
        "v1",
        ".",
        "0.0",
        ";",
        "object",
        "Box",
        "{",
        "}",
        "(",
        ")",
        "[",
        "]",
        "is",
        "int",
        "let",
        "x",
        "=",
        "1",
        "+",
        "->",
        "<-",
        "...",
        "..",
        "::",
        ":",
        ",",
        "if",
        "else",
        "match",
        "case",
        "loop",
        "last",
        "next",
        "try",
        "catch",
        "throw",
        "panic",
        "await",
        "spawn",
        "async",
        "test",
        "fake",
        "\"s\"",
        "'c'",
        "@",
        "%",
        "#",
        "projected",
        "cap",
        "has",
        "self",
        "^",
        "<",
        ">",
        ">>",
        "&",
        "&&",
        "!",
        "?",
        "null",
        "with_x",
    ];
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..3000 {
        let mut text = String::from("module app v1.0.0; ");
        let length = next() % 60;
        for _ in 0..length {
            text.push_str(words[(next() % words.len() as u64) as usize]);
            text.push(' ');
        }
        let _ = parse_in(&text, ParseMode::Full);
        let _ = parse_in(&text, ParseMode::Production);
    }
}

#[test]
fn lexical_errors_prevent_parsing_and_are_all_reported() {
    let source = SourceFile::new(
        "t.ok",
        "module app v1.0.0; main() is int { $; 'ab'; \"\\q\"; 0; }",
    )
    .unwrap();
    let diagnostics = parse(&source, &ParseOptions::default()).unwrap_err();
    assert_eq!(diagnostics.len(), 3);
    assert!(
        diagnostics
            .iter()
            .all(|d| d.kind == okkama_compiler::DiagnosticKind::Lexical)
    );
}

#[test]
fn syntax_errors_point_at_the_offending_token() {
    let source = SourceFile::new(
        "t.ok",
        "module app v1.0.0;\nmain() is int {\n    let x = 1 2;\n}\n",
    )
    .unwrap();
    let diagnostics = parse(&source, &ParseOptions::default()).unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    let at = source.location(diagnostics[0].span.start);
    assert_eq!((at.line, at.column), (3, 15));
    assert_eq!(
        diagnostics[0].headline(&source),
        "t.ok:3:15: syntax error at \"2\": Expect \";\" after let declaration."
    );
    let eof = SourceFile::new("t.ok", "module app v1.0.0; main() is int {").unwrap();
    let diagnostics = parse(&eof, &ParseOptions::default()).unwrap_err();
    assert_eq!(
        diagnostics[0].headline(&eof),
        "t.ok:1:35: syntax error at end of input: Expect \"}\" after block."
    );
}

/// Chains the parser builds by a loop, not by recursion (H-03): the tree is
/// as deep as the chain is long, so only the tree budget bounds it.
fn chain(shape: &str, length: usize) -> String {
    let body = match shape {
        "field" => format!("x{}", ".a".repeat(length)),
        "call" => format!("x{}", ".f()".repeat(length)),
        "index" => format!("x{}", "[0]".repeat(length)),
        "select" => format!("x{}", "::a".repeat(length)),
        operator => vec!["a"; length + 1].join(&format!(" {operator} ")),
    };
    format!("module app v1.0.0; main() is int {{ let a = 1; let x = 1; let b = {body}; 0; }}")
}

const CHAINS: [&str; 15] = [
    "||", "&&", "<-", "==", "!=", "<", "|", "^", "&", "<<", "+", "*", "field", "call", "index",
];

#[test]
fn long_chains_are_reported_not_overflowed() {
    on_small_stack(|| {
        for shape in CHAINS {
            let longest = if shape == "+" { 1_000_000 } else { 300_000 };
            for length in [10_000, longest] {
                let message = parse_in(&chain(shape, length), ParseMode::Full).expect_err(shape).0;
                assert!(message.starts_with("Expression nesting is too deep"), "{shape} × {length}: {message}");
                // The same chain with a syntax error after it: still a diagnostic.
                let broken = chain(shape, length).replace("; 0; }", " 0; }");
                assert!(parse_in(&broken, ParseMode::Full).is_err(), "{shape} × {length}");
            }
        }
        let message = parse_in(&chain("select", 100_000), ParseMode::Full).expect_err("select").0;
        assert!(!message.is_empty());
    });
}

#[test]
fn chains_up_to_the_tree_limit_pass_every_phase_on_a_small_stack() {
    use okkama_compiler::syntax::{format, print};
    on_small_stack(|| {
        for shape in CHAINS {
            let mut length = ParseOptions::DEFAULT_MAX_TREE_DEPTH;
            while parse_in(&chain(shape, length), ParseMode::Full).is_err() {
                length -= 1;
                assert!(length > 0, "{shape}: even length 1 fails");
            }
            // A `.f()` link is two nodes (`Get` and `Call`).
            assert!(length >= 60, "{shape}: only {length} links are accepted");
            let file = common::source(&chain(shape, length));
            let program = parse(&file, &ParseOptions::default()).unwrap();
            // Semantic errors are expected (`||` over integers); the phases
            // must finish, not overflow.
            let _ = okkama_compiler::lowering::lower_program(&program);
            let _ = print::print_program(&program, &file, print::Format::Tree, Default::default());
            let _ = print::print_program(&program, &file, print::Format::Json, Default::default());
            let _ = format::format(&file);
        }
    });
}

#[test]
fn deep_expressions_drop_without_recursion() {
    on_small_stack(|| {
        let leaf = || Expr {
            kind: ExprKind::Name(Name { text: "a".into(), span: Span::empty(0) }),
            span: Span::empty(0),
        };
        let mut expr = leaf();
        for _ in 0..1_000_000 {
            expr = Expr {
                kind: ExprKind::Binary {
                    op: BinaryOp::Add,
                    op_span: Span::empty(0),
                    left: Box::new(expr),
                    right: Box::new(leaf()),
                },
                span: Span::empty(0),
            };
        }
        drop(expr);
    });
}

#[test]
fn program_size_is_limited() {
    let text = format!("module app v1.0.0; main() is int {{ {} 0; }}", "let x = 1; ".repeat(2000));
    let source = SourceFile::new("big.ok", text).unwrap();
    let options = ParseOptions { max_nodes: 1000, ..ParseOptions::default() };
    let error = parse(&source, &options).expect_err("too large");
    assert!(error[0].message.starts_with("Program is too large"), "{}", error[0].message);
}

