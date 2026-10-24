//! `runtime/toml` (TOML 1.0 в дерево узлов) и `runtime/html` (поток событий
//! HTML с именованными ссылками WHATWG) — перенос `MarkupNatives` JVM.
//! Смещения — в символах; тексты отказов — как у Perl-provider.
use super::Error;
use super::support::{array, failure, handle, int, int_value, new_handle, string, text};
use crate::{IntValue, Value, abi::Native};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

fn slice(points: &[char], from: usize, to: usize) -> String {
    if to <= from {
        return String::new();
    }
    points[from..to.min(points.len())].iter().collect()
}

// ===================== runtime/toml =====================

const INTEGER: i64 = 1;
const REAL: i64 = 2;
const TEXT: i64 = 3;
const BOOLEAN: i64 = 4;
const ARRAY: i64 = 5;
const TABLE: i64 = 6;
const DATE: i64 = 7;
const TIME: i64 = 8;
const LOCAL: i64 = 9;
const OFFSET: i64 = 10;
const INFINITY: i64 = 11;
const NAN: i64 = 12;

/// Узел TOML: таблицы и массивы изменяются при разборе, поэтому общие.
#[derive(Debug, Default)]
struct TomlNode {
    kind: i64,
    depth: i64,
    definition: String,
    sealed: bool,
    aot: bool,
    names: Vec<String>,
    members: Vec<NodeRef>,
    index: HashMap<String, NodeRef>,
    values: Vec<NodeRef>,
    text: String,
    real: f64,
    flag: bool,
    coordinates: Vec<i64>,
}

type NodeRef = Rc<RefCell<TomlNode>>;

#[derive(Clone, Debug)]
struct TomlError {
    offset: i64,
    line: i64,
    column: i64,
    message: String,
}

fn toml_error(source: &[char], offset: usize, message: &str) -> TomlError {
    let prefix: String = source[..offset.min(source.len())].iter().collect();
    let prefix: Vec<char> = prefix.replace("\r\n", "\n").chars().collect();
    let newlines = prefix.iter().filter(|c| **c == '\n').count() as i64;
    let last = prefix.iter().rposition(|c| *c == '\n').map_or(-1, |at| at as i64);
    TomlError {
        offset: offset as i64,
        line: 1 + newlines,
        column: prefix.len() as i64 - last,
        message: message.into(),
    }
}

type Parsed<T> = Result<T, TomlError>;

struct Parser {
    source: Vec<char>,
    position: usize,
    maximum_depth: IntValue,
    maximum_nodes: IntValue,
    nodes: i64,
}

fn digits_group(text: &str, digit: impl Fn(char) -> bool) -> bool {
    // d(?:_?d)*
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() || !digit(chars[0]) || !digit(*chars.last().expect("non-empty")) {
        return false;
    }
    let mut previous_underscore = false;
    for c in &chars {
        if *c == '_' {
            if previous_underscore {
                return false;
            }
            previous_underscore = true;
        } else if digit(*c) {
            previous_underscore = false;
        } else {
            return false;
        }
    }
    true
}

/// `[+-]?(?:0|[1-9](?:_?[0-9])*)`
fn decimal_integer(raw: &str) -> bool {
    let body = raw.strip_prefix(['+', '-']).unwrap_or(raw);
    body == "0" || (body.starts_with(|c: char| ('1'..='9').contains(&c)) && digits_group(body, |c| c.is_ascii_digit()))
}

/// `DECIMAL(?:\.[0-9](?:_?[0-9])*)?(?:[eE][+-]?[0-9](?:_?[0-9])*)?`
fn decimal_float(raw: &str) -> bool {
    let (mantissa, exponent) = match raw.find(['e', 'E']) {
        Some(at) => (&raw[..at], Some(&raw[at + 1..])),
        None => (raw, None),
    };
    let (whole, fraction) = match mantissa.find('.') {
        Some(at) => (&mantissa[..at], Some(&mantissa[at + 1..])),
        None => (mantissa, None),
    };
    decimal_integer(whole)
        && fraction.is_none_or(|fraction| digits_group(fraction, |c| c.is_ascii_digit()))
        && exponent.is_none_or(|exponent| {
            let exponent = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
            digits_group(exponent, |c| c.is_ascii_digit())
        })
}

fn number_digits(text: &str, count: usize) -> Option<i64> {
    (text.len() == count && text.bytes().all(|b| b.is_ascii_digit())).then(|| text.parse().expect("digits"))
}

impl Parser {
    fn error(&self, message: &str) -> TomlError {
        toml_error(&self.source, self.position, message)
    }

    fn error_at(&self, message: &str, offset: usize) -> TomlError {
        toml_error(&self.source, offset, message)
    }

    fn peek_text(&self, count: usize) -> String {
        slice(&self.source, self.position.min(self.source.len()), self.source.len().min(self.position + count))
    }

    fn at(&self, index: usize) -> Option<char> {
        self.source.get(index).copied()
    }

    fn peek(&self) -> Option<char> {
        self.at(self.position)
    }

