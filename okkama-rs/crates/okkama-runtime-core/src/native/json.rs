//! `runtime/json`: строгий разбор JSON (RFC 8259) в дерево узлов — перенос
//! `JsonNatives` JVM. Смещения, строки и столбцы — в символах (code points),
//! тексты отказов — как у Perl-provider.
use super::Error;
use super::support::{failure, handle, int, int_value, items, new_handle, string, text};
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

/// JSON text being written: appends go to the end, and every append answers
/// the UTF-8 size of the whole text so far.
struct Output(RefCell<String>);

impl Output {
    fn appended(&self, append: impl FnOnce(&mut String)) -> Value {
        let mut text = self.0.borrow_mut();
        append(&mut text);
        int_value(text.len() as i64)
    }
}

/// Appends the JSON string literal of `origin` (see [`quote`]).
fn push_quoted(quoted: &mut String, origin: &str) {
    quoted.push('"');
    for character in origin.chars() {
        match character {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                quoted.push_str(&format!("\\u{:04x}", control as u32));
            }
            other => quoted.push(other),
        }
    }
    quoted.push('"');
}

/// A JSON string literal: `"` and `\` escaped, `\n`, `\r`, `\t` by name, the
/// other controls U+0000..U+001F as lowercase `\u00xx`, everything else as is.
fn quote(origin: &str) -> String {
    let mut quoted = String::with_capacity(origin.len() + 2);
    push_quoted(&mut quoted, origin);
    quoted
}

/// The JSON spelling of an exact decimal (as `json/write.JsonNumberSpelling`):
/// positional while that stays short — integers up to 21 digits, fractions
/// down to 1e-6, the thresholds of JavaScript's `Number#toString` — and the
/// canonical scientific form beyond them, so the value is never expanded.
fn number_text(parts: &super::numbers::Decimal) -> String {
    let sign = if parts.sign < 0 { "-" } else { "" };
    let digits = parts.digits.as_str();
    let length = digits.len() as i64;
    if parts.exponent >= 0 {
        if length + parts.exponent <= 21 {
            return format!("{sign}{digits}{}", "0".repeat(parts.exponent as usize));
        }
        return super::numbers::canonical(parts);
    }
    let point = length + parts.exponent;
    if point > 0 {
        let (whole, fraction) = digits.split_at(point as usize);
        format!("{sign}{whole}.{fraction}")
    } else if -point <= 5 {
        format!("{sign}0.{}{digits}", "0".repeat((-point) as usize))
    } else {
        super::numbers::canonical(parts)
    }
}

/// The member `name` of the object node `object`; `None` when the node is
/// not an object or has no such member.
fn member<'a>(object: &'a Value, name: &Value) -> Result<Option<&'a Node>, Error> {
    let name = string(name)?;
    Ok(match node(object)?.as_ref() {
        Node::Object(names, values) => names
            .iter()
            .position(|candidate| candidate == name)
            .map(|at| values[at].as_ref()),
        _ => None,
    })
}

fn node(value: &Value) -> Result<&Rc<Node>, Error> {
    handle::<Rc<Node>>(value)
}

