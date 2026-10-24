//! AST and token printers.
//!
//! Both AST formats are rendered from one intermediate, format-neutral
//! description of the tree ([`Node`] / [`Value`]) built by
//! [`program_node`]. The conversion matches every AST type exhaustively, so
//! adding an AST variant without teaching the printers about it is a compile
//! error, and the `tree` and `json` outputs can never disagree about what
//! the tree contains.
//!
//! * [`Format::Tree`] — an indented, labelled tree for people and snapshot
//!   tests;
//! * [`Format::Json`] — deterministic pretty-printed JSON for tools.
//!
//! Output is stable: field order is fixed by the conversion code, lists keep
//! source order, and nothing depends on addresses, hashing or time. Source
//! positions are printed only with [`PrintOptions::spans`].

mod build;
mod json;
mod tokens;
mod tree;

pub use build::program_node;
pub use tokens::{tokens_json, tokens_text};

use crate::source::{SourceFile, Span};
use crate::syntax::ast::Program;

/// One AST node in printer form: a kind name, its span and labelled fields.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub kind: &'static str,
    pub span: Span,
    pub fields: Vec<(&'static str, Value)>,
}

impl Node {
    pub fn new(kind: &'static str, span: Span) -> Self {
        Node {
            kind,
            span,
            fields: Vec::new(),
        }
    }

    pub fn field(mut self, name: &'static str, value: Value) -> Self {
        self.fields.push((name, value));
        self
    }

    /// The value of a field, for tests and tools.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.fields
            .iter()
            .find(|(field, _)| *field == name)
            .map(|(_, value)| value)
    }
}

/// A field value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Node(Box<Node>),
    List(Vec<Value>),
    /// A source name (identifier, type name, label) with its span.
    Name {
        text: String,
        span: Span,
    },
    /// String data, shown quoted (literal values, import paths, test names).
    Text(String),
    /// A bare word: operators, enum-like attributes, digit strings.
    Word(String),
    Bool(bool),
    /// An optional field that is not present.
    Absent,
}

impl Value {
    pub fn node(node: Node) -> Value {
        Value::Node(Box::new(node))
    }

    pub fn word(text: impl Into<String>) -> Value {
        Value::Word(text.into())
    }

    pub fn as_node(&self) -> Option<&Node> {
        match self {
            Value::Node(node) => Some(node),
            _ => None,
        }
    }

    fn is_scalar(&self) -> bool {
        !matches!(self, Value::Node(_) | Value::List(_))
    }
}

/// An AST output format.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    Tree,
    Json,
}

/// Printer options shared by both formats.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PrintOptions {
    /// Include source spans (byte offsets plus line/column ranges).
    pub spans: bool,
}

/// Prints a whole program. `source` is needed only to turn spans into
/// lines and columns.
pub fn print_program(
    program: &Program,
    source: &SourceFile,
    format: Format,
    options: PrintOptions,
) -> String {
    let node = program_node(program);
    match format {
        Format::Tree => tree::render(&node, source, options),
        Format::Json => json::render(&node, source, options),
    }
}

/// `L:C-L:C` for a span (end column is exclusive).
pub(crate) fn span_label(span: Span, source: &SourceFile) -> String {
    let start = source.location(span.start);
    let end = source.location(span.end);
    format!(
        "{}:{}-{}:{}",
        start.line, start.column, end.line, end.column
    )
}