    fn horizontal(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.position += 1;
        }
    }

    fn newline(&mut self) -> Parsed<bool> {
        if self.peek_text(2) == "\r\n" {
            self.position += 2;
            return Ok(true);
        }
        match self.peek() {
            Some('\n') => {
                self.position += 1;
                Ok(true)
            }
            Some('\r') => Err(self.error("bare carriage return is not a TOML newline")),
            _ => Ok(false),
        }
    }

    fn comment(&mut self) -> Parsed<()> {
        if self.peek() != Some('#') {
            return Ok(());
        }
        while let Some(code) = self.peek() {
            if code == '\n' || code == '\r' {
                break;
            }
            if ((code as u32) < 32 && code != '\t') || code as u32 == 127 {
                return Err(self.error("control character in comment"));
            }
            self.position += 1;
        }
        Ok(())
    }

    fn space(&mut self) -> Parsed<()> {
        loop {
            self.horizontal();
            self.comment()?;
            if !self.newline()? {
                return Ok(());
            }
        }
    }

    fn expect(&mut self, token: &str) -> Parsed<()> {
        if self.peek_text(token.chars().count()) != token {
            return Err(self.error(&format!("expected {token}")));
        }
        self.position += token.chars().count();
        Ok(())
    }

    fn node(&mut self, kind: i64, depth: i64) -> Parsed<NodeRef> {
        if crate::integer::compare(&IntValue::from_i64(depth), &self.maximum_depth).is_gt() {
            return Err(self.error("maximum TOML depth exceeded"));
        }
        if crate::integer::compare(&IntValue::from_i64(self.nodes), &self.maximum_nodes).is_ge() {
            return Err(self.error("maximum TOML nodes exceeded"));
        }
        self.nodes += 1;
        Ok(Rc::new(RefCell::new(TomlNode {
            kind,
            depth,
            ..TomlNode::default()
        })))
    }

    fn table(&mut self, depth: i64, definition: &str) -> Parsed<NodeRef> {
        let node = self.node(TABLE, depth)?;
        node.borrow_mut().definition = definition.into();
        Ok(node)
    }

    fn put(&self, table: &NodeRef, name: String, value: NodeRef) -> Parsed<()> {
        let mut table = table.borrow_mut();
        if table.index.contains_key(&name) {
            return Err(self.error("duplicate TOML key"));
        }
        if table.members.len() >= 1024 {
            return Err(self.error("maximum TOML table members exceeded"));
        }
        table.index.insert(name.clone(), value.clone());
        table.names.push(name);
        table.members.push(value);
        Ok(())
    }

    fn string(&mut self, key: bool) -> Parsed<String> {
        let quote = self.peek().expect("string starts with a quote");
        let triple: String = [quote; 3].iter().collect();
        let multiline = self.peek_text(3) == triple;
        if key && multiline {
            return Err(self.error("multiline key is not allowed"));
        }
        self.position += if multiline { 3 } else { 1 };
        if multiline {
            self.newline()?;
        }
        let mut result = String::new();
        while self.position < self.source.len() {
            let start = self.position;
            let c = self.source[self.position];
            if c == quote {
                if !multiline {
                    self.position += 1;
                    return Ok(result);
                }
                let mut count = 0;
                while self.at(start + count) == Some(quote) {
                    count += 1;
                }
                if count > 5 {
                    return Err(self.error("too many closing quotes"));
                }
                self.position += count;
                if count >= 3 {
                    for _ in 0..count - 3 {
                        result.push(quote);
                    }
                    return Ok(result);
                }
                for _ in 0..count {
                    result.push(quote);
                }
                continue;
            }
            if c == '\n' || c == '\r' {
                if !multiline {
                    return Err(self.error("newline in single-line string"));
                }
                self.newline()?;
                result.push('\n');
                continue;
            }
            if ((c as u32) < 32 && c != '\t') || c as u32 == 127 {
                return Err(self.error("control character in string"));
            }
            self.position += 1;
            if c != '\\' || quote == '\'' {
                result.push(c);
                continue;
            }
            if multiline {
                let saved = self.position;
                self.horizontal();
                if self.newline()? {
                    loop {
                        self.horizontal();
                        if !self.newline()? {
                            break;
                        }
                    }
                    continue;
                }
                self.position = saved;
            }
            let Some(escape) = self.at(self.position) else {
                return Err(self.error_at("unfinished string escape", start));
            };
            self.position += 1;
            let simple = match escape {
                'b' => Some('\u{08}'),
                't' => Some('\t'),
                'n' => Some('\n'),
                'f' => Some('\u{0c}'),
                'r' => Some('\r'),
                '"' => Some('"'),
                '\\' => Some('\\'),
                _ => None,
            };
            if let Some(simple) = simple {
                result.push(simple);
                continue;
            }
            if escape != 'u' && escape != 'U' {
                return Err(self.error_at("unknown string escape", start));
            }
            let length = if escape == 'u' { 4 } else { 8 };
            let hex = self.peek_text(length);
            if hex.chars().count() != length || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(self.error_at("invalid Unicode escape", start));
            }
            let point = u32::from_str_radix(&hex, 16).unwrap_or(u32::MAX);
            let Some(scalar) = char::from_u32(point) else {
                return Err(self.error_at("escape is not a Unicode scalar", start));
            };
            self.position += length;
            result.push(scalar);
        }
        Err(self.error("unterminated string"))
    }

    fn bare(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '_' || c == '-'
    }

    fn key(&mut self) -> Parsed<Vec<String>> {
        let mut parts = Vec::new();
        loop {
            self.horizontal();
            match self.peek() {
                Some('"' | '\'') => parts.push(self.string(true)?),
                _ => {
                    let start = self.position;
                    while self.peek().is_some_and(Self::bare) {
                        self.position += 1;
                    }
                    if start == self.position {
                        return Err(self.error("expected key"));
                    }
                    parts.push(slice(&self.source, start, self.position));
                }
            }
            self.horizontal();
            if self.peek() != Some('.') {
                return Ok(parts);
            }
            self.position += 1;
        }
    }

    fn descend(&mut self, table: &NodeRef, name: &str, definition: &str) -> Parsed<NodeRef> {
        if table.borrow().sealed {
            return Err(self.error("inline table cannot be extended"));
        }
        let existing = table.borrow().index.get(name).cloned();
        let mut node = match existing {
            Some(node) => node,
            None => {
                let depth = table.borrow().depth + 1;
                let node = self.table(depth, definition)?;
                self.put(table, name.to_string(), node.clone())?;
                node
            }
        };
        {
            let current = node.borrow();
            if definition == "dotted"
                && (current.kind == ARRAY || (current.kind == TABLE && current.definition == "explicit"))
            {
                return Err(self.error("dotted key cannot extend a header-defined table"));
            }
        }
        let last = {
            let current = node.borrow();
            (current.kind == ARRAY && current.aot).then(|| current.values.last().cloned())
        };
        if let Some(last) = last {
            node = last.expect("array of tables has an element");
        }
        let mut current = node.borrow_mut();
        if current.kind != TABLE || current.sealed {
            drop(current);
            return Err(self.error("key is not an extensible table"));
        }
        if definition == "dotted" && current.definition == "implicit" {
            current.definition = "dotted".into();
        }
        drop(current);
        Ok(node)
    }

    fn binding(&mut self, table: &NodeRef) -> Parsed<()> {
        let key_offset = self.position;
        let mut path = self.key()?;
        self.expect("=")?;
        self.horizontal();
        let name = path.pop().expect("key has a part");
        let mut table = table.clone();
        for part in &path {
            table = self.descend(&table, part, "dotted")?;
        }
        if table.borrow().index.contains_key(&name) {
            return Err(self.error_at("duplicate TOML key", key_offset));
        }
        if table.borrow().members.len() >= 1024 {
            return Err(self.error_at("maximum TOML table members exceeded", key_offset));
        }
        let depth = table.borrow().depth + 1;
        let value = self.value(depth)?;
        self.put(&table, name, value)
    }

    fn header(&mut self, root: &NodeRef) -> Parsed<NodeRef> {
        let is_array = self.peek_text(2) == "[[";
        self.position += if is_array { 2 } else { 1 };
        let mut path = self.key()?;
        self.expect(if is_array { "]]" } else { "]" })?;
        let name = path.pop().expect("key has a part");
        let mut parent = root.clone();
        for part in &path {
            parent = self.descend(&parent, part, "implicit")?;
        }
        let existing = parent.borrow().index.get(&name).cloned();
        if is_array {
            let node = match existing {
                Some(node) => node,
                None => {
                    let depth = parent.borrow().depth + 1;
                    let node = self.node(ARRAY, depth)?;
                    node.borrow_mut().aot = true;
                    self.put(&parent, name, node.clone())?;
                    node
                }
            };
            if node.borrow().kind != ARRAY || !node.borrow().aot {
                return Err(self.error("array-of-tables conflicts with existing value"));
            }
            if node.borrow().values.len() >= 10_000 {
                return Err(self.error("maximum TOML array items exceeded"));
            }
            let depth = node.borrow().depth + 1;
            let element = self.table(depth, "explicit")?;
            node.borrow_mut().values.push(element.clone());
            return Ok(element);
        }
        match existing {
            None => {
                let depth = parent.borrow().depth + 1;
                let node = self.table(depth, "explicit")?;
                self.put(&parent, name, node.clone())?;
                Ok(node)
            }
            Some(node) => {
                {
                    let current = node.borrow();
                    if !(current.kind == TABLE && !current.sealed && current.definition == "implicit") {
                        drop(current);
                        return Err(self.error("table redefinition or conflict"));
                    }
                }
                node.borrow_mut().definition = "explicit".into();
                Ok(node)
            }
        }
    }

    fn date_valid(y: i64, m: i64, d: i64) -> bool {
        if !(1..=12).contains(&m) || d < 1 {
            return false;
        }
        let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
        let days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        d <= days[(m - 1) as usize]
    }

    /// `HH:MM:SS(.f+)?` → часы, минуты, секунды, дробь.
    fn time_parts(text: &str) -> Option<(i64, i64, i64, String)> {
        let (main, fraction) = match text.split_once('.') {
            Some((main, fraction)) => {
                if fraction.is_empty() || !fraction.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                (main, fraction.to_string())
            }
            None => (text, String::new()),
        };
        let mut parts = main.split(':');
        let hour = number_digits(parts.next()?, 2)?;
        let minute = number_digits(parts.next()?, 2)?;
        let second = number_digits(parts.next()?, 2)?;
        parts.next().is_none().then_some((hour, minute, second, fraction))
    }

    fn temporal(&mut self, raw: &str, depth: i64, at: usize) -> Parsed<Option<NodeRef>> {
        if !raw.is_ascii() {
            return Ok(None);
        }
        let mut date = None;
        let mut time = None;
        let mut zone: Option<String> = None;
        // YYYY-MM-DD([Tt ]HH:MM:SS(.f)?(Z|±HH:MM)?)?
        let bytes = raw.as_bytes();
        let is_date = bytes.len() >= 10
            && bytes[4] == b'-'
            && bytes[7] == b'-'
            && number_digits(&raw[0..4], 4).is_some()
            && number_digits(&raw[5..7], 2).is_some()
            && number_digits(&raw[8..10], 2).is_some();
        if is_date {
            date = Some((
                raw[0..4].parse::<i64>().expect("digits"),
                raw[5..7].parse::<i64>().expect("digits"),
                raw[8..10].parse::<i64>().expect("digits"),
            ));
            let rest = &raw[10..];
            if !rest.is_empty() {
                let separator = rest.as_bytes()[0];
                if !matches!(separator, b'T' | b't' | b' ') || !rest.is_char_boundary(1) {
                    return Ok(None);
                }
                let rest = &rest[1..];
                let zone_at = rest
                    .char_indices()
                    .find(|(_, c)| matches!(c, 'Z' | 'z' | '+' | '-'))
                    .map(|(index, _)| index);
                let (clock, offset) = match zone_at {
                    Some(index) => (&rest[..index], Some(&rest[index..])),
                    None => (rest, None),
                };
                let Some(parts) = Self::time_parts(clock) else {
                    return Ok(None);
                };
                if let Some(offset) = offset {
                    let valid = offset.eq_ignore_ascii_case("z")
                        || (offset.len() == 6
                            && offset.as_bytes()[3] == b':'
                            && number_digits(&offset[1..3], 2).is_some()
                            && number_digits(&offset[4..6], 2).is_some());
                    if !valid {
                        return Ok(None);
                    }
                    zone = Some(offset.to_string());
                }
                time = Some(parts);
            }
        } else {
            let Some(parts) = Self::time_parts(raw) else {
                return Ok(None);
            };
            time = Some(parts);
        }
        if let Some((y, m, d)) = date {
            if !Self::date_valid(y, m, d) {
                return Err(self.error_at("invalid Gregorian date", at));
            }
        }
        let mut coordinates = Vec::new();
        if let Some((y, m, d)) = date {
            coordinates.extend([y, m, d]);
        }
        if let Some((hour, minute, second, fraction)) = &time {
            if *hour > 23 || *minute > 59 || *second > 60 {
                return Err(self.error_at("invalid local time", at));
            }
            let fraction: String = format!("{fraction}000000000").chars().take(9).collect();
            coordinates.extend([*hour, *minute, *second, fraction.parse::<i64>().expect("digits")]);
        }
        let node = if let Some(zone) = zone {
            let (mut minutes, mut known) = (0, 1);
            if !zone.eq_ignore_ascii_case("Z") {
                let sign = if zone.starts_with('-') { -1 } else { 1 };
                let hour: i64 = zone[1..3].parse().expect("digits");
                let minute: i64 = zone[4..6].parse().expect("digits");
                if hour > 23 || minute > 59 {
                    return Err(self.error_at("invalid fixed offset", at));
                }
                minutes = (hour * 60 + minute) * sign;
                if zone == "-00:00" {
                    known = 0;
                }
            }
            coordinates.extend([minutes, known]);
            self.node(OFFSET, depth)?
        } else {
            let kind = match (date.is_some(), time.is_some()) {
                (true, true) => LOCAL,
                (true, false) => DATE,
                _ => TIME,
            };
            self.node(kind, depth)?
        };
        node.borrow_mut().coordinates = coordinates;
        Ok(Some(node))
    }

    fn scalar(&mut self, depth: i64) -> Parsed<NodeRef> {
        let start = self.position;
        // `YYYY-MM-DD ` перед `HH:` — дата и время через пробел.
        let ahead = self.peek_text(13);
        let ahead: Vec<char> = ahead.chars().collect();
        if ahead.len() >= 13
            && ahead[..4].iter().all(char::is_ascii_digit)
            && ahead[4] == '-'
            && ahead[5..7].iter().all(char::is_ascii_digit)
            && ahead[7] == '-'
            && ahead[8..10].iter().all(char::is_ascii_digit)
            && ahead[10] == ' '
            && ahead[11..13].iter().all(char::is_ascii_digit)
            && self.at(self.position + 13) == Some(':')
        {
            self.position += 11;
        }
        while let Some(c) = self.peek() {
            if " \t\r\n,]}#".contains(c) {
                break;
            }
            self.position += 1;
        }
        let raw = slice(&self.source, start, self.position);
        if let Some(node) = self.temporal(&raw, depth, start)? {
            return Ok(node);
        }
        if raw == "true" || raw == "false" {
            let node = self.node(BOOLEAN, depth)?;
            node.borrow_mut().flag = raw == "true";
            return Ok(node);
        }
        if matches!(raw.as_str(), "inf" | "+inf" | "-inf") {
            let node = self.node(INFINITY, depth)?;
            node.borrow_mut().flag = raw.starts_with('-');
            return Ok(node);
        }
        if matches!(raw.as_str(), "nan" | "+nan" | "-nan") {
            return self.node(NAN, depth);
        }
        let clean = raw.replace('_', "");
        let integer = if decimal_integer(&raw) {
            crate::bignum::Big::parse(clean.strip_prefix('+').unwrap_or(&clean))
        } else if let Some(hex) = raw.strip_prefix("0x").filter(|hex| digits_group(hex, |c| c.is_ascii_hexdigit())) {
            radix(&hex.replace('_', ""), 16)
        } else if let Some(octal) = raw.strip_prefix("0o").filter(|octal| digits_group(octal, |c| ('0'..='7').contains(&c))) {
            radix(&octal.replace('_', ""), 8)
        } else if let Some(binary) = raw.strip_prefix("0b").filter(|binary| digits_group(binary, |c| c == '0' || c == '1')) {
            radix(&binary.replace('_', ""), 2)
        } else {
            None
        };
        if let Some(integer) = integer {
            let value = IntValue::from_big(integer);
            if value.to_i64().is_none() {
                return Err(self.error_at("integer outside signed 64-bit range", start));
            }
            let node = self.node(INTEGER, depth)?;
            node.borrow_mut().text = value.to_string();
            return Ok(node);
        }
        if decimal_float(&raw) && raw.contains(['.', 'e', 'E']) {
            let number: f64 = clean.parse().expect("validated float");
            if !number.is_finite() {
                return Err(self.error_at("floating-point overflow", start));
            }
            let node = self.node(REAL, depth)?;
            node.borrow_mut().real = number;
            return Ok(node);
        }
        Err(self.error_at("expected TOML value", start))
    }

    fn value(&mut self, depth: i64) -> Parsed<NodeRef> {
        match self.peek() {
            Some('"' | '\'') => {
                let node = self.node(TEXT, depth)?;
                let value = self.string(false)?;
                node.borrow_mut().text = value;
                Ok(node)
            }
            Some('[') => {
                let node = self.node(ARRAY, depth)?;
                self.position += 1;
                self.space()?;
                while self.peek() != Some(']') {
                    if node.borrow().values.len() >= 10_000 {
                        return Err(self.error("maximum TOML array items exceeded"));
                    }
                    let item = self.value(depth + 1)?;
                    node.borrow_mut().values.push(item);
                    self.space()?;
                    if self.peek() == Some(']') {
                        break;
                    }
                    self.expect(",")?;
                    self.space()?;
                }
                self.position += 1;
                Ok(node)
            }
            Some('{') => {
                let node = self.table(depth, "inline")?;
                self.position += 1;
                self.horizontal();
                if self.peek() != Some('}') {
                    loop {
                        self.binding(&node)?;
                        self.horizontal();
                        if self.peek() == Some('}') {
                            break;
                        }
                        self.expect(",")?;
                        self.horizontal();
                    }
                }
                self.position += 1;
                node.borrow_mut().sealed = true;
                Ok(node)
            }
            _ => self.scalar(depth),
        }
    }
}