fn node_value(node: Rc<Node>) -> Value {
    new_handle(node)
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    let invalid = |message: &str| failure("core.json", "invalid_input", native, message);
    match native {
        Native::JsonJsonParseState => Ok(new_handle(ParseState {
            source: string(&args[0])?.chars().collect(),
            maximum_depth: int(&args[1])?.clone(),
            outcome: RefCell::new(None),
        })),
        Native::JsonJsonParseValid => Ok(Value::Bool(parsed(&args[0])?.root.is_some())),
        Native::JsonJsonParseHasDuplicate => Ok(Value::Bool(parsed(&args[0])?.duplicate.is_some())),
        Native::JsonJsonParseOffset => Ok(int_value(parsed(&args[0])?.offset)),
        Native::JsonJsonParseLine => Ok(int_value(parsed(&args[0])?.line)),
        Native::JsonJsonParseColumn => Ok(int_value(parsed(&args[0])?.column)),
        Native::JsonJsonParseMessage => Ok(text(parsed(&args[0])?.message.clone())),
        Native::JsonJsonParseDuplicate => Ok(text(parsed(&args[0])?.duplicate.clone().unwrap_or_default())),
        Native::JsonJsonParseRoot => match parsed(&args[0])?.root.clone() {
            Some(root) => Ok(node_value(root)),
            None => Err(failure(
                "core.json",
                "invalid_data",
                native,
                "invalid JSON has no root",
            )),
        },
        Native::JsonJsonNodeKind => Ok(int_value(node(&args[0])?.kind())),
        Native::JsonJsonNodeBoolean => match node(&args[0])?.as_ref() {
            Node::Boolean(flag) => Ok(Value::Bool(*flag)),
            _ => Err(invalid("JSON node is not boolean")),
        },
        Native::JsonJsonNodeNumber => match node(&args[0])?.as_ref() {
            Node::Number(value) => Ok(text(value.clone())),
            _ => Err(invalid("JSON node has the wrong kind")),
        },
        Native::JsonJsonNodeText => match node(&args[0])?.as_ref() {
            Node::Text(value) => Ok(text(value.clone())),
            _ => Err(invalid("JSON node has the wrong kind")),
        },
        Native::JsonJsonNodeSize => match node(&args[0])?.as_ref() {
            Node::Array(values) | Node::Object(_, values) => Ok(int_value(values.len() as i64)),
            _ => Err(invalid("JSON node has no children")),
        },
        Native::JsonJsonNodeItem => match node(&args[0])?.as_ref() {
            Node::Array(values) => {
                let at = super::support::small(int(&args[1])?, values.len())
                    .ok_or_else(|| invalid("JSON array index is outside the node"))?;
                Ok(node_value(values[at].clone()))
            }
            _ => Err(invalid("JSON node is not an array")),
        },
        Native::JsonJsonMemberName | Native::JsonJsonMemberValue => match node(&args[0])?.as_ref() {
            Node::Object(names, values) => {
                let at = super::support::small(int(&args[1])?, values.len())
                    .ok_or_else(|| invalid("JSON member index is outside the node"))?;
                Ok(if native == Native::JsonJsonMemberName {
                    text(names[at].clone())
                } else {
                    node_value(values[at].clone())
                })
            }
            _ => Err(invalid("JSON node is not an object")),
        },
        Native::JsonJsonMemberPosition => match node(&args[0])?.as_ref() {
            Node::Object(names, _) => {
                let sought = string(&args[1])?;
                Ok(int_value(names.iter().position(|name| name == sought).map_or(-1, |at| at as i64)))
            }
            _ => Err(invalid("JSON node is not an object")),
        },
        Native::JsonJsonQuote => Ok(text(quote(string(&args[0])?))),
        Native::JsonJsonNumberText => super::numbers::components(string(&args[0])?)
            .map(|parts| text(number_text(&parts)))
            .ok_or_else(|| invalid("not a decimal number")),
        Native::JsonJsonOutput => Ok(new_handle(Output(RefCell::new(String::new())))),
        Native::JsonJsonOutputRaw => {
            let raw = string(&args[1])?;
            Ok(handle::<Output>(&args[0])?.appended(|text| text.push_str(raw)))
        }
        Native::JsonJsonOutputText => {
            let origin = string(&args[1])?;
            Ok(handle::<Output>(&args[0])?.appended(|text| push_quoted(text, origin)))
        }
        Native::JsonJsonOutputNumber => {
            let parts = super::numbers::components(string(&args[1])?)
                .ok_or_else(|| invalid("not a decimal number"))?;
            Ok(handle::<Output>(&args[0])?.appended(|text| text.push_str(&number_text(&parts))))
        }
        Native::JsonJsonOutputMember => {
            let name = string(&args[1])?;
            let first = matches!(args[2], Value::Bool(true));
            Ok(handle::<Output>(&args[0])?.appended(|text| {
                if !first {
                    text.push(',');
                }
                push_quoted(text, name);
                text.push(':');
            }))
        }
        Native::JsonJsonOutputString => Ok(text(handle::<Output>(&args[0])?.0.borrow().clone())),
        Native::JsonJsonMemberText => match member(&args[0], &args[1])? {
            Some(Node::Text(value)) => Ok(text(value.clone())),
            _ => Err(invalid("JSON member is missing or not a string")),
        },
        Native::JsonJsonMemberSpelling => match member(&args[0], &args[1])? {
            Some(Node::Number(value)) => Ok(text(value.clone())),
            _ => Err(invalid("JSON member is missing or not a number")),
        },
        Native::JsonJsonMemberBoolean => match member(&args[0], &args[1])? {
            Some(Node::Boolean(flag)) => Ok(Value::Bool(*flag)),
            _ => Err(invalid("JSON member is missing or not boolean")),
        },
        // Only a plain integer spelling of up to 18 digits: always within int.
        Native::JsonJsonMemberInt => match member(&args[0], &args[1])? {
            Some(Node::Number(value))
                if value.trim_start_matches('-').len() <= 18
                    && !value.trim_start_matches('-').is_empty()
                    && value.trim_start_matches('-').bytes().all(|byte| byte.is_ascii_digit()) =>
            {
                Ok(int_value(value.parse::<i64>().map_err(|_| invalid("JSON integer"))?))
            }
            _ => Err(invalid("JSON member is missing or not a short integer")),
        },
        Native::JsonJsonRepeatedName => {
            let names = items(&args[0])?;
            let mut seen = std::collections::HashSet::with_capacity(names.len());
            for (index, name) in names.iter().enumerate() {
                if !seen.insert(name.as_string()?) {
                    return Ok(int_value(index as i64));
                }
            }
            Ok(int_value(names.len() as i64))
        }
        _ => Err(Error::Unsupported(format!(
            "native provider is not implemented: {}",
            native.symbol()
        ))),
    }
}
