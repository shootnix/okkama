//! Natives форматов данных — перенос `DataNatives` JVM: `runtime/io_decorators`,
//! `runtime/testing`, `runtime/csv_state`, `runtime/pem`, `runtime/template`,
//! `runtime/ini`. Смещения и длины — в символах (code points), тексты отказов
//! — как у Perl-provider. Строки Rust всегда скалярны, поэтому проверки
//! одиночных суррогатов JVM здесь не срабатывают никогда.
use super::Error;
use super::support::{
    bytes, failed, failure, handle, int, int_value, new_handle, string, string_array, text,
};
use crate::{IntValue, Value, abi::Native};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;

fn operation(native: Native) -> &'static str {
    native.name()
}

fn chars_of(text: &str) -> Vec<char> {
    text.chars().collect()
}

fn slice(points: &[char], from: usize, to: usize) -> String {
    points[from..to].iter().collect()
}

// ---- runtime/io_decorators ----

#[derive(Debug)]
struct Counter(RefCell<IntValue>);

// ---- runtime/testing ----

#[derive(Debug)]
struct TestState {
    limit: i64,
    size: Cell<i64>,
    closed: Cell<bool>,
}

fn require_test(native: Native) -> Result<(), Error> {
    if !crate::testing::active() {
        return Err(failure(
            "core.testing",
            "permission_denied",
            native,
            "Test state requires a running case",
        ));
    }
    Ok(())
}

fn test_state(native: Native, value: &Value) -> Result<&TestState, Error> {
    require_test(native)?;
    let state = handle::<TestState>(value)?;
    if state.closed.get() {
        return Err(failure(
            "core.testing",
            "already_closed",
            native,
            "Native handle is already closed",
        ));
    }
    Ok(state)
}

// ---- runtime/csv_state ----

#[derive(Debug)]
struct Csv {
    limits: [i64; 5],
    buffer: Vec<u8>,
    fed: i64,
    bytes: i64,
    scalars: i64,
    rows: i64,
    field: String,
    field_length: i64,
    fields: Vec<String>,
    mode: &'static str,
    active: bool,
    eof: bool,
    status: &'static str,
    offset: i64,
}

impl Csv {
    fn request(&self) -> i64 {
        let remaining = self.limits[0] - self.fed;
        if remaining >= 4096 {
            4096
        } else if remaining >= 0 {
            remaining + 1
        } else {
            1
        }
    }

    fn error(&mut self, kind: &'static str, at: Option<i64>) -> &'static str {
        self.status = kind;
        self.offset = at.unwrap_or(self.scalars);
        self.buffer.clear();
        self.field.clear();
        self.field_length = 0;
        self.fields.clear();
        kind
    }

    fn append(&mut self, point: char) -> bool {
        if self.field_length >= self.limits[2] {
            self.error("field", None);
            return false;
        }
        self.field.push(point);
        self.field_length += 1;
        true
    }

    fn finish_field(&mut self) -> bool {
        if self.fields.len() as i64 >= self.limits[4] {
            self.error("columns", None);
            return false;
        }
        self.fields.push(std::mem::take(&mut self.field));
        self.field_length = 0;
        true
    }

    fn record(&mut self) -> &'static str {
        if self.rows >= self.limits[3] {
            return self.error("rows", None);
        }
        self.rows += 1;
        self.active = false;
        self.mode = "start";
        self.status = "row";
        "row"
    }

    fn advance(&mut self) -> &'static str {
        if self.status != "need" {
            return self.status;
        }
        loop {
            if self.buffer.is_empty() {
                if !self.eof {
                    return "need";
                }
                if self.mode == "quoted" || self.mode == "cr" {
                    return self.error("syntax", None);
                }
                if self.active {
                    if !self.finish_field() {
                        return self.status;
                    }
                    return self.record();
                }
                self.status = "eof";
                return "eof";
            }
            if self.bytes >= self.limits[0] {
                return self.error("bytes", None);
            }
            let lead = self.buffer[0];
            let size = match lead {
                0x00..=0x7f => 1,
                0xc2..=0xdf => 2,
                0xe0..=0xef => 3,
                0xf0..=0xf4 => 4,
                _ => 0,
            };
            if size == 0 {
                return self.error("utf8", Some(self.bytes));
            }
            if self.buffer.len() < size {
                if self.fed > self.limits[0] {
                    return self.error("bytes", None);
                }
                if self.eof {
                    return self.error("utf8", Some(self.bytes));
                }
                return "need";
            }
            if size as i64 > self.limits[0] - self.bytes {
                return self.error("bytes", None);
            }
            let Some(point) = std::str::from_utf8(&self.buffer[..size])
                .ok()
                .and_then(|text| text.chars().next())
            else {
                return self.error("utf8", Some(self.bytes));
            };
            if self.scalars >= self.limits[1] {
                return self.error("input", None);
            }
            self.buffer.drain(..size);
            self.bytes += size as i64;
            if self.mode == "cr" {
                if point != '\n' {
                    return self.error("syntax", None);
                }
                self.scalars += 1;
                return self.record();
            }
            if self.mode == "quoted" {
                if point == '"' {
                    self.mode = "closed";
                } else if !self.append(point) {
                    return self.status;
                }
            } else if self.mode == "closed" && point == '"' {
                if !self.append(point) {
                    return self.status;
                }
                self.mode = "quoted";
            } else if point == ',' {
                if self.fields.len() as i64 + 1 >= self.limits[4] {
                    return self.error("columns", None);
                }
                if !self.finish_field() {
                    return self.status;
                }
                self.mode = "start";
                self.active = true;
            } else if point == '\r' {
                if !self.finish_field() {
                    return self.status;
                }
                self.mode = "cr";
            } else if point == '\n' || self.mode == "closed" {
                return self.error("syntax", None);
            } else if point == '"' {
                if self.mode != "start" {
                    return self.error("syntax", None);
                }
                self.mode = "quoted";
                self.active = true;
            } else {
                if !self.append(point) {
                    return self.status;
                }
                self.mode = "plain";
                self.active = true;
            }
            self.scalars += 1;
        }
    }
}

