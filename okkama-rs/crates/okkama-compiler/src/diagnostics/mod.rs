//! Structured diagnostics.
//!
//! The library reports problems as values. A [`Diagnostic`] knows which
//! front-end phase produced it, its message, the exact source span and — for
//! syntax errors — the token the parser was looking at. Rendering to text is a
//! pure function ([`Diagnostic::render`]); writing the text anywhere is the
//! caller's job.

use std::fmt::{self, Write as _};

use crate::source::{SourceFile, Span};

/// The front-end phase that produced a diagnostic.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DiagnosticKind {
    /// Malformed characters, literals or comments (lexer).
    Lexical,
    /// A token sequence that the grammar does not accept (parser).
    Syntax,
}

impl DiagnosticKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DiagnosticKind::Lexical => "lexical",
            DiagnosticKind::Syntax => "syntax",
        }
    }
}

/// What the parser found at the error position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    /// A token with this exact source spelling.
    Token(String),
    /// The end of the input.
    EndOfInput,
}

/// One error. The front end does not emit warnings yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub message: String,
    pub span: Span,
    /// The offending token (syntax errors only).
    pub found: Option<Found>,
}

impl Diagnostic {
    pub fn lexical(message: impl Into<String>, span: Span) -> Self {
        Diagnostic {
            kind: DiagnosticKind::Lexical,
            message: message.into(),
            span,
            found: None,
        }
    }

    /// A syntax error about the tree as a whole (its depth or size), not
    /// about one token.
    pub fn syntax_limit(message: impl Into<String>, span: Span) -> Self {
        Diagnostic {
            kind: DiagnosticKind::Syntax,
            message: message.into(),
            span,
            found: None,
        }
    }

    pub fn syntax(message: impl Into<String>, span: Span, found: Found) -> Self {
        Diagnostic {
            kind: DiagnosticKind::Syntax,
            message: message.into(),
            span,
            found: Some(found),
        }
    }

    /// The one-line header, e.g.
    /// `main.ok:3:15: syntax error at "2": Expect ";" after expression.`
    pub fn headline(&self, source: &SourceFile) -> String {
        let location = source.location(self.span.start);
        let mut line = format!(
            "{}:{}:{}: {} error",
            source.name(),
            location.line,
            location.column,
            self.kind.as_str()
        );
        match &self.found {
            Some(Found::Token(lexeme)) => {
                let _ = write!(line, " at {}", quote(lexeme));
            }
            Some(Found::EndOfInput) => line.push_str(" at end of input"),
            None => {}
        }
        let _ = write!(line, ": {}", self.message);
        line
    }

    /// Renders the headline followed by the source line and a caret marker.
    pub fn render(&self, source: &SourceFile) -> String {
        let mut out = self.headline(source);
        out.push('\n');
        let start = source.location(self.span.start);
        let end = source.location(self.span.end);
        let line_text = source.line_text(start.line);
        let gutter = start.line.to_string();
        let pad = " ".repeat(gutter.len());
        let _ = writeln!(out, "{pad} |");
        let _ = writeln!(out, "{gutter} | {}", visible(line_text));
        // Underline to the end of the span, or to the end of the line for
        // spans that continue on later lines; at least one caret.
        let line_columns = line_text.chars().count() as u32 + 1;
        let last_column = if end.line == start.line {
            end.column
        } else {
            line_columns
        };
        let width = last_column.saturating_sub(start.column).max(1) as usize;
        let _ = writeln!(
            out,
            "{pad} | {}{}",
            " ".repeat(start.column.saturating_sub(1) as usize),
            "^".repeat(width)
        );
        out
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} error: {}", self.kind.as_str(), self.message)
    }
}

/// Renders a list of diagnostics, separated by blank lines.
pub fn render_all(diagnostics: &[Diagnostic], source: &SourceFile) -> String {
    let mut out = String::new();
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(&diagnostic.render(source));
    }
    out
}

fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Keeps the caret aligned: tabs and other control characters each occupy one
/// column in the location rule, so they are shown as a single space.
fn visible(line: &str) -> String {
    line.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_headline_snippet_and_caret() {
        let source = SourceFile::new("main.ok", "let x = 1 2;\n").unwrap();
        let diagnostic = Diagnostic::syntax(
            "Expect \";\" after expression.",
            Span::new(10, 11),
            Found::Token("2".into()),
        );
        assert_eq!(
            diagnostic.render(&source),
            "main.ok:1:11: syntax error at \"2\": Expect \";\" after expression.\n  |\n1 | let x = 1 2;\n  |           ^\n"
        );
    }

    #[test]
    fn end_of_input_and_lexical_headlines() {
        let source = SourceFile::new("a.ok", "x").unwrap();
        let eof = Diagnostic::syntax(
            "Expect \"}\" after block.",
            Span::empty(1),
            Found::EndOfInput,
        );
        assert_eq!(
            eof.headline(&source),
            "a.ok:1:2: syntax error at end of input: Expect \"}\" after block."
        );
        let lexical = Diagnostic::lexical("Unterminated string.", Span::new(0, 1));
        assert_eq!(
            lexical.headline(&source),
            "a.ok:1:1: lexical error: Unterminated string."
        );
    }

    #[test]
    fn multi_line_spans_underline_to_end_of_first_line() {
        let source = SourceFile::new("a.ok", "\"abc\ndef").unwrap();
        let diagnostic = Diagnostic::lexical("Unterminated string.", Span::new(0, 8));
        assert!(diagnostic.render(&source).ends_with("  | ^^^^\n"));
    }
}