fn radix(digits: &str, base: u32) -> Option<crate::bignum::Big> {
    let mut number = crate::bignum::Big::zero();
    let multiplier = crate::bignum::Big::from_u64(u64::from(base));
    for c in digits.chars() {
        number = number
            .multiply(&multiplier)
            .add(&crate::bignum::Big::from_u64(u64::from(c.to_digit(base)?)));
    }
    Some(number)
}

#[derive(Debug)]
struct TomlState {
    source: String,
    input: IntValue,
    depth: IntValue,
    nodes: IntValue,
    parsed: RefCell<Option<Result<NodeRef, TomlError>>>,
}

fn toml_parse(state: &TomlState) -> Result<NodeRef, TomlError> {
    let mut parser = Parser {
        source: state.source.chars().collect(),
        position: 0,
        maximum_depth: state.depth.clone(),
        maximum_nodes: state.nodes.clone(),
        nodes: 0,
    };
    let depth = state.depth.to_i64();
    if state.input.is_negative()
        || !depth.is_some_and(|depth| (1..=128).contains(&depth))
        || crate::integer::compare(&state.nodes, &IntValue::from_i64(1)).is_lt()
    {
        return Err(parser.error("invalid TOML budgets"));
    }
    if crate::integer::compare(&IntValue::from_u128(parser.source.len() as u128), &state.input).is_gt() {
        let at = state.input.to_i64().unwrap_or(0) as usize;
        return Err(parser.error_at("maximum TOML input exceeded", at));
    }
    let root = parser.table(1, "explicit")?;
    let mut current = root.clone();
    parser.space()?;
    while parser.position < parser.source.len() {
        if parser.peek() == Some('[') {
            current = parser.header(&root)?;
        } else {
            parser.binding(&current)?;
        }
        parser.horizontal();
        parser.comment()?;
        if parser.position == parser.source.len() {
            break;
        }
        if !parser.newline()? {
            return Err(parser.error("expected newline after statement"));
        }
        parser.space()?;
    }
    Ok(root)
}

