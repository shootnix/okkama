//! `runtime/json`: строгий разбор JSON (RFC 8259) в дерево узлов — перенос
//! `JsonNatives` JVM. Смещения, строки и столбцы — в символах (code points),
//! тексты отказов — как у Perl-provider.
use super::Error;
use super::support::{failure, handle, int, int_value, new_handle, string, text};
use crate::{IntValue, Value, abi::Native};
use std::cell::RefCell;
use std::rc::Rc;

/// Глубина рекурсии разбора, на которой JVM теряет стек (`StackOverflowError`).
const STACK_DEPTH: i64 = 100_000;

#[derive(Debug)]
enum Node {
    Null,
    Boolean(bool),
    Number(String),
    Text(String),
    Array(Vec<Rc<Node>>),
    Object(Vec<String>, Vec<Rc<Node>>),
}

impl Node {
    fn kind(&self) -> i64 {
        match self {
            Node::Null => 0,
            Node::Boolean(_) => 1,
            Node::Number(_) => 2,
            Node::Text(_) => 3,
            Node::Array(_) => 4,
            Node::Object(..) => 5,
        }
    }
}

struct ParseError {
    offset: usize,
    message: String,
    duplicate: Option<String>,
}

struct Parser<'a> {
    source: &'a [char],
    maximum_depth: &'a IntValue,
    position: usize,
}

type Parsed<T> = Result<T, ParseError>;

