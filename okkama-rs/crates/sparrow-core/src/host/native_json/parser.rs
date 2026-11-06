//! Iterative JSON parser. Completed nodes are emitted in postorder; child
//! indices are resolved into the native arena only after successful parsing.
use super::Node;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use std::{collections::HashSet, rc::Rc};
#[derive(Debug)]
pub(super) struct ParseError {
    pub offset: usize,
    pub message: String,
    pub duplicate: Option<String>,
}
type Parsed<T> = Result<T, ParseError>;
struct Parser<'a> {
    source: &'a [char],
    position: usize,
    scratch: String,
}
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

    fn string(&mut self) -> Parsed<Rc<str>> {
        let mut result = std::mem::take(&mut self.scratch);
        result.clear();
        let parsed = self.string_into(&mut result);
        self.scratch = result;
        parsed.map(|()| Rc::from(self.scratch.as_str()))
    }

    fn string_into(&mut self, result: &mut String) -> Parsed<()> {
        if self.at(self.position) != Some('"') {
            return Err(self.error("expected JSON string"));
        }
        self.position += 1;
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
                return Ok(());
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
        let written = BigInt::parse_bytes(written.as_bytes(), 10).expect("exponent digits");
        let mut exponent = written - BigInt::from(fraction_end - fraction_start);
        let digits = digits.trim_start_matches('0');
        if !digits.is_empty() {
            let zeros = digits.len() - digits.trim_end_matches('0').len();
            exponent += BigInt::from(zeros);
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

    fn literal(&mut self, word: &str) -> bool {
        let end = self.position + word.len();
        if end > self.source.len()
            || !self.source[self.position..end]
                .iter()
                .copied()
                .eq(word.chars())
        {
            return false;
        }
        self.position = end;
        true
    }
    fn member_name(&mut self, names: &mut Vec<Rc<str>>, seen: &mut HashSet<Rc<str>>) -> Parsed<()> {
        let offset = self.position;
        let name = self.string()?;
        if !seen.insert(name.clone()) {
            return Err(ParseError {
                offset,
                message: format!("duplicate JSON member \"{name}\""),
                duplicate: Some(name.to_string()),
            });
        }
        self.skip_whitespace();
        if self.at(self.position) != Some(':') {
            return Err(self.error("expected colon after JSON member name"));
        }
        self.position += 1;
        names.push(name);
        Ok(())
    }
}
enum Frame {
    Array(Vec<u64>),
    Object {
        names: Vec<Rc<str>>,
        values: Vec<u64>,
        seen: HashSet<Rc<str>>,
    },
}
pub(super) fn parse(source: &[char], maximum_depth: &BigInt) -> Parsed<(Vec<Node>, u64)> {
    let mut p = Parser {
        source,
        position: 0,
        scratch: String::new(),
    };
    let mut nodes = Vec::new();
    let mut stack = Vec::new();
    'value: loop {
        p.skip_whitespace();
        let depth = stack.len() + 1;
        if &BigInt::from(depth) > maximum_depth {
            return Err(p.error("maximum JSON depth exceeded"));
        }
        if depth > 100_000 {
            return Err(p.error("\u{0}overflow"));
        }
        let Some(ch) = p.at(p.position) else {
            return Err(p.error("expected JSON value"));
        };
        let node = match ch {
            '"' => Node::Text(p.string()?),
            '[' => {
                p.position += 1;
                p.skip_whitespace();
                if p.at(p.position) == Some(']') {
                    p.position += 1;
                    Node::Array(vec![])
                } else {
                    stack.push(Frame::Array(vec![]));
                    continue 'value;
                }
            }
            '{' => {
                p.position += 1;
                p.skip_whitespace();
                if p.at(p.position) == Some('}') {
                    p.position += 1;
                    Node::Object(vec![], vec![])
                } else {
                    let mut names = Vec::new();
                    let mut seen = HashSet::new();
                    p.member_name(&mut names, &mut seen)?;
                    stack.push(Frame::Object {
                        names,
                        values: Vec::new(),
                        seen,
                    });
                    continue 'value;
                }
            }
            '-' | '0'..='9' => Node::Number(p.number()?.into()),
            _ if p.literal("null") => Node::Null,
            _ if p.literal("false") => Node::Boolean(false),
            _ if p.literal("true") => Node::Boolean(true),
            _ => return Err(p.error("expected JSON value")),
        };
        let mut value = nodes.len() as u64;
        nodes.push(node);
        loop {
            p.skip_whitespace();
            let Some(parent) = stack.last_mut() else {
                if p.position != source.len() {
                    return Err(p.error("trailing input after JSON value"));
                }
                return Ok((nodes, value));
            };
            let closing = match parent {
                Frame::Array(values) => {
                    values.push(value);
                    ']'
                }
                Frame::Object { values, .. } => {
                    values.push(value);
                    '}'
                }
            };
            if p.at(p.position) == Some(closing) {
                p.position += 1;
                let node = match stack.pop().expect("parent") {
                    Frame::Array(values) => Node::Array(values),
                    Frame::Object { names, values, .. } => Node::Object(names, values),
                };
                value = nodes.len() as u64;
                nodes.push(node);
                continue;
            }
            if p.at(p.position) != Some(',') {
                return Err(p.error(if closing == ']' {
                    "expected comma or closing bracket in JSON array"
                } else {
                    "expected comma or closing brace in JSON object"
                }));
            }
            p.position += 1;
            p.skip_whitespace();
            if p.at(p.position) == Some(closing) {
                return Err(p.error(if closing == ']' {
                    "trailing comma in JSON array"
                } else {
                    "trailing comma in JSON object"
                }));
            }
            if let Frame::Object { names, seen, .. } = parent {
                p.member_name(names, seen)?;
            }
            continue 'value;
        }
    }
}