// ---- runtime/pem ----

const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn kind_invalid_offset(kind: &str) -> i64 {
    let text = chars_of(kind);
    if text.is_empty() {
        return 0;
    }
    let mut separator = true;
    for (offset, symbol) in text.iter().enumerate() {
        if *symbol == ' ' || *symbol == '-' {
            if separator || offset + 1 == text.len() {
                return offset as i64;
            }
            separator = true;
            continue;
        }
        if !(symbol.is_ascii_uppercase() || symbol.is_ascii_digit()) {
            return offset as i64;
        }
        separator = false;
    }
    -1
}

fn base64_invalid(source: &[char]) -> i64 {
    let size = source.len();
    if size == 0 {
        return -1;
    }
    let mut data_end = size;
    let mut padding = 0;
    let mut inside = false;
    for (offset, symbol) in source.iter().enumerate() {
        if *symbol == '=' {
            if !inside {
                inside = true;
                data_end = offset;
            }
            padding += 1;
            continue;
        }
        if inside || !ALPHABET.contains(*symbol) {
            return offset as i64;
        }
    }
    if size % 4 != 0 {
        return size as i64;
    }
    let expected = match data_end % 4 {
        0 => 0,
        2 => 2,
        3 => 1,
        _ => -1,
    };
    if expected < 0 || padding != expected {
        return data_end as i64;
    }
    let mut buffer: u64 = 0;
    let mut bits = 0;
    for symbol in &source[..data_end] {
        let digit = ALPHABET.find(*symbol).expect("validated base64") as u64;
        buffer = ((buffer << 6) | digit) & 0xffff_ffff;
        bits += 6;
        while bits >= 8 {
            bits -= 8;
        }
    }
    if bits > 0 && (buffer & ((1 << bits) - 1)) != 0 {
        return data_end as i64 - 1;
    }
    -1
}

struct PemLine {
    text: String,
    start: usize,
    end: usize,
    length: usize,
}

struct PemBlock {
    kind: String,
    base64: String,
    source: String,
}

fn pem_kind(line: &str) -> Option<&str> {
    let kind = line.strip_prefix("-----BEGIN ")?.strip_suffix("-----")?;
    // [A-Z0-9]+(?:[ -][A-Z0-9]+)*
    let word = |w: &str| !w.is_empty() && w.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
    kind.split([' ', '-']).all(word).then_some(kind)
}