impl Parser<'_> {
    fn error(&self, message: &str) -> ParseError {
        ParseError {
            offset: self.position,
            message: message.into(),
            duplicate: None,
        }
    }

    fn at(&self, position: usize) -> Option<char> {
        self.source.get(position).copied()
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.at(self.position), Some(' ' | '\t' | '\n' | '\r')) {
            self.position += 1;
        }
    }

    fn hex4(&self, position: usize) -> Option<u32> {
        if position + 4 > self.source.len() {
            return None;
        }
        let mut value = 0;
        for c in &self.source[position..position + 4] {
            value = value * 16 + c.to_digit(16).filter(|_| *c <= 'f')?;
        }
        Some(value)
    }

    fn string(&mut self) -> Parsed<String> {
        if self.at(self.position) != Some('"') {
            return Err(self.error("expected JSON string"));
        }
        self.position += 1;
        let mut result = String::new();
        let fail = |offset, message: &str| ParseError {
            offset,
            message: message.into(),
            duplicate: None,
        };
        while self.position < self.source.len() {
            let offset = self.position;
            let character = self.source[self.position];
            self.position += 1;
            if character == '"' {
                return Ok(result);
            }
            if (character as u32) < 0x20 {
                return Err(fail(offset, "unescaped control character in JSON string"));
            }
            if character != '\\' {
                result.push(character);
                continue;
            }
            let Some(escape) = self.at(self.position) else {
                return Err(fail(offset, "unfinished JSON escape"));
            };
            self.position += 1;
            let simple = match escape {
                '"' => Some('"'),
                '\\' => Some('\\'),
                '/' => Some('/'),
                'b' => Some('\u{08}'),
                'f' => Some('\u{0c}'),
                'n' => Some('\n'),
                'r' => Some('\r'),
                't' => Some('\t'),
                'u' => None,
                _ => return Err(fail(offset, "invalid JSON escape")),
            };
            if let Some(simple) = simple {
                result.push(simple);
                continue;
            }
            if self.position + 4 > self.source.len() {
                return Err(fail(offset, "incomplete Unicode escape"));
            }
            let Some(mut codepoint) = self.hex4(self.position) else {
                return Err(fail(offset, "invalid Unicode escape"));
            };
            self.position += 4;
            if (0xD800..=0xDBFF).contains(&codepoint) {
                if !(self.at(self.position) == Some('\\')
                    && self.at(self.position + 1) == Some('u')
                    && self.position + 6 <= self.source.len())
                {
                    return Err(fail(offset, "unpaired UTF-16 high surrogate"));
                }
                let low = self.hex4(self.position + 2).unwrap_or(0);
                if !(0xDC00..=0xDFFF).contains(&low) {
                    return Err(fail(offset, "unpaired UTF-16 high surrogate"));
                }
                self.position += 6;
                codepoint = 0x10000 + ((codepoint - 0xD800) << 10) + (low - 0xDC00);
            } else if (0xDC00..=0xDFFF).contains(&codepoint) {
                return Err(fail(offset, "unpaired UTF-16 low surrogate"));
            }
            result.push(char::from_u32(codepoint).expect("scalar"));
        }
        Err(self.error("unterminated JSON string"))
    }

    fn digit(&self) -> bool {
        self.at(self.position).is_some_and(|c| c.is_ascii_digit())
    }

    fn number(&mut self) -> Parsed<String> {
        let start = self.position;
        if self.at(self.position) == Some('-') {
            self.position += 1;
        }
        if !self.digit() {
            return Err(self.error("expected digit in JSON number"));
        }
        let whole_start = self.position;
        if self.at(self.position) == Some('0') {
            self.position += 1;
            if self.digit() {
                return Err(self.error("leading zero in JSON number"));
            }
        } else {
            while self.digit() {
                self.position += 1;
            }
        }
        let whole_end = self.position;
        let (mut fraction_start, mut fraction_end) = (self.position, self.position);
        if self.at(self.position) == Some('.') {
            self.position += 1;
            if !self.digit() {
                return Err(self.error("expected digit after decimal point"));
            }
            fraction_start = self.position;
            while self.digit() {
                self.position += 1;
            }
            fraction_end = self.position;
        }
        let mut written: String = "0".into();
        if matches!(self.at(self.position), Some('e' | 'E')) {
            self.position += 1;
            let exponent_start = self.position;
            if matches!(self.at(self.position), Some('+' | '-')) {
                self.position += 1;
            }
            if !self.digit() {
                return Err(self.error("expected digit in JSON exponent"));
            }
            while self.digit() {
                self.position += 1;
            }
            written = self.source[exponent_start..self.position].iter().collect();
        }
        let digits: String = self.source[whole_start..whole_end]
            .iter()
            .chain(&self.source[fraction_start..fraction_end])
            .collect();
        let written = written.strip_prefix('+').unwrap_or(&written);
        let written = IntValue::from_big(crate::bignum::Big::parse(written).expect("exponent digits"));
        let mut exponent = crate::integer::subtract(
            &written,
            &IntValue::from_i64((fraction_end - fraction_start) as i64),
        );
        let digits = digits.trim_start_matches('0');
        if !digits.is_empty() {
            let zeros = digits.len() - digits.trim_end_matches('0').len();
            exponent = crate::integer::add(&exponent, &IntValue::from_i64(zeros as i64));
        }
        if exponent.to_i64().is_none() {
            return Err(ParseError {
                offset: start,
                message: "JSON number exponent is outside supported range".into(),
                duplicate: None,
            });
        }
        Ok(self.source[start..self.position].iter().collect())
    }

    fn value(&mut self, depth: i64) -> Parsed<Rc<Node>> {
        self.skip_whitespace();
        if crate::integer::compare(&IntValue::from_i64(depth), self.maximum_depth).is_gt() {
            return Err(self.error("maximum JSON depth exceeded"));
        }
        if depth > STACK_DEPTH {
            return Err(ParseError {
                offset: self.position,
                message: "\u{0}overflow".into(),
                duplicate: None,
            });
        }
        let Some(character) = self.at(self.position) else {
            return Err(self.error("expected JSON value"));
        };
        Ok(Rc::new(match character {
            '"' => Node::Text(self.string()?),
            '[' => self.array(depth)?,
            '{' => self.object(depth)?,
            '-' | '0'..='9' => Node::Number(self.number()?),
            _ if self.literal("null") => Node::Null,
            _ if self.literal("false") => Node::Boolean(false),
            _ if self.literal("true") => Node::Boolean(true),
            _ => return Err(self.error("expected JSON value")),
        }))
    }

    fn literal(&mut self, word: &str) -> bool {
        let end = self.position + word.len();
        if end > self.source.len() || !self.source[self.position..end].iter().copied().eq(word.chars()) {
            return false;
        }
        self.position = end;
        true
    }

    fn array(&mut self, depth: i64) -> Parsed<Node> {
        self.position += 1;
        self.skip_whitespace();
        let mut values = Vec::new();
        if self.at(self.position) == Some(']') {
            self.position += 1;
            return Ok(Node::Array(values));
        }
        loop {
            values.push(self.value(depth + 1)?);
            self.skip_whitespace();
            match self.at(self.position) {
                Some(']') => {
                    self.position += 1;
                    return Ok(Node::Array(values));
                }
                Some(',') => {}
                _ => return Err(self.error("expected comma or closing bracket in JSON array")),
            }
            self.position += 1;
            self.skip_whitespace();
            if self.at(self.position) == Some(']') {
                return Err(self.error("trailing comma in JSON array"));
            }
        }
    }

    fn object(&mut self, depth: i64) -> Parsed<Node> {
        self.position += 1;
        self.skip_whitespace();
        let (mut names, mut values) = (Vec::new(), Vec::new());
        let mut seen = std::collections::HashSet::new();
        if self.at(self.position) == Some('}') {
            self.position += 1;
            return Ok(Node::Object(names, values));
        }
        loop {
            let name_offset = self.position;
            let name = self.string()?;
            if !seen.insert(name.clone()) {
                return Err(ParseError {
                    offset: name_offset,
                    message: format!("duplicate JSON member \"{name}\""),
                    duplicate: Some(name),
                });
            }
            self.skip_whitespace();
            if self.at(self.position) != Some(':') {
                return Err(self.error("expected colon after JSON member name"));
            }
            self.position += 1;
            names.push(name);
            values.push(self.value(depth + 1)?);
            self.skip_whitespace();
            match self.at(self.position) {
                Some('}') => {
                    self.position += 1;
                    return Ok(Node::Object(names, values));
                }
                Some(',') => {}
                _ => return Err(self.error("expected comma or closing brace in JSON object")),
            }
            self.position += 1;
            self.skip_whitespace();
            if self.at(self.position) == Some('}') {
                return Err(self.error("trailing comma in JSON object"));
            }
        }
    }
}