fn toml_state(value: &Value) -> Result<Result<NodeRef, TomlError>, Error> {
    let state = handle::<TomlState>(value)?;
    if state.parsed.borrow().is_none() {
        let parsed = toml_parse(state);
        *state.parsed.borrow_mut() = Some(parsed);
    }
    Ok(state.parsed.borrow().clone().expect("parsed above"))
}

fn toml_node(value: &Value) -> Result<NodeRef, Error> {
    Ok(handle::<NodeRef>(value)?.clone())
}

fn toml_execute(native: Native, name: &str, args: &[Value]) -> Result<Value, Error> {
    let unsupported = |field: &str| {
        failure(
            "core.toml",
            "invalid_input",
            native,
            format!("TOML node does not support {field}"),
        )
    };
    match name {
        "toml_parse_state" => Ok(new_handle(TomlState {
            source: string(&args[0])?.to_string(),
            input: int(&args[1])?.clone(),
            depth: int(&args[2])?.clone(),
            nodes: int(&args[3])?.clone(),
            parsed: RefCell::new(None),
        })),
        "toml_parse_valid" => Ok(Value::Bool(toml_state(&args[0])?.is_ok())),
        "toml_parse_offset" | "toml_parse_line" | "toml_parse_column" => {
            Ok(int_value(match toml_state(&args[0])? {
                Ok(_) => 0,
                Err(error) => match name {
                    "toml_parse_offset" => error.offset,
                    "toml_parse_line" => error.line,
                    _ => error.column,
                },
            }))
        }
        "toml_parse_message" => Ok(text(match toml_state(&args[0])? {
            Ok(_) => String::new(),
            Err(error) => error.message,
        })),
        "toml_parse_root" => match toml_state(&args[0])? {
            Ok(root) => Ok(new_handle(root)),
            Err(_) => Err(failure(
                "core.toml",
                "invalid_data",
                native,
                "invalid TOML has no root",
            )),
        },
        "toml_node_kind" => Ok(int_value(toml_node(&args[0])?.borrow().kind)),
        "toml_node_size" => {
            let node = toml_node(&args[0])?;
            let node = node.borrow();
            match node.kind {
                TABLE => Ok(int_value(node.members.len() as i64)),
                ARRAY => Ok(int_value(node.values.len() as i64)),
                _ => Err(unsupported("size")),
            }
        }
        "toml_node_integer" => {
            let node = toml_node(&args[0])?;
            let node = node.borrow();
            if node.kind != INTEGER {
                return Err(unsupported("integer"));
            }
            Ok(Value::Int(IntValue::parse(&node.text).expect("canonical integer")))
        }
        "toml_node_real" => {
            let node = toml_node(&args[0])?;
            let node = node.borrow();
            if node.kind != REAL {
                return Err(unsupported("real"));
            }
            Ok(Value::Float(node.real))
        }
        "toml_node_boolean" => {
            let node = toml_node(&args[0])?;
            let node = node.borrow();
            if node.kind != BOOLEAN && node.kind != INFINITY {
                return Err(unsupported("boolean"));
            }
            Ok(Value::Bool(node.flag))
        }
        "toml_node_text" => {
            let node = toml_node(&args[0])?;
            let node = node.borrow();
            if node.kind != TEXT && node.kind != INTEGER {
                return Err(unsupported("text"));
            }
            Ok(text(node.text.clone()))
        }
        "toml_node_coordinates" => {
            let node = toml_node(&args[0])?;
            let node = node.borrow();
            if !(DATE..=OFFSET).contains(&node.kind) {
                return Err(unsupported("coordinates"));
            }
            Ok(array(node.coordinates.iter().map(|value| int_value(*value)).collect()))
        }
        "toml_node_item" | "toml_member_name" => {
            let node = toml_node(&args[0])?;
            let node = node.borrow();
            let table = node.kind == TABLE;
            if !table && !(node.kind == ARRAY && name == "toml_node_item") {
                return Err(failure(
                    "core.toml",
                    "invalid_input",
                    native,
                    "TOML node has no such children",
                ));
            }
            let size = if table { node.members.len() } else { node.values.len() };
            let at = super::support::small(int(&args[1])?, size).ok_or_else(|| {
                failure(
                    "core.toml",
                    "invalid_input",
                    native,
                    "TOML child index outside container",
                )
            })?;
            if name == "toml_member_name" {
                return Ok(text(node.names[at].clone()));
            }
            Ok(new_handle(if table {
                node.members[at].clone()
            } else {
                node.values[at].clone()
            }))
        }
        _ => Err(Error::Unsupported(format!(
            "native provider is not implemented: {}",
            native.symbol()
        ))),
    }
}