/// Смещение первой ошибки (или -1) и блоки.
fn pem_parsed(origin: &str) -> (i64, Vec<PemBlock>) {
    let source = chars_of(origin);
    let size = source.len();
    let mut lines = Vec::new();
    let (mut start, mut offset) = (0, 0);
    let line = |from: usize, stop: usize, end: usize| PemLine {
        text: slice(&source, from, stop),
        start: from,
        end,
        length: stop - from,
    };
    while offset < size {
        let symbol = source[offset];
        if symbol == '\r' {
            if offset + 1 >= size || source[offset + 1] != '\n' {
                return (offset as i64, Vec::new());
            }
            lines.push(line(start, offset, offset + 2));
            offset += 2;
            start = offset;
            continue;
        }
        if symbol == '\n' {
            lines.push(line(start, offset, offset + 1));
            offset += 1;
            start = offset;
            continue;
        }
        offset += 1;
    }
    if start < size {
        lines.push(line(start, size, size));
    }
    let mut blocks = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        if lines[index].text.is_empty() {
            index += 1;
            continue;
        }
        let Some(kind) = pem_kind(&lines[index].text).map(str::to_string) else {
            return (lines[index].start as i64, Vec::new());
        };
        let block_start = lines[index].start;
        let mut block_end = lines[index].end;
        let mut last_line_start = lines[index].start;
        index += 1;
        let mut body: Vec<char> = Vec::new();
        let mut mapping = Vec::new();
        let mut body_lines: Vec<(usize, usize)> = Vec::new();
        let mut closed = false;
        while index < lines.len() {
            let current = &lines[index];
            last_line_start = current.start;
            if let Some(end_kind) = current
                .text
                .strip_prefix("-----END ")
                .and_then(|rest| rest.strip_suffix("-----"))
            {
                if end_kind != kind {
                    return ((current.start + 9) as i64, Vec::new());
                }
                closed = true;
                block_end = current.end;
                index += 1;
                break;
            }
            if current.text.is_empty() || current.text.starts_with("-----BEGIN ") {
                return (current.start as i64, Vec::new());
            }
            if current.length > 64 {
                return ((current.start + 64) as i64, Vec::new());
            }
            body_lines.push((current.start, current.length));
            for (column, point) in current.text.chars().enumerate() {
                body.push(point);
                mapping.push(current.start + column);
            }
            index += 1;
        }
        if !closed {
            return (size as i64, Vec::new());
        }
        for (at, length) in body_lines.iter().take(body_lines.len().saturating_sub(1)) {
            if *length != 64 {
                return ((at + length) as i64, Vec::new());
            }
        }
        let error = base64_invalid(&body);
        if error != -1 {
            let at = if (error as usize) < mapping.len() {
                mapping[error as usize]
            } else if let Some((start, length)) = body_lines.last() {
                start + length
            } else {
                last_line_start
            };
            return (at as i64, Vec::new());
        }
        blocks.push(PemBlock {
            kind,
            base64: body.into_iter().collect(),
            source: slice(&source, block_start, block_end),
        });
    }
    (-1, blocks)
}

fn pem_block(native: Native, origin: &str, index: &IntValue) -> Result<PemBlock, Error> {
    let (offset, mut blocks) = pem_parsed(origin);
    if offset != -1 {
        return Err(failed(native));
    }
    let at = super::support::small(index, blocks.len()).ok_or_else(|| failed(native))?;
    Ok(blocks.swap_remove(at))
}

// ---- runtime/template ----

#[derive(Clone, Debug)]
struct TemplateError {
    offset: i64,
    line: i64,
    column: i64,
    message: String,
}

fn template_error(source: &[char], offset: usize, message: &str) -> TemplateError {
    let prefix: String = source[..offset.min(source.len())].iter().collect();
    let prefix: Vec<char> = prefix.replace("\r\n", "\n").chars().collect();
    let newlines = prefix.iter().filter(|c| **c == '\n').count() as i64;
    let length = prefix.len() as i64;
    let last = prefix.iter().rposition(|c| *c == '\n').map_or(-1, |at| at as i64);
    TemplateError {
        offset: offset as i64,
        line: 1 + newlines,
        column: length - last,
        message: message.into(),
    }
}

#[derive(Debug)]
struct Template {
    source: String,
    input: IntValue,
    maximum_nodes: IntValue,
    compiled: RefCell<Option<Result<Vec<(i64, String)>, TemplateError>>>,
}

fn index_of(source: &[char], needle: &str, from: usize) -> Option<usize> {
    let needle: Vec<char> = needle.chars().collect();
    (from..=source.len().saturating_sub(needle.len()))
        .find(|at| *at + needle.len() <= source.len() && source[*at..*at + needle.len()] == needle[..])
}