#[derive(Debug)]
struct Outcome {
    root: Option<Rc<Node>>,
    offset: i64,
    line: i64,
    column: i64,
    message: String,
    duplicate: Option<String>,
}

#[derive(Debug)]
struct ParseState {
    source: Vec<char>,
    maximum_depth: IntValue,
    outcome: RefCell<Option<Rc<Outcome>>>,
}

fn parsed(value: &Value) -> Result<Rc<Outcome>, Error> {
    let state = handle::<ParseState>(value)?;
    if let Some(outcome) = state.outcome.borrow().clone() {
        return Ok(outcome);
    }
    let mut parser = Parser {
        source: &state.source,
        maximum_depth: &state.maximum_depth,
        position: 0,
    };
    let result = parser.value(1).and_then(|root| {
        parser.skip_whitespace();
        if parser.position != state.source.len() {
            return Err(parser.error("trailing input after JSON value"));
        }
        Ok(root)
    });
    let outcome = match result {
        Ok(root) => Outcome {
            root: Some(root),
            offset: 0,
            line: 0,
            column: 0,
            message: String::new(),
            duplicate: None,
        },
        Err(error) if error.message == "\u{0}overflow" => Outcome {
            root: None,
            offset: error.offset as i64,
            line: 0,
            column: 0,
            message: "maximum JSON depth exceeded".into(),
            duplicate: None,
        },
        Err(error) => {
            let (mut line, mut column, mut position) = (1, 1, 0);
            while position < error.offset {
                let character = state.source[position];
                if character == '\r' {
                    position += 1;
                    if position < error.offset && state.source[position] == '\n' {
                        position += 1;
                    }
                    line += 1;
                    column = 1;
                    continue;
                }
                if character == '\n' {
                    line += 1;
                    column = 1;
                } else {
                    column += 1;
                }
                position += 1;
            }
            Outcome {
                root: None,
                offset: error.offset as i64,
                line,
                column,
                message: error.message,
                duplicate: error.duplicate,
            }
        }
    };
    let outcome = Rc::new(outcome);
    *state.outcome.borrow_mut() = Some(outcome.clone());
    Ok(outcome)
}

