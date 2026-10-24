//! Token dumps (`okkama tokens`).
//!
//! The text form prints one token per line:
//!
//! ```text
//! 1:1-1:7      MODULE         "module"
//! 3:9-3:11     INT            "42"  int decimal 42
//! ```
//!
//! i.e. the position range (end column exclusive), the kind name, the
//! quoted source spelling and, for literals, the decoded value. The JSON
//! form is an array of objects with `kind`, `lexeme`, `span` and `value`.

use std::fmt::Write as _;

use super::json::{JsonWriter, quote};
use super::span_label;
use crate::source::SourceFile;
use crate::syntax::token::{Token, TokenValue};

fn value_text(value: &TokenValue) -> String {
    match value {
        TokenValue::Int(literal) => format!("int {} {}", literal.radix.as_str(), literal.digits),
        TokenValue::Float(literal) => format!("float {}", literal.text()),
        TokenValue::Str(literal) => {
            format!("string {} {}", literal.form.as_str(), quote(&literal.value))
        }
        TokenValue::Char(ch) => format!("char {}", quote(&ch.to_string())),
    }
}

/// One line per token, including the final `EOF`.
pub fn tokens_text(tokens: &[Token], source: &SourceFile) -> String {
    let mut out = String::new();
    for token in tokens {
        let position = span_label(token.span, source);
        let lexeme = quote(source.slice(token.span));
        let _ = write!(out, "{position:<12} {:<14} {lexeme}", token.kind.name());
        if let Some(value) = &token.value {
            let _ = write!(out, "  {}", value_text(value));
        }
        out.push('\n');
    }
    out
}

/// A JSON array of tokens.
pub fn tokens_json(tokens: &[Token], source: &SourceFile) -> String {
    let mut writer = JsonWriter::new(source);
    writer.open_array();
    for token in tokens {
        writer.array_element();
        writer.open_object();
        writer.key("kind");
        writer.string(token.kind.name());
        writer.key("lexeme");
        writer.string(source.slice(token.span));
        writer.key("span");
        writer.span(token.span);
        writer.key("value");
        match &token.value {
            None => writer.raw("null"),
            Some(value) => write_value(&mut writer, value),
        }
        writer.close_object();
    }
    writer.close_array();
    writer.out.push('\n');
    writer.out
}

fn write_value(writer: &mut JsonWriter<'_>, value: &TokenValue) {
    writer.open_object();
    match value {
        TokenValue::Int(literal) => {
            writer.key("type");
            writer.string("int");
            writer.key("radix");
            writer.string(literal.radix.as_str());
            writer.key("digits");
            writer.string(&literal.digits);
        }
        TokenValue::Float(literal) => {
            writer.key("type");
            writer.string("float");
            writer.key("value");
            writer.string(&literal.text());
        }
        TokenValue::Str(literal) => {
            writer.key("type");
            writer.string("string");
            writer.key("form");
            writer.string(literal.form.as_str());
            writer.key("value");
            writer.string(&literal.value);
        }
        TokenValue::Char(ch) => {
            writer.key("type");
            writer.string("char");
            writer.key("value");
            writer.string(&ch.to_string());
        }
    }
    writer.close_object();
}