fn compile_template(state: &Template) -> Result<Vec<(i64, String)>, TemplateError> {
    let source = chars_of(&state.source);
    if state.input.is_negative() || state.maximum_nodes.is_negative() {
        return Err(template_error(&source, 0, "negative template budget"));
    }
    if crate::integer::compare(&IntValue::from_u128(source.len() as u128), &state.input).is_gt() {
        return Err(template_error(&source, 0, "template input limit exceeded"));
    }
    let mut nodes: Vec<(i64, String)> = Vec::new();
    let mut literal = String::new();
    let append = |nodes: &mut Vec<(i64, String)>, kind: i64, text: String, offset: usize| {
        if crate::integer::compare(&IntValue::from_u128(nodes.len() as u128), &state.maximum_nodes)
            .is_ge()
        {
            return Err(template_error(&source, offset, "template node limit exceeded"));
        }
        nodes.push((kind, text));
        Ok(())
    };
    let flush = |nodes: &mut Vec<(i64, String)>, literal: &mut String, offset: usize| {
        if !literal.is_empty() {
            append(nodes, 0, std::mem::take(literal), offset)?;
        }
        Ok::<(), TemplateError>(())
    };
    let mut position = 0;
    while position < source.len() {
        let four = slice(&source, position, source.len().min(position + 4));
        if four == "{{{{" || four == "}}}}" {
            literal.push_str(&four[..2]);
            position += 4;
            continue;
        }
        let two = slice(&source, position, source.len().min(position + 2));
        if two == "}}" {
            return Err(template_error(
                &source,
                position,
                "unexpected closing template delimiter",
            ));
        }
        if two != "{{" {
            let mut next = source.len();
            for found in [index_of(&source, "{{", position), index_of(&source, "}}", position)]
                .into_iter()
                .flatten()
            {
                next = next.min(found);
            }
            literal.push_str(&slice(&source, position, next));
            position = next;
            continue;
        }
        let start = position;
        let Some(end) = index_of(&source, "}}", start + 2) else {
            return Err(template_error(&source, start, "unclosed template tag"));
        };
        let body = slice(&source, start + 2, end);
        position = end + 2;
        if body.starts_with('!') {
            let line_start = source[..start]
                .iter()
                .rposition(|c| *c == '\n')
                .map_or(0, |at| at + 1);
            let prefix = slice(&source, line_start, start);
            let rest = &source[position..];
            let spaces = rest.iter().take_while(|c| **c == ' ' || **c == '\t').count();
            let ending = match (rest.get(spaces), rest.get(spaces + 1)) {
                (None, _) => Some(0),
                (Some('\n'), _) => Some(1),
                (Some('\r'), Some('\n')) => Some(2),
                _ => None,
            };
            if prefix.chars().all(|c| c == ' ' || c == '\t') {
                if let Some(ending) = ending {
                    if !prefix.is_empty() {
                        let keep = literal.len() - prefix.len();
                        literal.truncate(keep);
                    }
                    position += spaces + ending;
                }
            }
            continue;
        }
        let body = body.trim_matches([' ', '\t']).to_string();
        if body.is_empty()
            || body.starts_with(['#', '^', '/', '>', '=', '&', '{'])
            || body.contains(['{', '}', '\r', '\n'])
        {
            return Err(template_error(
                &source,
                start,
                "unsupported or empty template tag",
            ));
        }
        if body.chars().count() > 4096 {
            return Err(template_error(&source, start, "template path length exceeded"));
        }
        let parts: Vec<&str> = body.split('.').collect();
        if parts.len() > 64 || parts.iter().any(|part| part.is_empty()) {
            return Err(template_error(&source, start, "invalid named template path"));
        }
        flush(&mut nodes, &mut literal, start)?;
        append(&mut nodes, 1, body, start)?;
    }
    flush(&mut nodes, &mut literal, position)?;
    Ok(nodes)
}

fn template(value: &Value) -> Result<Result<Vec<(i64, String)>, TemplateError>, Error> {
    let state = handle::<Template>(value)?;
    if state.compiled.borrow().is_none() {
        let compiled = compile_template(state);
        *state.compiled.borrow_mut() = Some(compiled);
    }
    Ok(state.compiled.borrow().clone().expect("compiled above"))
}

fn template_nodes(native: Native, value: &Value) -> Result<Vec<(i64, String)>, Error> {
    template(value)?.map_err(|_| {
        failure(
            "core.template",
            "invalid_data",
            native,
            "invalid template has no instructions",
        )
    })
}

// ---- runtime/ini ----

#[derive(Clone, Debug)]
struct IniError {
    line: i64,
    column: i64,
    message: String,
}

fn ini_error(line: i64, position: usize, message: &str) -> IniError {
    IniError {
        line,
        column: position as i64 + 1,
        message: message.into(),
    }
}

type IniSections = Vec<(String, Vec<(String, String)>)>;

#[derive(Debug)]
struct IniParse {
    source: String,
    input: IntValue,
    sections: IntValue,
    entries: IntValue,
    parsed: RefCell<Option<Result<IniSections, IniError>>>,
}

fn space(text: &[char], mut position: usize) -> usize {
    while position < text.len() && (text[position] == ' ' || text[position] == '\t') {
        position += 1;
    }
    position
}

fn quoted(text: &[char], position: usize, line: i64) -> Result<(String, usize), IniError> {
    let mut position = position + 1;
    let mut value = String::new();
    while position < text.len() {
        let point = text[position];
        position += 1;
        if point == '"' {
            return Ok((value, position));
        }
        if point != '\\' {
            value.push(point);
            continue;
        }
        if position == text.len() {
            return Err(ini_error(line, position, "unfinished escape"));
        }
        let escape = text[position];
        position += 1;
        let simple = match escape {
            '"' => Some('"'),
            '\\' => Some('\\'),
            'b' => Some('\u{08}'),
            't' => Some('\t'),
            'n' => Some('\n'),
            'f' => Some('\u{0c}'),
            'r' => Some('\r'),
            _ => None,
        };
        if let Some(simple) = simple {
            value.push(simple);
            continue;
        }
        if escape != 'u' && escape != 'U' {
            return Err(ini_error(line, position - 1, "unknown escape"));
        }
        let width = if escape == 'u' { 4 } else { 8 };
        let digits = slice(text, position, text.len().min(position + width));
        if digits.chars().count() != width || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ini_error(line, position, "invalid Unicode escape"));
        }
        let code = u32::from_str_radix(&digits, 16).unwrap_or(u32::MAX);
        let Some(scalar) = char::from_u32(code) else {
            return Err(ini_error(line, position, "escape is not a Unicode scalar"));
        };
        value.push(scalar);
        position += width;
    }
    Err(ini_error(line, position, "unterminated quoted string"))
}

