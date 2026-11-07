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
        // spans that continue on later lines; at least one caret. Columns are
        // characters, the terminal shows cells: a wide character takes two, a
        // combining mark none (#48).
        let chars: Vec<char> = line_text.chars().collect();
        let first = (start.column.saturating_sub(1) as usize).min(chars.len());
        let last = if end.line == start.line {
            (end.column.saturating_sub(1) as usize).clamp(first, chars.len())
        } else {
            chars.len()
        };
        let indent: usize = chars[..first].iter().map(|&ch| display_width(ch)).sum();
        let width = chars[first..last].iter().map(|&ch| display_width(ch)).sum::<usize>().max(1);
        let _ = writeln!(out, "{pad} | {}{}", " ".repeat(indent), "^".repeat(width));
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
/// Terminal cells of `ch` as [`visible`] prints it: control characters
/// (tabs too) become one space, combining marks and zero-width joiners take
/// none, East Asian wide characters and emoji take two.
pub fn display_width(ch: char) -> usize {
    let code = ch as u32;
    if ch.is_control() {
        return 1;
    }
    let zero = matches!(code,
        0x0300..=0x036F | 0x0483..=0x0489 | 0x0591..=0x05BD | 0x0610..=0x061A | 0x064B..=0x065F
        | 0x0E31 | 0x0E34..=0x0E3A | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x200B..=0x200F
        | 0x20D0..=0x20FF | 0xFE00..=0xFE0F | 0xFE20..=0xFE2F | 0xE0100..=0xE01EF);
    if zero {
        return 0;
    }
    let wide = matches!(code,
        0x1100..=0x115F | 0x231A..=0x231B | 0x2329..=0x232A | 0x23E9..=0x23EC | 0x23F0 | 0x23F3
        | 0x25FD..=0x25FE | 0x2614..=0x2615 | 0x2648..=0x2653 | 0x267F | 0x2693 | 0x26A1
        | 0x26AA..=0x26AB | 0x26BD..=0x26BE | 0x26C4..=0x26C5 | 0x26CE | 0x26D4 | 0x26EA
        | 0x26F2..=0x26F3 | 0x26F5 | 0x26FA | 0x26FD | 0x2705 | 0x270A..=0x270B | 0x2728 | 0x274C
        | 0x274E | 0x2753..=0x2755 | 0x2757 | 0x2795..=0x2797 | 0x27B0 | 0x27BF | 0x2B1B..=0x2B1C
        | 0x2B50 | 0x2B55 | 0x2E80..=0x303E | 0x3041..=0x33FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF | 0xA960..=0xA97F | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE10..=0xFE19
        | 0xFE30..=0xFE6F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6 | 0x1F004 | 0x1F0CF | 0x1F18E
        | 0x1F191..=0x1F19A | 0x1F200..=0x1F2FF | 0x1F300..=0x1F64F | 0x1F680..=0x1F6FF
        | 0x1F7E0..=0x1F7EB | 0x1F90C..=0x1F9FF | 0x1FA70..=0x1FAFF | 0x20000..=0x3FFFD);
    if wide { 2 } else { 1 }
}

fn visible(line: &str) -> String {
    line.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Carets under wide characters, combining marks and tabs (#48).
    #[test]
    fn carets_follow_display_width() {
        let caret = |text: &str, needle: &str| {
            let source = SourceFile::new("main.ok", text).unwrap();
            let start = text.find(needle).unwrap() as u32;
            let diagnostic = Diagnostic::lexical("x", Span::new(start, start + needle.len() as u32));
            diagnostic.render(&source).lines().last().unwrap().to_string()
        };
        assert_eq!(caret("let 名前 = 1;\n", "1"), format!("  | {}^", " ".repeat(11)));
        assert_eq!(caret("\"😀\" bad\n", "bad"), "  |      ^^^");
        assert_eq!(caret("\"e\u{301}\" bad\n", "bad"), "  |     ^^^");
        assert_eq!(caret("\tbad\n", "bad"), "  |  ^^^");
        assert_eq!(caret("x = \"名前\";\n", "\"名前\""), "  |     ^^^^^^");
        assert_eq!(display_width('Ж'), 1);
    }

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