// ===================== runtime/html =====================

fn entities() -> &'static HashMap<&'static str, String> {
    static ENTITIES: std::sync::OnceLock<HashMap<&'static str, String>> = std::sync::OnceLock::new();
    ENTITIES.get_or_init(|| {
        crate::data::html_entities()
            .lines()
            .filter_map(|line| {
                let (name, points) = line.split_once('\t')?;
                let value = points
                    .split(' ')
                    .filter_map(|hex| u32::from_str_radix(hex, 16).ok().and_then(char::from_u32))
                    .collect();
                Some((name, value))
            })
            .collect()
    })
}

const NUMERIC_REPLACEMENT: [(u32, u32); 27] = [
    (0x80, 0x20AC), (0x82, 0x201A), (0x83, 0x0192), (0x84, 0x201E), (0x85, 0x2026),
    (0x86, 0x2020), (0x87, 0x2021), (0x88, 0x02C6), (0x89, 0x2030), (0x8A, 0x0160),
    (0x8B, 0x2039), (0x8C, 0x0152), (0x8E, 0x017D), (0x91, 0x2018), (0x92, 0x2019),
    (0x93, 0x201C), (0x94, 0x201D), (0x95, 0x2022), (0x96, 0x2013), (0x97, 0x2014),
    (0x98, 0x02DC), (0x99, 0x2122), (0x9A, 0x0161), (0x9B, 0x203A), (0x9C, 0x0153),
    (0x9E, 0x017E), (0x9F, 0x0178),
];