fn ini_name(text: &[char], position: usize, delimiter: char, line: i64) -> Result<(String, usize), IniError> {
    let position = space(text, position);
    if position < text.len() && text[position] == '"' {
        let (name, after) = quoted(text, position, line)?;
        return Ok((name, space(text, after)));
    }
    let Some(end) = (position..text.len()).find(|at| text[*at] == delimiter) else {
        return Err(ini_error(line, text.len(), &format!("missing {delimiter}")));
    };
    let name = slice(text, position, end).trim_end_matches([' ', '\t']).to_string();
    if name.is_empty() {
        return Err(ini_error(line, position, "empty bare name"));
    }
    Ok((name, end))
}

fn ini_parse(state: &IniParse) -> Result<IniSections, IniError> {
    if state.input.is_negative() || state.sections.is_negative() || state.entries.is_negative() {
        return Err(ini_error(1, 0, "negative INI budget"));
    }
    let all = chars_of(&state.source);
    if crate::integer::compare(&IntValue::from_u128(all.len() as u128), &state.input).is_gt() {
        return Err(ini_error(1, 0, "maximum INI input exceeded"));
    }
    let mut sections: IniSections = Vec::new();
    let mut names_by_section: Vec<HashSet<String>> = Vec::new();
    let mut seen = HashSet::new();
    let mut entries: i64 = 0;
    let mut raw_lines = Vec::new();
    let mut from = 0;
    for at in 0..all.len() {
        if all[at] == '\n' {
            raw_lines.push(slice(&all, from, at + 1));
            from = at + 1;
        }
    }
    if from < all.len() {
        raw_lines.push(slice(&all, from, all.len()));
    }
    let limit = |value: &IntValue, count: usize| {
        crate::integer::compare(&IntValue::from_u128(count as u128), value).is_ge()
    };
    for (index, piece) in raw_lines.iter().enumerate() {
        let line = index as i64 + 1;
        let raw = piece
            .strip_suffix("\r\n")
            .or_else(|| piece.strip_suffix('\n'))
            .unwrap_or(piece);
        let text = chars_of(raw);
        if let Some(first) = text
            .iter()
            .position(|c| matches!(*c as u32, 0x00..=0x08 | 0x0a..=0x1f | 0x7f))
        {
            return Err(ini_error(line, first, "invalid scalar or raw control character"));
        }
        let position = space(&text, 0);
        if position == text.len() || text[position] == '#' || text[position] == ';' {
            continue;
        }
        if text[position] == '[' {
            let (name, end) = ini_name(&text, position + 1, ']', line)?;
            if end >= text.len() || text[end] != ']' {
                return Err(ini_error(line, end, "expected closing bracket"));
            }
            if space(&text, end + 1) != text.len() {
                return Err(ini_error(line, end + 1, "trailing section material"));
            }
            if seen.contains(&name) {
                return Err(ini_error(line, position, "duplicate INI section"));
            }
            if limit(&state.sections, sections.len()) {
                return Err(ini_error(line, position, "maximum INI sections exceeded"));
            }
            seen.insert(name.clone());
            sections.push((name, Vec::new()));
            names_by_section.push(HashSet::new());
            continue;
        }
        if sections.is_empty() {
            return Err(ini_error(line, position, "entry requires a section"));
        }
        let (name, end) = ini_name(&text, position, '=', line)?;
        if end >= text.len() || text[end] != '=' {
            return Err(ini_error(line, end, "expected equals"));
        }
        let start = space(&text, end + 1);
        let value = if start < text.len() && text[start] == '"' {
            let (value, following) = quoted(&text, start, line)?;
            if space(&text, following) != text.len() {
                return Err(ini_error(line, following, "trailing value material"));
            }
            value
        } else {
            slice(&text, start.min(text.len()), text.len())
                .trim_end_matches([' ', '\t'])
                .to_string()
        };
        let names = names_by_section.last_mut().expect("current section");
        let list = &mut sections.last_mut().expect("current section").1;
        if names.contains(&name) {
            return Err(ini_error(line, position, "duplicate INI entry"));
        }
        if limit(&state.entries, entries as usize) || list.len() >= 1024 {
            return Err(ini_error(line, position, "maximum INI entries exceeded"));
        }
        names.insert(name.clone());
        list.push((name, value));
        entries += 1;
    }
    Ok(sections)
}

fn ini(value: &Value) -> Result<Result<IniSections, IniError>, Error> {
    let state = handle::<IniParse>(value)?;
    if state.parsed.borrow().is_none() {
        let parsed = ini_parse(state);
        *state.parsed.borrow_mut() = Some(parsed);
    }
    Ok(state.parsed.borrow().clone().expect("parsed above"))
}

fn ini_sections(native: Native, value: &Value) -> Result<IniSections, Error> {
    ini(value)?.map_err(|_| failure("core.ini", "invalid_data", native, "invalid INI has no sections"))
}

