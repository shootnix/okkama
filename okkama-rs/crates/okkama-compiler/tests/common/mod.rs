//! Shared helpers for the parser integration tests.
//!
//! `sexpr` renders the printer's node description (`print::program_node`)
//! as a compact S-expression such as `(+ 1 (* 2 3))`, so tests can state
//! tree shapes in one line. Kinds without a dedicated short form fall back
//! to `(Kind label=value ...)`, which keeps every node renderable.

#![allow(dead_code)]

pub mod json;

use okkama_compiler::source::SourceFile;
use okkama_compiler::syntax::print::{Node, Value, program_node};
use okkama_compiler::syntax::{ParseMode, ParseOptions, ast::Program, parse};

pub fn source(text: &str) -> SourceFile {
    SourceFile::new("test.ok", text).expect("test sources are small")
}

pub fn parse_in(text: &str, mode: ParseMode) -> Result<Program, (String, String, u32)> {
    let file = source(text);
    let options = ParseOptions {
        mode,
        ..ParseOptions::default()
    };
    parse(&file, &options).map_err(|diagnostics| {
        let first = &diagnostics[0];
        let found = match &first.found {
            Some(okkama_compiler::diagnostics::Found::Token(lexeme)) => lexeme.clone(),
            Some(okkama_compiler::diagnostics::Found::EndOfInput) => "<end>".to_string(),
            None => "<lexical>".to_string(),
        };
        (
            first.message.clone(),
            found,
            file.location(first.span.start).line,
        )
    })
}

/// Parses in full mode and panics with the diagnostic on failure.
pub fn parse_ok(text: &str) -> Program {
    match parse_in(text, ParseMode::Full) {
        Ok(program) => program,
        Err((message, found, line)) => {
            panic!("unexpected error at line {line} ({found}): {message}\nsource:\n{text}")
        }
    }
}

/// Parses in full mode and returns the first diagnostic message.
pub fn parse_err(text: &str) -> String {
    parse_err_in(text, ParseMode::Full)
}

pub fn parse_err_in(text: &str, mode: ParseMode) -> String {
    match parse_in(text, mode) {
        Ok(_) => panic!("source unexpectedly parsed:\n{text}"),
        Err((message, _, _)) => message,
    }
}

/// Asserts that parsing fails with a message containing `expected`.
#[track_caller]
pub fn assert_rejects(text: &str, expected: &str) {
    let message = parse_err(text);
    assert!(
        message.contains(expected),
        "expected an error containing {expected:?}, got {message:?}\nsource:\n{text}"
    );
}

/// The whole program as an S-expression.
pub fn program_sexpr(text: &str) -> String {
    sexpr_node(&program_node(&parse_ok(text)))
}

/// `body` placed inside `main() is int { body }`; returns the S-expressions
/// of the main body's statements, separated by `; `.
pub fn body_sexpr(body: &str) -> String {
    let text = format!("module app v1.0.0; main() is int {{ {body} }}");
    let node = program_node(&parse_ok(&text));
    let items = list(node.get("items").unwrap());
    let main = items.last().unwrap().as_node().unwrap();
    let block = main.get("body").unwrap().as_node().unwrap();
    list(block.get("statements").unwrap())
        .iter()
        .map(sexpr)
        .collect::<Vec<_>>()
        .join("; ")
}

/// The S-expression of a single expression.
pub fn expr_sexpr(expr: &str) -> String {
    body_sexpr(&format!("{expr};"))
}

fn list(value: &Value) -> &[Value] {
    match value {
        Value::List(items) => items,
        other => panic!("not a list: {other:?}"),
    }
}

fn field<'a>(node: &'a Node, name: &str) -> &'a Value {
    node.get(name)
        .unwrap_or_else(|| panic!("{} has no field {name}", node.kind))
}

fn word(value: &Value) -> String {
    match value {
        Value::Word(word) | Value::Text(word) => word.clone(),
        Value::Name { text, .. } => text.clone(),
        other => sexpr(other),
    }
}

pub fn sexpr(value: &Value) -> String {
    match value {
        Value::Node(node) => sexpr_node(node),
        Value::List(items) => format!(
            "[{}]",
            items.iter().map(sexpr).collect::<Vec<_>>().join(" ")
        ),
        Value::Name { text, .. } => text.clone(),
        Value::Text(text) => format!("{text:?}"),
        Value::Word(word) => word.clone(),
        Value::Bool(flag) => flag.to_string(),
        Value::Absent => "_".to_string(),
    }
}

fn args(node: &Node) -> String {
    list(field(node, "arguments"))
        .iter()
        .map(|arg| {
            let arg = arg.as_node().unwrap();
            match field(arg, "name") {
                Value::Absent => sexpr(field(arg, "value")),
                name => format!("{}:{}", sexpr(name), sexpr(field(arg, "value"))),
            }
        })
        .map(|text| format!(" {text}"))
        .collect()
}

