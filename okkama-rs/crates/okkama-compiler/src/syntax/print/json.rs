//! The `json` format: deterministic, pretty-printed JSON.
//!
//! * a node is an object whose first key is `"node"` (the kind), followed by
//!   `"span"` when spans are requested, then the fields in their fixed order;
//! * a list is an array, `Absent` is `null`, booleans are booleans;
//! * names, words (operators, digit strings, ...) and text are strings —
//!   integer digits stay strings so no literal is ever rounded;
//! * with spans, a name becomes `{"name": "...", "span": {...}}`.
//!
//! A span is `{"start": byte, "end": byte, "start_line", "start_column",
//! "end_line", "end_column"}` (1-based lines and columns, end exclusive).
//!
//! The JSON writer is hand-written to keep the crate free of dependencies;
//! it escapes `"`, `\\` and control characters and writes everything else as
//! UTF-8.

use std::fmt::Write as _;

use super::{Node, PrintOptions, Value};
use crate::source::{SourceFile, Span};

pub(super) fn render(node: &Node, source: &SourceFile, options: PrintOptions) -> String {
    let mut writer = JsonWriter::new(source);
    writer.node(node, options);
    writer.out.push('\n');
    writer.out
}

/// JSON string literal for `text`.
pub(crate) fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A small pretty-printing JSON writer, shared with the token printer.
///
/// Objects are written with `open_object` / `key` / `close_object`; the
/// writer tracks whether a separator is needed, so callers only describe the
/// structure.
pub(crate) struct JsonWriter<'a> {
    pub(crate) out: String,
    source: &'a SourceFile,
    /// One entry per open object/array: whether it already has an element.
    has_elements: Vec<bool>,
}

impl<'a> JsonWriter<'a> {
    pub(crate) fn new(source: &'a SourceFile) -> Self {
        JsonWriter {
            out: String::new(),
            source,
            has_elements: Vec::new(),
        }
    }

    fn newline(&mut self) {
        self.out.push('\n');
        for _ in 0..self.has_elements.len() {
            self.out.push_str("  ");
        }
    }

    /// Starts the next element of the innermost object or array.
    fn element(&mut self) {
        if let Some(has) = self.has_elements.last_mut() {
            if *has {
                self.out.push(',');
            }
            *has = true;
        }
        self.newline();
    }

    pub(crate) fn open_object(&mut self) {
        self.out.push('{');
        self.has_elements.push(false);
    }

    pub(crate) fn close_object(&mut self) {
        let had_elements = self.has_elements.pop().unwrap_or(false);
        if had_elements {
            self.newline();
        }
        self.out.push('}');
    }

    pub(crate) fn open_array(&mut self) {
        self.out.push('[');
        self.has_elements.push(false);
    }

    pub(crate) fn close_array(&mut self) {
        let had_elements = self.has_elements.pop().unwrap_or(false);
        if had_elements {
            self.newline();
        }
        self.out.push(']');
    }

    /// Starts `"key": ` inside an object.
    pub(crate) fn key(&mut self, key: &str) {
        self.element();
        self.out.push_str(&quote(key));
        self.out.push_str(": ");
    }

    /// Starts the next element inside an array.
    pub(crate) fn array_element(&mut self) {
        self.element();
    }

    pub(crate) fn string(&mut self, text: &str) {
        self.out.push_str(&quote(text));
    }

    pub(crate) fn raw(&mut self, json: &str) {
        self.out.push_str(json);
    }

    pub(crate) fn span(&mut self, span: Span) {
        let start = self.source.location(span.start);
        let end = self.source.location(span.end);
        self.open_object();
        for (key, number) in [
            ("start", span.start),
            ("end", span.end),
            ("start_line", start.line),
            ("start_column", start.column),
            ("end_line", end.line),
            ("end_column", end.column),
        ] {
            self.key(key);
            let _ = write!(self.out, "{number}");
        }
        self.close_object();
    }

    fn node(&mut self, node: &Node, options: PrintOptions) {
        self.open_object();
        self.key("node");
        self.string(node.kind);
        if options.spans {
            self.key("span");
            self.span(node.span);
        }
        for (label, value) in &node.fields {
            self.key(label);
            self.value(value, options);
        }
        self.close_object();
    }

    fn value(&mut self, value: &Value, options: PrintOptions) {
        match value {
            Value::Node(node) => self.node(node, options),
            Value::List(items) => {
                self.open_array();
                for item in items {
                    self.array_element();
                    self.value(item, options);
                }
                self.close_array();
            }
            Value::Name { text, span } => {
                if options.spans {
                    self.open_object();
                    self.key("name");
                    self.string(text);
                    self.key("span");
                    self.span(*span);
                    self.close_object();
                } else {
                    self.string(text);
                }
            }
            Value::Text(text) | Value::Word(text) => self.string(text),
            Value::Bool(flag) => self.raw(if *flag { "true" } else { "false" }),
            Value::Absent => self.raw("null"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_escapes_json_specials_only() {
        assert_eq!(quote("a\"b\\c"), r#""a\"b\\c""#);
        assert_eq!(quote("\n\r\t\u{8}\u{c}"), r#""\n\r\t\b\f""#);
        assert_eq!(quote("\u{0}\u{1f}\u{7f}"), r#""\u0000\u001f\u007f""#);
        assert_eq!(quote("é🙂"), "\"é🙂\"");
    }

    #[test]
    fn writer_layout_is_stable() {
        let source = SourceFile::new("t.ok", "ab").unwrap();
        let mut writer = JsonWriter::new(&source);
        writer.open_object();
        writer.key("empty");
        writer.open_array();
        writer.close_array();
        writer.key("list");
        writer.open_array();
        writer.array_element();
        writer.string("x");
        writer.array_element();
        writer.raw("null");
        writer.close_array();
        writer.key("object");
        writer.open_object();
        writer.close_object();
        writer.close_object();
        assert_eq!(
            writer.out,
            "{\n  \"empty\": [],\n  \"list\": [\n    \"x\",\n    null\n  ],\n  \"object\": {}\n}"
        );
    }
}