fn ini_section(native: Native, value: &Value, index: &IntValue) -> Result<(String, Vec<(String, String)>), Error> {
    let mut sections = ini_sections(native, value)?;
    let at = super::support::small(index, sections.len()).ok_or_else(|| {
        failure(
            "core.ini",
            "invalid_input",
            native,
            "INI section index outside document",
        )
    })?;
    Ok(sections.swap_remove(at))
}

#[derive(Debug)]
struct IniWrite {
    maximum: IntValue,
    text: String,
    length: i64,
    status: i64,
    name: String,
    parent: Option<String>,
    sections: HashSet<String>,
    entries: HashSet<String>,
}

impl IniWrite {
    fn error(&mut self, status: i64, name: &str) -> bool {
        if self.status != 0 {
            return false;
        }
        self.status = status;
        self.name = name.to_string();
        self.text.clear();
        self.length = 0;
        false
    }

    fn append(&mut self, text: &str) -> bool {
        if self.status != 0 {
            return false;
        }
        let size = text.chars().count() as i64;
        let room = crate::integer::subtract(&self.maximum, &IntValue::from_i64(self.length));
        if crate::integer::compare(&IntValue::from_i64(size), &room).is_gt() {
            return self.error(4, "");
        }
        self.text.push_str(text);
        self.length += size;
        true
    }

    fn quote(&mut self, text: &str) -> bool {
        if !self.append("\"") {
            return false;
        }
        for point in text.chars() {
            let escaped = match point {
                '"' => "\\\"".to_string(),
                '\\' => "\\\\".to_string(),
                '\u{08}' => "\\b".to_string(),
                '\t' => "\\t".to_string(),
                '\n' => "\\n".to_string(),
                '\u{0c}' => "\\f".to_string(),
                '\r' => "\\r".to_string(),
                c if (c as u32) < 32 || c as u32 == 127 => format!("\\u{:04x}", c as u32),
                c => c.to_string(),
            };
            if !self.append(&escaped) {
                return false;
            }
        }
        self.append("\"")
    }
}