pub fn sexpr_node(node: &Node) -> String {
    let f = |name: &str| sexpr(field(node, name));
    match node.kind {
        "NameExpression" => f("name"),
        "SelfExpression" => "self".into(),
        "IntLiteral" => match word(field(node, "radix")).as_str() {
            "decimal" => f("digits"),
            "hexadecimal" => format!("0x{}", f("digits")),
            _ => format!("0b{}", f("digits")),
        },
        "FloatLiteral" => f("value"),
        "StringLiteral" => match word(field(node, "form")).as_str() {
            "endl" => "endl".into(),
            "fenced" => format!("```{}```", word(field(node, "value"))),
            _ => f("value"),
        },
        "CharLiteral" => format!("'{}'", word(field(node, "value"))),
        "BoolLiteral" => f("value"),
        "Grouping" => format!("(group {})", f("expression")),
        "Unary" => format!("({} {})", f("operator"), f("operand")),
        "Binary" => format!("({} {} {})", f("operator"), f("left"), f("right")),
        "StreamWrite" => format!("(<- {} {})", f("sink"), f("source")),
        "Range" => format!("({} {} {})", f("operator"), f("start"), f("end")),
        "Call" => format!("(call {}{})", f("callee"), args(node)),
        "ArrayCast" => format!("(@{}{})", f("target"), args(node)),
        "Get" => format!("(. {} {})", f("object"), f("name")),
        "Select" => format!("(:: {} {})", f("object"), f("name")),
        "OwnerField" => format!("(owner:: {} {})", f("owner"), f("name")),
        "Index" => match field(node, "fallback") {
            Value::Absent => format!("(index {} {})", f("object"), f("index")),
            _ => format!(
                "(index {} {} else {})",
                f("object"),
                f("index"),
                f("fallback")
            ),
        },
        "Unpack" => format!("(... {})", f("operand")),
        "Projection" => format!("(projection {} {})", f("prefix"), f("construction")),
        "Functional" => format!(
            "({} {}{} -> {} {})",
            f("operation"),
            f("receiver"),
            args(node),
            f("bindings"),
            f("body")
        ),
        "AnonymousAction" => format!("(fn {} {} {})", f("params"), f("result"), f("body")),
        "Param" => {
            let variadic = matches!(field(node, "variadic"), Value::Bool(true));
            format!(
                "{} {}{}",
                f("name"),
                if variadic { "..." } else { "" },
                f("type")
            )
        }
        "ArrayLiteral" => format!(
            "(array{})",
            list(field(node, "elements"))
                .iter()
                .map(|e| format!(" {}", sexpr(e)))
                .collect::<String>()
        ),
        "MapLiteral" => format!(
            "(map{})",
            list(field(node, "entries"))
                .iter()
                .map(|entry| {
                    let entry = entry.as_node().unwrap();
                    format!(
                        " {}:{}",
                        sexpr(field(entry, "key")),
                        sexpr(field(entry, "value"))
                    )
                })
                .collect::<String>()
        ),
        "IfExpression" => format!("(if {} {} {})", f("condition"), f("then"), f("else")),
        "LoopExpression" => format!("(loop {})", f("body")),
        "Panic" => format!("(panic {})", f("value")),
        "Await" => format!("(await {})", f("value")),
        "Spawn" => format!("(spawn {})", f("call")),
        "Block" => format!(
            "{{{}}}",
            list(field(node, "statements"))
                .iter()
                .map(sexpr)
                .collect::<Vec<_>>()
                .join("; ")
        ),
        "ExpressionStatement" => match field(node, "condition") {
            Value::Absent => f("expression"),
            _ => format!("{} if {}", f("expression"), f("condition")),
        },
        "NamedType" => {
            let path = list(field(node, "path"))
                .iter()
                .map(sexpr)
                .collect::<Vec<_>>()
                .join(".");
            let type_args = list(field(node, "args"));
            if type_args.is_empty() {
                path
            } else {
                format!(
                    "{path}<{}>",
                    type_args.iter().map(sexpr).collect::<Vec<_>>().join(", ")
                )
            }
        }
        "ArrayType" => format!("@{}", f("element")),
        "MapType" => format!("%{}:{}", f("key"), f("value")),
        "NestedType" => format!("{}::{}", f("owner"), f("name")),
        "VoidResult" => "void".into(),
        "SuccessorResult" => format!("^{}", f("type")),
        "ActionType" => {
            let params = list(field(node, "params"))
                .iter()
                .map(sexpr)
                .collect::<Vec<_>>()
                .join(", ");
            let prefix = if matches!(field(node, "async"), Value::Bool(true)) {
                "async "
            } else {
                ""
            };
            format!("{prefix}({params}) is {}", f("result"))
        }
        _ => {
            let mut out = format!("({}", node.kind);
            for (label, value) in &node.fields {
                out.push_str(&format!(" {label}={}", sexpr(value)));
            }
            out.push(')');
            out
        }
    }
}