fn numeric_character(digits: &str, base: u32) -> String {
    let digits = digits.trim_start_matches('0');
    let value = if digits.len() > 8 {
        None
    } else {
        u32::from_str_radix(if digits.is_empty() { "0" } else { digits }, base).ok()
    };
    let Some(mut point) = value.filter(|value| *value != 0 && *value <= 0x10FFFF && !(0xD800..=0xDFFF).contains(value)) else {
        return "\u{FFFD}".into();
    };
    for (from, to) in NUMERIC_REPLACEMENT {
        if from == point {
            point = to;
        }
    }
    char::from_u32(point).expect("scalar").to_string()
}

/// Ссылка `&…;` в позиции: значение и длина.
fn reference_at(source: &[char], offset: usize, attribute: bool) -> Option<(String, usize)> {
    let length = source.len();
    if offset >= length || source[offset] != '&' {
        return None;
    }
    let following = offset + 1;
    if following >= length {
        return None;
    }
    if source[following] == '#' {
        let mut position = following + 1;
        let mut base = 10;
        if position < length && (source[position] == 'x' || source[position] == 'X') {
            base = 16;
            position += 1;
        }
        let start = position;
        while position < length
            && if base == 16 {
                source[position].is_ascii_hexdigit()
            } else {
                source[position].is_ascii_digit()
            }
        {
            position += 1;
        }
        if position == start {
            return None;
        }
        let digits = slice(source, start, position);
        if position < length && source[position] == ';' {
            position += 1;
        }
        return Some((numeric_character(&digits, base), position - offset));
    }
    let maximum = (length - following).min(32);
    let table = entities();
    for amount in (1..=maximum).rev() {
        let name = slice(source, following, following + amount);
        let Some(value) = table.get(name.as_str()) else {
            continue;
        };
        if attribute && !name.ends_with(';') {
            let after = following + amount;
            if after < length && (source[after].is_ascii_alphanumeric() || source[after] == '=') {
                continue;
            }
        }
        return Some((value.clone(), amount + 1));
    }
    None
}

fn decode_references(value: &str, attribute: bool) -> String {
    let source: Vec<char> = value.chars().collect();
    let mut result = String::new();
    let mut offset = 0;
    while offset < source.len() {
        if source[offset] == '&' {
            if let Some((decoded, length)) = reference_at(&source, offset, attribute) {
                result.push_str(&decoded);
                offset += length;
                continue;
            }
        }
        result.push(source[offset]);
        offset += 1;
    }
    result
}