fn ini_write(value: &Value) -> Result<&RefCell<IniWrite>, Error> {
    handle::<RefCell<IniWrite>>(value)
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    let name = operation(native);
    match name {
        // ---- io_decorators ----
        "limited_reading_state_create" => {
            let limit = int(&args[0])?;
            if limit.is_negative() {
                return Err(failure(
                    "core.io_decorators",
                    "invalid_state",
                    native,
                    "Limited reading state requires a non-negative limit",
                ));
            }
            Ok(new_handle(Counter(RefCell::new(limit.clone()))))
        }
        "limited_reading_state_remaining" | "chained_reading_state_position" => {
            Ok(Value::Int(handle::<Counter>(&args[0])?.0.borrow().clone()))
        }
        "limited_reading_state_consume" => {
            let counter = handle::<Counter>(&args[0])?;
            let amount = int(&args[1])?;
            let current = counter.0.borrow().clone();
            if amount.is_negative() || crate::integer::compare(amount, &current).is_gt() {
                return Err(failure(
                    "core.io_decorators",
                    "invalid_state",
                    native,
                    "Limited reading consumption exceeds remaining bytes",
                ));
            }
            let next = crate::integer::subtract(&current, amount);
            *counter.0.borrow_mut() = next.clone();
            Ok(Value::Int(next))
        }
        "chained_reading_state_create" => {
            Ok(new_handle(Counter(RefCell::new(IntValue::from_i64(0)))))
        }
        "chained_reading_state_advance" => {
            let counter = handle::<Counter>(&args[0])?;
            let next = crate::integer::add(&counter.0.borrow(), &IntValue::from_i64(1));
            *counter.0.borrow_mut() = next.clone();
            Ok(Value::Int(next))
        }
        // ---- testing ----
        "test_state_open" => {
            require_test(native)?;
            let limit = int(&args[0])?
                .to_i64()
                .filter(|limit| (0..=65_536).contains(limit))
                .ok_or_else(|| {
                    failure(
                        "core.testing",
                        "invalid_input",
                        native,
                        "Test state capacity must be 0..65536",
                    )
                })?;
            Ok(new_handle(TestState {
                limit,
                size: Cell::new(0),
                closed: Cell::new(false),
            }))
        }
        "test_state_size" => Ok(int_value(test_state(native, &args[0])?.size.get())),
        "test_state_next" => {
            let state = test_state(native, &args[0])?;
            if state.size.get() >= state.limit {
                return Err(failure(
                    "core.testing",
                    "resource_exhausted",
                    native,
                    "Test state capacity exhausted",
                ));
            }
            state.size.set(state.size.get() + 1);
            Ok(int_value(state.size.get() - 1))
        }
        "test_state_close" => {
            require_test(native)?;
            handle::<TestState>(&args[0])?.closed.set(true);
            Ok(Value::Bool(true))
        }
        "test_context_active" => Ok(Value::Bool(crate::testing::active())),
        // ---- csv_state ----
        "csv_state_create" => {
            let mut limits = [0i64; 5];
            for (index, value) in args.iter().take(5).enumerate() {
                let value = int(value)?;
                if value.is_negative() {
                    return Err(failure(
                        "core.csv_state",
                        "invalid_input",
                        native,
                        "CSV limits must be nonnegative",
                    ));
                }
                limits[index] = value
                    .to_i64()
                    .filter(|value| *value < 1 << 62)
                    .unwrap_or(i64::MAX / 2);
            }
            Ok(new_handle(RefCell::new(Csv {
                limits,
                buffer: Vec::new(),
                fed: 0,
                bytes: 0,
                scalars: 0,
                rows: 0,
                field: String::new(),
                field_length: 0,
                fields: Vec::new(),
                mode: "start",
                active: false,
                eof: false,
                status: "need",
                offset: 0,
            })))
        }
        "csv_state_feed" => {
            let mut state = handle::<RefCell<Csv>>(&args[0])?.borrow_mut();
            let data = bytes(&args[1])?;
            if state.status != "need" || data.len() as i64 > state.request() || state.eof {
                return Err(failure(
                    "core.csv_state",
                    "invalid_state",
                    native,
                    "Invalid CSV state operation",
                ));
            }
            state.fed += data.len() as i64;
            if data.is_empty() {
                state.eof = true;
            }
            state.buffer.extend(data);
            Ok(Value::Bool(true))
        }
        "csv_state_advance" => Ok(text(handle::<RefCell<Csv>>(&args[0])?.borrow_mut().advance())),
        "csv_state_row" => {
            let mut state = handle::<RefCell<Csv>>(&args[0])?.borrow_mut();
            if state.status != "row" {
                return Err(failure(
                    "core.csv_state",
                    "invalid_state",
                    native,
                    "Invalid CSV state operation",
                ));
            }
            let row = std::mem::take(&mut state.fields);
            state.status = "need";
            Ok(string_array(row))
        }
        "csv_state_offset" => Ok(int_value(handle::<RefCell<Csv>>(&args[0])?.borrow().offset)),
        "csv_state_request" => Ok(int_value(handle::<RefCell<Csv>>(&args[0])?.borrow().request())),
        "csv_state_closed" => Ok(Value::Bool(
            handle::<RefCell<Csv>>(&args[0])?.borrow().status == "closed",
        )),
        "csv_state_close" => {
            handle::<RefCell<Csv>>(&args[0])?
                .borrow_mut()
                .error("closed", Some(0));
            Ok(Value::Bool(true))
        }
        // ---- pem ----
        "pem_kind_invalid_offset" => Ok(int_value(kind_invalid_offset(string(&args[0])?))),
        "pem_encode" => {
            let (kind, base64) = (string(&args[0])?, chars_of(string(&args[1])?));
            let mut out = format!("-----BEGIN {kind}-----\n");
            for chunk in base64.chunks(64) {
                out.extend(chunk);
                out.push('\n');
            }
            out.push_str(&format!("-----END {kind}-----\n"));
            Ok(text(out))
        }
        "pem_block_sources" => {
            let (offset, blocks) = pem_parsed(string(&args[0])?);
            if offset != -1 {
                return Err(failure("core.pem", "invalid_data", native, "invalid PEM input"));
            }
            Ok(string_array(blocks.into_iter().map(|block| block.source)))
        }
        "pem_invalid_offset" => Ok(int_value(pem_parsed(string(&args[0])?).0)),
        "pem_block_count" => {
            let (offset, blocks) = pem_parsed(string(&args[0])?);
            if offset != -1 {
                return Err(failed(native));
            }
            Ok(int_value(blocks.len() as i64))
        }
        "pem_block_source" => Ok(text(pem_block(native, string(&args[0])?, int(&args[1])?)?.source)),
        "pem_block_kind" => Ok(text(pem_block(native, string(&args[0])?, int(&args[1])?)?.kind)),
        "pem_block_base64" => Ok(text(pem_block(native, string(&args[0])?, int(&args[1])?)?.base64)),
        // ---- template ----
        "template_compile_state" => Ok(new_handle(Template {
            source: string(&args[0])?.to_string(),
            input: int(&args[1])?.clone(),
            maximum_nodes: int(&args[2])?.clone(),
            compiled: RefCell::new(None),
        })),
        "template_compile_valid" => Ok(Value::Bool(template(&args[0])?.is_ok())),
        "template_compile_offset" | "template_compile_line" | "template_compile_column" => {
            Ok(int_value(match template(&args[0])? {
                Ok(_) => 0,
                Err(error) => match name {
                    "template_compile_offset" => error.offset,
                    "template_compile_line" => error.line,
                    _ => error.column,
                },
            }))
        }
        "template_compile_message" => Ok(text(match template(&args[0])? {
            Ok(_) => String::new(),
            Err(error) => error.message,
        })),
        "template_node_count" => Ok(int_value(template_nodes(native, &args[0])?.len() as i64)),
        "template_node_kind" | "template_node_text" => {
            let nodes = template_nodes(native, &args[0])?;
            let at = super::support::small(int(&args[1])?, nodes.len()).ok_or_else(|| {
                failure(
                    "core.template",
                    "invalid_input",
                    native,
                    "template instruction index outside program",
                )
            })?;
            let (kind, value) = nodes[at].clone();
            Ok(if name == "template_node_kind" {
                int_value(kind)
            } else {
                text(value)
            })
        }
        "template_output_size" => {
            let maximum = int(&args[1])?;
            if maximum.is_negative() {
                return Err(failure(
                    "core.template",
                    "invalid_input",
                    native,
                    "negative template byte budget",
                ));
            }
            let value = string(&args[0])?;
            let limit = maximum.to_i64().unwrap_or(i64::MAX);
            if value.chars().count() as i64 > limit {
                return Ok(int_value(-1));
            }
            let size = value.len() as i64;
            Ok(int_value(if size > limit { -1 } else { size }))
        }
        // ---- ini ----
        "ini_parse_state" => Ok(new_handle(IniParse {
            source: string(&args[0])?.to_string(),
            input: int(&args[1])?.clone(),
            sections: int(&args[2])?.clone(),
            entries: int(&args[3])?.clone(),
            parsed: RefCell::new(None),
        })),
        "ini_parse_valid" => Ok(Value::Bool(ini(&args[0])?.is_ok())),
        "ini_parse_line" | "ini_parse_column" => Ok(int_value(match ini(&args[0])? {
            Ok(_) => 0,
            Err(error) if name == "ini_parse_line" => error.line,
            Err(error) => error.column,
        })),
        "ini_parse_message" => Ok(text(match ini(&args[0])? {
            Ok(_) => String::new(),
            Err(error) => error.message,
        })),
        "ini_section_count" => Ok(int_value(ini_sections(native, &args[0])?.len() as i64)),
        "ini_section_name" => Ok(text(ini_section(native, &args[0], int(&args[1])?)?.0)),
        "ini_entry_count" => Ok(int_value(
            ini_section(native, &args[0], int(&args[1])?)?.1.len() as i64,
        )),
        "ini_entry_name" | "ini_entry_value" => {
            let entries = ini_section(native, &args[0], int(&args[1])?)?.1;
            let at = super::support::small(int(&args[2])?, entries.len()).ok_or_else(|| {
                failure(
                    "core.ini",
                    "invalid_input",
                    native,
                    "INI entry index outside section",
                )
            })?;
            let (entry_name, value) = entries[at].clone();
            Ok(text(if name == "ini_entry_name" {
                entry_name
            } else {
                value
            }))
        }
        "ini_write_state" => {
            let maximum = int(&args[0])?;
            if maximum.is_negative() {
                return Err(failure(
                    "core.ini",
                    "invalid_input",
                    native,
                    "negative INI output budget",
                ));
            }
            Ok(new_handle(RefCell::new(IniWrite {
                maximum: maximum.clone(),
                text: String::new(),
                length: 0,
                status: 0,
                name: String::new(),
                parent: None,
                sections: HashSet::new(),
                entries: HashSet::new(),
            })))
        }
        "ini_write_section" => {
            let mut state = ini_write(&args[0])?.borrow_mut();
            let section = string(&args[1])?;
            if state.status != 0 {
                return Ok(Value::Bool(false));
            }
            if state.sections.contains(section) {
                return Ok(Value::Bool(state.error(2, section)));
            }
            if !(state.append("[") && state.quote(section) && state.append("]\n")) {
                return Ok(Value::Bool(false));
            }
            state.sections.insert(section.to_string());
            state.parent = Some(section.to_string());
            state.entries.clear();
            Ok(Value::Bool(true))
        }
        "ini_write_entry" => {
            let mut state = ini_write(&args[0])?.borrow_mut();
            let (entry, value) = (string(&args[1])?, string(&args[2])?);
            if state.status != 0 {
                return Ok(Value::Bool(false));
            }
            if state.parent.is_none() {
                return Ok(Value::Bool(state.error(1, "")));
            }
            if state.entries.contains(entry) {
                return Ok(Value::Bool(state.error(3, entry)));
            }
            if !(state.quote(entry) && state.append("=") && state.quote(value) && state.append("\n"))
            {
                return Ok(Value::Bool(false));
            }
            state.entries.insert(entry.to_string());
            Ok(Value::Bool(true))
        }
        "ini_write_status" => Ok(int_value(ini_write(&args[0])?.borrow().status)),
        "ini_write_name" => Ok(text(ini_write(&args[0])?.borrow().name.clone())),
        "ini_write_parent" => Ok(text(
            ini_write(&args[0])?.borrow().parent.clone().unwrap_or_default(),
        )),
        "ini_write_string" => {
            let state = ini_write(&args[0])?.borrow();
            if state.status != 0 {
                return Err(failure(
                    "core.ini",
                    "invalid_data",
                    native,
                    "failed INI emission has no text",
                ));
            }
            Ok(text(state.text.clone()))
        }
        _ => Err(Error::Unsupported(format!(
            "native provider is not implemented: {}",
            native.symbol()
        ))),
    }
}
