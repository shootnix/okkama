//! The syntactic front end: tokens, lexer, AST, parser, pruning, printers.
//!
//! The usual entry point is [`parse`]:
//!
//! ```
//! use okkama_compiler::SourceFile;
//! use okkama_compiler::syntax::{self, ParseOptions};
//!
//! let source = SourceFile::new("main.ok", "module app v1.0.0;\nmain() is int { 0; }\n").unwrap();
//! let program = syntax::parse(&source, &ParseOptions::default()).expect("valid program");
//! assert_eq!(program.items.len(), 1);
//! ```
//!
//! Parsing never touches the file system: imports are AST nodes and the
//! imported modules do not need to exist.

pub mod ast;
pub mod format;
pub mod lexer;
pub mod parser;
pub mod print;
pub mod prune;
pub mod token;

use crate::diagnostics::Diagnostic;
use crate::source::SourceFile;

/// How `test` and `fake` bodies are treated.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum ParseMode {
    /// Parse everything, including the bodies of `test` blocks and role
    /// `fake` blocks. This is the grammar of `okkama -t` and the default,
    /// because it loses nothing.
    #[default]
    Full,
    /// Production parsing as specified in §14 of the language
    /// specification: `test`/`fake` headers are parsed, their bodies are
    /// consumed as balanced braces without parsing the inner grammar (lexical
    /// errors still apply) and recorded as
    /// [`TestBody::Skipped`](ast::TestBody::Skipped) /
    /// [`FakeBody::Skipped`](ast::FakeBody::Skipped). Removing these nodes
    /// altogether is a separate step: [`prune::strip_test_items`].
    Production,
}

impl ParseMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ParseMode::Full => "full",
            ParseMode::Production => "production",
        }
    }
}

/// Parser configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseOptions {
    pub mode: ParseMode,
    /// Maximum syntactic nesting depth (parentheses, blocks, prefix
    /// operators, types, `else if` links, ...). Deeper input is reported as
    /// a syntax error instead of exhausting the native stack.
    pub max_nesting: usize,
}

impl ParseOptions {
    /// The default nesting limit.
    ///
    /// The deepest nesting in `std/`, `examples/` and the conformance
    /// sources is 20 levels, so 64 leaves ample room. At this limit the
    /// most stack-hungry shapes (nested anonymous actions and block
    /// statements, ~22 KiB per level in unoptimised builds, under 7 KiB
    /// optimised) still fit a 2 MiB thread stack; `parser_robustness`
    /// checks this.
    pub const DEFAULT_MAX_NESTING: usize = 64;
}

impl Default for ParseOptions {
    fn default() -> Self {
        ParseOptions {
            mode: ParseMode::Full,
            max_nesting: Self::DEFAULT_MAX_NESTING,
        }
    }
}

/// Lexes and parses one source file.
///
/// Returns the program, or the diagnostics that prevent building it: every
/// lexical error of the file (no program is built from a lexically invalid
/// source), or otherwise the first syntax error.
pub fn parse(source: &SourceFile, options: &ParseOptions) -> Result<ast::Program, Vec<Diagnostic>> {
    let lexed = lexer::lex(source.text());
    if lexed.has_errors() {
        return Err(lexed.diagnostics);
    }
    parser::parse_tokens(source.text(), lexed.tokens, options.clone()).map_err(|error| vec![error])
}