#[derive(Debug, Default)]
struct Event {
    kind: i64,
    name: Option<String>,
    value: Option<String>,
    target: Option<String>,
    data: Option<String>,
    self_closing: Option<bool>,
    attributes: Vec<(String, String)>,
    valued: Vec<bool>,
}

fn push_text(events: &mut Vec<Event>, value: &str, raw: bool) {
    if value.is_empty() {
        return;
    }
    let decoded = if raw {
        value.to_string()
    } else {
        decode_references(value, false)
    };
    if let Some(last) = events.last_mut().filter(|last| last.kind == 2) {
        last.value.get_or_insert_with(String::new).push_str(&decoded);
    } else {
        events.push(Event {
            kind: 2,
            value: Some(decoded),
            ..Event::default()
        });
    }
}

fn tag_end(source: &[char], offset: usize) -> Option<usize> {
    let mut quote = None;
    for (position, c) in source.iter().enumerate().skip(offset) {
        match quote {
            Some(open) if *c == open => quote = None,
            Some(_) => {}
            None if *c == '"' || *c == '\'' => quote = Some(*c),
            None if *c == '>' => return Some(position),
            None => {}
        }
    }
    None
}

fn lower(value: &str) -> String {
    value.chars().map(|c| c.to_ascii_lowercase()).collect()
}

fn attributes(body: &[char], mut position: usize, event: &mut Event) {
    let length = body.len();
    while position < length {
        while position < length && body[position].is_whitespace() {
            position += 1;
        }
        if position >= length {
            break;
        }
        let start = position;
        while position < length && !body[position].is_whitespace() && body[position] != '=' {
            position += 1;
        }
        let name = slice(body, start, position);
        if name.is_empty() {
            break;
        }
        let name = lower(&name);
        while position < length && body[position].is_whitespace() {
            position += 1;
        }
        let mut valued = false;
        let mut value = String::new();
        if position < length && body[position] == '=' {
            valued = true;
            position += 1;
            while position < length && body[position].is_whitespace() {
                position += 1;
            }
            if position < length && (body[position] == '"' || body[position] == '\'') {
                let quote = body[position];
                position += 1;
                let value_start = position;
                while position < length && body[position] != quote {
                    position += 1;
                }
                value = slice(body, value_start, position);
                if position < length {
                    position += 1;
                }
            } else {
                let value_start = position;
                while position < length && !body[position].is_whitespace() {
                    position += 1;
                }
                value = slice(body, value_start, position);
            }
            value = decode_references(&value, true);
        }
        event.attributes.push((name, value));
        event.valued.push(valued);
    }
}

/// `[A-Za-z][^\s/>]*` с начала.
fn tag_name(body: &[char], from: usize) -> Option<usize> {
    if !body.get(from).is_some_and(char::is_ascii_alphabetic) {
        return None;
    }
    let mut end = from + 1;
    while end < body.len() && !body[end].is_whitespace() && body[end] != '/' && body[end] != '>' {
        end += 1;
    }
    Some(end)
}

fn start_tag(source: &[char], offset: usize) -> Option<(Event, usize)> {
    if source[offset] != '<' {
        return None;
    }
    let end = tag_end(source, offset + 1)?;
    let mut body: Vec<char> = source[offset + 1..end].to_vec();
    let name_end = tag_name(&body, 0)?;
    let name = lower(&slice(&body, 0, name_end));
    // `/\s*\z`: последний `/`, за которым только пробелы.
    let trimmed = body.iter().rposition(|c| !c.is_whitespace());
    let self_closing = trimmed.is_some_and(|at| body[at] == '/');
    if self_closing {
        body.truncate(trimmed.expect("checked"));
    }
    let mut event = Event {
        kind: 0,
        name: Some(name),
        self_closing: Some(self_closing),
        ..Event::default()
    };
    attributes(&body, name_end, &mut event);
    Some((event, end + 1))
}

fn index_of(source: &[char], needle: &str, from: usize) -> Option<usize> {
    let needle: Vec<char> = needle.chars().collect();
    let mut at = from;
    while at + needle.len() <= source.len() {
        if source[at..at + needle.len()] == needle[..] {
            return Some(at);
        }
        at += 1;
    }
    None
}

fn end_tag(source: &[char], offset: usize) -> Option<(Event, usize)> {
    if slice(source, offset, source.len().min(offset + 2)) != "</" {
        return None;
    }
    let end = index_of(source, ">", offset + 2)?;
    let body = &source[offset + 2..end];
    let start = body.iter().position(|c| !c.is_whitespace()).unwrap_or(body.len());
    let name_end = tag_name(body, start)?;
    Some((
        Event {
            kind: 1,
            name: Some(lower(&slice(body, start, name_end))),
            ..Event::default()
        },
        end + 1,
    ))
}

fn same_letter(a: char, b: char) -> bool {
    a == b
        || a.to_uppercase().eq(b.to_uppercase())
        || a.to_lowercase().eq(b.to_lowercase())
}

/// `</\s*name(?=[\s/>])` без учёта регистра: смещение начала.
fn raw_close(tail: &[char], name: &str) -> Option<usize> {
    let name: Vec<char> = name.chars().collect();
    (0..tail.len()).find(|start| {
        if tail.get(*start) != Some(&'<') || tail.get(start + 1) != Some(&'/') {
            return false;
        }
        let mut at = start + 2;
        while at < tail.len() && tail[at].is_whitespace() {
            at += 1;
        }
        if at + name.len() > tail.len() || !tail[at..at + name.len()].iter().zip(&name).all(|(a, b)| same_letter(*a, *b)) {
            return false;
        }
        tail.get(at + name.len())
            .is_some_and(|c| c.is_whitespace() || *c == '/' || *c == '>')
    })
}