fn node(value: &Value) -> Result<&Rc<Node>, Error> {
    handle::<Rc<Node>>(value)
}

fn node_value(node: Rc<Node>) -> Value {
    new_handle(node)
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    let name = native
        .symbol()
        .split("::")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("");
    let invalid = |message: &str| failure("core.json", "invalid_input", native, message);
    match name {
        "json_parse_state" => Ok(new_handle(ParseState {
            source: string(&args[0])?.chars().collect(),
            maximum_depth: int(&args[1])?.clone(),
            outcome: RefCell::new(None),
        })),
        "json_parse_valid" => Ok(Value::Bool(parsed(&args[0])?.root.is_some())),
        "json_parse_has_duplicate" => Ok(Value::Bool(parsed(&args[0])?.duplicate.is_some())),
        "json_parse_offset" => Ok(int_value(parsed(&args[0])?.offset)),
        "json_parse_line" => Ok(int_value(parsed(&args[0])?.line)),
        "json_parse_column" => Ok(int_value(parsed(&args[0])?.column)),
        "json_parse_message" => Ok(text(parsed(&args[0])?.message.clone())),
        "json_parse_duplicate" => Ok(text(parsed(&args[0])?.duplicate.clone().unwrap_or_default())),
        "json_parse_root" => match parsed(&args[0])?.root.clone() {
            Some(root) => Ok(node_value(root)),
            None => Err(failure(
                "core.json",
                "invalid_data",
                native,
                "invalid JSON has no root",
            )),
        },
        "json_node_kind" => Ok(int_value(node(&args[0])?.kind())),
        "json_node_boolean" => match node(&args[0])?.as_ref() {
            Node::Boolean(flag) => Ok(Value::Bool(*flag)),
            _ => Err(invalid("JSON node is not boolean")),
        },
        "json_node_number" => match node(&args[0])?.as_ref() {
            Node::Number(value) => Ok(text(value.clone())),
            _ => Err(invalid("JSON node has the wrong kind")),
        },
        "json_node_text" => match node(&args[0])?.as_ref() {
            Node::Text(value) => Ok(text(value.clone())),
            _ => Err(invalid("JSON node has the wrong kind")),
        },
        "json_node_size" => match node(&args[0])?.as_ref() {
            Node::Array(values) | Node::Object(_, values) => Ok(int_value(values.len() as i64)),
            _ => Err(invalid("JSON node has no children")),
        },
        "json_node_item" => match node(&args[0])?.as_ref() {
            Node::Array(values) => {
                let at = super::support::small(int(&args[1])?, values.len())
                    .ok_or_else(|| invalid("JSON array index is outside the node"))?;
                Ok(node_value(values[at].clone()))
            }
            _ => Err(invalid("JSON node is not an array")),
        },
        "json_member_name" | "json_member_value" => match node(&args[0])?.as_ref() {
            Node::Object(names, values) => {
                let at = super::support::small(int(&args[1])?, values.len())
                    .ok_or_else(|| invalid("JSON member index is outside the node"))?;
                Ok(if name == "json_member_name" {
                    text(names[at].clone())
                } else {
                    node_value(values[at].clone())
                })
            }
            _ => Err(invalid("JSON node is not an object")),
        },
        _ => Err(Error::Unsupported(format!(
            "native provider is not implemented: {}",
            native.symbol()
        ))),
    }
}
