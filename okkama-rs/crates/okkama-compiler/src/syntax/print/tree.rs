//! The `tree` format: an indented tree with node kinds and labelled fields.
//!
//! ```text
//! Program
//!   module: ModuleDecl
//!     path: [app]
//!     version: v1.0.0
//!   imports: []
//!   items:
//!     - ActionDecl
//!       async: false
//!       ...
//! ```
//!
//! Rules: a node prints its kind; each field prints `label: value` one level
//! deeper. Scalars and lists of scalars stay on the field line; nested nodes
//! and lists of nodes continue on the following lines (list items start with
//! `- `). Absent optional fields print `(none)`, string data is quoted with
//! JSON escapes, and spans (with `--spans`) are appended as `@L:C-L:C`.

use super::{Node, PrintOptions, Value, span_label};
use crate::source::{SourceFile, Span};

pub(super) fn render(node: &Node, source: &SourceFile, options: PrintOptions) -> String {
    let mut printer = TreePrinter {
        source,
        options,
        out: String::new(),
    };
    printer.out.push_str(&printer.header(node));
    printer.out.push('\n');
    printer.fields(node, 1);
    printer.out
}

struct TreePrinter<'a> {
    source: &'a SourceFile,
    options: PrintOptions,
    out: String,
}

impl TreePrinter<'_> {
    fn header(&self, node: &Node) -> String {
        format!("{}{}", node.kind, self.span_suffix(node.span))
    }

    fn span_suffix(&self, span: Span) -> String {
        if self.options.spans {
            format!(" @{}", span_label(span, self.source))
        } else {
            String::new()
        }
    }

    fn line(&mut self, indent: usize, text: &str) {
        for _ in 0..indent {
            self.out.push_str("  ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn fields(&mut self, node: &Node, indent: usize) {
        for (label, value) in &node.fields {
            self.field(label, value, indent);
        }
    }

    fn field(&mut self, label: &str, value: &Value, indent: usize) {
        match value {
            Value::Node(child) => {
                let header = self.header(child);
                self.line(indent, &format!("{label}: {header}"));
                self.fields(child, indent + 1);
            }
            Value::List(items) if items.iter().all(Value::is_scalar) => {
                let inline: Vec<String> = items.iter().map(|item| self.scalar(item)).collect();
                self.line(indent, &format!("{label}: [{}]", inline.join(", ")));
            }
            Value::List(items) => {
                self.line(indent, &format!("{label}:"));
                for item in items {
                    self.item(item, indent + 1);
                }
            }
            scalar => {
                let text = self.scalar(scalar);
                self.line(indent, &format!("{label}: {text}"));
            }
        }
    }

    fn item(&mut self, value: &Value, indent: usize) {
        match value {
            Value::Node(child) => {
                let header = self.header(child);
                self.line(indent, &format!("- {header}"));
                self.fields(child, indent + 1);
            }
            Value::List(items) => {
                self.line(indent, "-");
                for nested in items {
                    self.item(nested, indent + 1);
                }
            }
            scalar => {
                let text = self.scalar(scalar);
                self.line(indent, &format!("- {text}"));
            }
        }
    }

    fn scalar(&self, value: &Value) -> String {
        match value {
            Value::Name { text, span } => format!("{text}{}", self.span_suffix(*span)),
            Value::Text(text) => super::json::quote(text),
            Value::Word(word) => word.clone(),
            Value::Bool(flag) => flag.to_string(),
            Value::Absent => "(none)".to_string(),
            // Not scalars; callers dispatch them before reaching here.
            Value::Node(node) => self.header(node),
            Value::List(_) => "[...]".to_string(),
        }
    }
}