fn tokenize(origin: &str) -> Vec<Event> {
    let source: Vec<char> = origin.chars().collect();
    let mut events = Vec::new();
    let mut offset = 0;
    let mut raw_name: Option<String> = None;
    while offset < source.len() {
        if let Some(name) = raw_name.take() {
            let tail = &source[offset..];
            match raw_close(tail, &name) {
                Some(position) => {
                    push_text(&mut events, &slice(&source, offset, offset + position), true);
                    offset += position;
                    continue;
                }
                None => {
                    push_text(&mut events, &slice(&source, offset, source.len()), true);
                    break;
                }
            }
        }
        let four = slice(&source, offset, source.len().min(offset + 4));
        let two = slice(&source, offset, source.len().min(offset + 2));
        if four == "<!--" {
            match index_of(&source, "-->", offset + 4) {
                None => {
                    events.push(Event {
                        kind: 3,
                        value: Some(slice(&source, offset + 4, source.len())),
                        ..Event::default()
                    });
                    break;
                }
                Some(end) => {
                    events.push(Event {
                        kind: 3,
                        value: Some(slice(&source, offset + 4, end)),
                        ..Event::default()
                    });
                    offset = end + 3;
                    continue;
                }
            }
        }
        if two == "<?" {
            let (end, ending) = match index_of(&source, "?>", offset + 2) {
                Some(end) => (end, 2),
                None => match index_of(&source, ">", offset + 2) {
                    Some(end) => (end, 1),
                    None => (source.len(), 0),
                },
            };
            let body = slice(&source, offset + 2, end).trim().to_string();
            if body.is_empty() {
                push_text(&mut events, &slice(&source, offset, end + ending), false);
            } else {
                let split = body.find(char::is_whitespace);
                let (target, data) = match split {
                    Some(at) => (&body[..at], body[at..].trim_start()),
                    None => (body.as_str(), ""),
                };
                events.push(Event {
                    kind: 5,
                    target: Some(target.to_string()),
                    data: Some(data.to_string()),
                    ..Event::default()
                });
            }
            offset = end + ending;
            continue;
        }
        if two == "<!" {
            match index_of(&source, ">", offset + 2) {
                None => {
                    events.push(Event {
                        kind: 4,
                        value: Some(slice(&source, offset + 2, source.len()).trim().to_string()),
                        ..Event::default()
                    });
                    break;
                }
                Some(end) => {
                    events.push(Event {
                        kind: 4,
                        value: Some(slice(&source, offset + 2, end).trim().to_string()),
                        ..Event::default()
                    });
                    offset = end + 1;
                    continue;
                }
            }
        }
        if two == "</" {
            if let Some((event, next)) = end_tag(&source, offset) {
                events.push(event);
                offset = next;
                continue;
            }
        }
        if source[offset] == '<' {
            if let Some((event, next)) = start_tag(&source, offset) {
                let name = event.name.clone().expect("start tag has a name");
                let raw = event.self_closing == Some(false) && (name == "script" || name == "style");
                events.push(event);
                offset = next;
                if raw {
                    raw_name = Some(name);
                }
                continue;
            }
        }
        let following = index_of(&source, "<", offset + 1).unwrap_or(source.len());
        push_text(&mut events, &slice(&source, offset, following), false);
        offset = following;
    }
    events
}

#[derive(Debug)]
struct Events(Vec<Event>);

fn html_execute(native: Native, name: &str, args: &[Value]) -> Result<Value, Error> {
    if name == "html_events" {
        return Ok(new_handle(Events(tokenize(string(&args[0])?))));
    }
    let events = &handle::<Events>(&args[0])?.0;
    if name == "html_event_count" {
        return Ok(int_value(events.len() as i64));
    }
    let event = super::support::small(int(&args[1])?, events.len())
        .map(|at| &events[at])
        .ok_or_else(|| {
            Error::Failure(crate::abi::NativeFailure {
                domain: "core.html".into(),
                code: "invalid_input".into(),
                operation: "runtime/html@1::html_event/2".into(),
                message: "HTML event index is outside the stream".into(),
            })
        })?;
    let missing = |field: &str| {
        failure(
            "core.html",
            "invalid_input",
            native,
            format!("HTML event has no {field}"),
        )
    };
    let field = |value: &Option<String>, label: &str| value.clone().map(text).ok_or_else(|| missing(label));
    match name {
        "html_event_kind" => Ok(int_value(event.kind)),
        "html_event_name" => field(&event.name, "name"),
        "html_event_value" => field(&event.value, "value"),
        "html_event_target" => field(&event.target, "target"),
        "html_event_data" => field(&event.data, "data"),
        "html_event_self_closing" => event
            .self_closing
            .map(Value::Bool)
            .ok_or_else(|| missing("self_closing")),
        "html_attribute_count" | "html_attribute_name" | "html_attribute_valued"
        | "html_attribute_value" => {
            if event.kind != 0 {
                return Err(failure(
                    "core.html",
                    "invalid_input",
                    native,
                    "HTML event has no attributes",
                ));
            }
            if name == "html_attribute_count" {
                return Ok(int_value(event.attributes.len() as i64));
            }
            let at = super::support::small(int(&args[2])?, event.attributes.len()).ok_or_else(|| {
                failure(
                    "core.html",
                    "invalid_input",
                    native,
                    "HTML attribute index is outside the tag",
                )
            })?;
            Ok(match name {
                "html_attribute_name" => text(event.attributes[at].0.clone()),
                "html_attribute_valued" => Value::Bool(event.valued[at]),
                _ => text(event.attributes[at].1.clone()),
            })
        }
        _ => Err(Error::Unsupported(format!(
            "native provider is not implemented: {}",
            native.symbol()
        ))),
    }
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    let name = native
        .symbol()
        .split("::")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("");
    if native.symbol().starts_with("runtime/toml@") {
        toml_execute(native, name, args)
    } else {
        html_execute(native, name, args)
    }
}
