//! `runtime/x509` — перенос `X509Natives` JVM: строгий DER-конверт и снимок
//! фактов сертификата, наблюдатели расширений, системные корни и проверка
//! цепочки. Декодирование ключа (`CertificateFactory` JDK) и построение пути
//! PKIX (`CertPathBuilder` JDK) — здесь же, подписи проверяет okkama-native.
use super::idna::{hostname_ascii, suffix_parts};
use super::net::{format as ip_text, parse_ipv4, parse_ipv6};
use super::support::{array, byte_array, bytes, failure, items, string, string_array, text};
use super::Error;
use crate::integer;
use crate::{IntValue, Value, abi::Native};
use std::collections::{HashMap, HashSet};
use unicode_normalization::UnicodeNormalization;

const DOMAIN: &str = "core.x509";
const VERIFY: &str = "core.x509.verify";

fn raise(domain: &str, code: &str, operation: &str, message: &str) -> Error {
    Error::Failure(crate::abi::NativeFailure {
        domain: domain.into(),
        code: code.into(),
        operation: operation.into(),
        message: message.into(),
    })
}

/// Отказ разбора: `invalid`, `limit` или `unsupported`.
#[derive(Debug)]
struct Bad(&'static str);

fn bad<T>() -> Result<T, Bad> {
    Err(Bad("invalid"))
}

fn limit<T>() -> Result<T, Bad> {
    Err(Bad("limit"))
}

// ---- DER ----

#[derive(Debug, Clone)]
struct Node {
    tag: u8,
    number: u32,
    cls: u8,
    constructed: bool,
    from: usize,
    content: usize,
    length: usize,
    to: usize,
    children: Vec<Node>,
}

fn content<'a>(source: &'a [u8], node: &Node) -> &'a [u8] {
    &source[node.content..node.content + node.length]
}

fn encoded<'a>(source: &'a [u8], node: &Node) -> &'a [u8] {
    &source[node.from..node.to]
}

fn octet(source: &[u8], offset: usize, end: usize) -> Result<u8, Bad> {
    if offset >= end {
        return bad();
    }
    Ok(source[offset])
}

fn bit_string(s: &[u8]) -> Result<(), Bad> {
    if s.is_empty() {
        return bad();
    }
    let unused = s[0];
    if unused > 7 || (s.len() == 1 && unused != 0) {
        return bad();
    }
    if unused != 0 && (s[s.len() - 1] & ((1u8 << unused) - 1)) != 0 {
        return bad();
    }
    Ok(())
}

fn integer_bytes(s: &[u8]) -> Result<(), Bad> {
    if s.is_empty() {
        return bad();
    }
    if s.len() > 1 {
        let (a, b) = (s[0], s[1]);
        if (a == 0 && b & 128 == 0) || (a == 255 && b & 128 != 0) {
            return bad();
        }
    }
    Ok(())
}

fn oid(s: &[u8]) -> Result<String, Bad> {
    if s.is_empty() {
        return bad();
    }
    if s.len() > 1024 {
        return limit();
    }
    let mut arcs: Vec<IntValue> = Vec::new();
    let mut value = IntValue::from_i64(0);
    let mut starting = true;
    let shift = IntValue::from_i64(128);
    for &octet in s {
        if starting && octet == 128 {
            return bad();
        }
        value = integer::add(&integer::multiply(&value, &shift), &IntValue::from_i64(i64::from(octet & 127)));
        starting = false;
        if octet & 128 == 0 {
            arcs.push(value);
            if arcs.len() > 127 {
                return limit();
            }
            value = IntValue::from_i64(0);
            starting = true;
        }
    }
    if !starting {
        return bad();
    }
    let first = arcs.remove(0);
    let forty = IntValue::from_i64(40);
    let eighty = IntValue::from_i64(80);
    let root: i64 = if integer::compare(&first, &forty).is_lt() {
        0
    } else if integer::compare(&first, &eighty).is_lt() {
        1
    } else {
        2
    };
    let mut out = format!("{root}.{}", integer::subtract(&first, &IntValue::from_i64(40 * root)));
    for arc in arcs {
        out.push('.');
        out.push_str(&arc.to_string());
    }
    if out.len() > 1024 {
        return limit();
    }
    Ok(out)
}

fn node(source: &[u8], mut offset: usize, end: usize, depth: usize, budget: &mut usize) -> Result<Node, Bad> {
    if depth > 32 {
        return limit();
    }
    *budget += 1;
    if *budget > 8192 {
        return limit();
    }
    let from = offset;
    let tag = octet(source, offset, end)?;
    offset += 1;
    let mut number = u32::from(tag & 31);
    if number == 31 {
        number = 0;
        let mut first = true;
        loop {
            let part = octet(source, offset, end)?;
            offset += 1;
            if first && part & 127 == 0 {
                return bad();
            }
            if number > 0x1f_ffff {
                return limit();
            }
            number = (number << 7) | u32::from(part & 127);
            first = false;
            if part & 128 == 0 {
                break;
            }
        }
        if number < 31 {
            return bad();
        }
    }
    let mut length = u64::from(octet(source, offset, end)?);
    offset += 1;
    if length & 128 != 0 {
        let width = (length & 127) as usize;
        if width == 0 || width > 4 {
            return bad();
        }
        if octet(source, offset, end)? == 0 {
            return bad();
        }
        length = 0;
        for _ in 0..width {
            length = (length << 8) | u64::from(octet(source, offset, end)?);
            offset += 1;
        }
        if length < 128 {
            return bad();
        }
    }
    if length > (end - offset) as u64 {
        return bad();
    }
    let length = length as usize;
    let to = offset + length;
    let constructed = tag & 32 != 0;
    let cls = tag >> 6;
    if cls == 0 {
        if number == 0 {
            return bad();
        }
        if if number == 16 || number == 17 { !constructed } else { constructed } {
            return bad();
        }
    }
    let mut result = Node {
        tag,
        number,
        cls,
        constructed,
        from,
        content: offset,
        length,
        to,
        children: Vec::new(),
    };
    if constructed {
        let mut cursor = offset;
        while cursor < to {
            let child = node(source, cursor, to, depth + 1, budget)?;
            cursor = child.to;
            result.children.push(child);
        }
        if cls == 0 && number == 17 {
            for pair in result.children.windows(2) {
                if encoded(source, &pair[0]) > encoded(source, &pair[1]) {
                    return bad();
                }
            }
        }
    } else if cls == 0 {
        let s = &source[offset..to];
        if number == 1 && (length != 1 || (s[0] != 0 && s[0] != 0xff)) {
            return bad();
        }
        if number == 2 || number == 10 {
            integer_bytes(s)?;
        }
        if number == 3 {
            bit_string(s)?;
        }
        if number == 5 && length != 0 {
            return bad();
        }
        if number == 6 {
            oid(s)?;
        }
    }
    Ok(result)
}

fn parse(source: &[u8]) -> Result<Node, Bad> {
    if source.len() > 1_048_576 {
        return limit();
    }
    let mut budget = 0;
    let root = node(source, 0, source.len(), 0, &mut budget)?;
    if root.to != source.len() {
        return bad();
    }
    Ok(root)
}

fn tag(node: Option<&Node>, tag: u8) -> Result<&Node, Bad> {
    match node {
        Some(node) if node.tag == tag => Ok(node),
        _ => bad(),
    }
}

fn sequence(node: &Node, size: Option<usize>) -> Result<&[Node], Bad> {
    tag(Some(node), 0x30)?;
    if size.is_some_and(|size| node.children.len() != size) {
        return bad();
    }
    Ok(&node.children)
}

fn algorithm(node: &Node) -> Result<(), Bad> {
    let fields = sequence(node, None)?;
    if fields.len() != 1 && fields.len() != 2 {
        return bad();
    }
    tag(fields.first(), 6)?;
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|index| u8::from_str_radix(&text[2 * index..2 * index + 2], 16).unwrap_or(0))
        .collect()
}

/// Атрибут имени: OID, тег значения, hex значения.
type Attribute = (String, u8, String);
type Name = Vec<Vec<Attribute>>;

fn name(source: &[u8], node: &Node) -> Result<Name, Bad> {
    let groups = sequence(node, None)?;
    if groups.len() > 128 {
        return limit();
    }
    let mut result = Vec::new();
    for group in groups {
        tag(Some(group), 0x31)?;
        if group.children.is_empty() {
            return bad();
        }
        if group.children.len() > 32 {
            return limit();
        }
        let mut attributes = Vec::new();
        for member in &group.children {
            let parts = sequence(member, Some(2))?;
            tag(parts.first(), 6)?;
            if parts[1].constructed {
                return bad();
            }
            attributes.push((oid(content(source, &parts[0]))?, parts[1].tag, hex(content(source, &parts[1]))));
        }
        result.push(attributes);
    }
    Ok(result)
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn time(source: &[u8], node: &Node) -> Result<String, Bad> {
    let text = content(source, node);
    let digits = |range: std::ops::Range<usize>| -> i64 {
        text[range].iter().fold(0i64, |acc, b| acc * 10 + i64::from(b - b'0'))
    };
    let shape = |width: usize| text.len() == width + 1 && text[width] == b'Z' && text[..width].iter().all(u8::is_ascii_digit);
    let (year, at) = if node.tag == 23 && shape(12) {
        let year = digits(0..2);
        (year + if year >= 50 { 1900 } else { 2000 }, 2)
    } else if node.tag == 24 && shape(14) {
        (digits(0..4), 4)
    } else {
        return bad();
    };
    let month = digits(at..at + 2);
    let day = digits(at + 2..at + 4);
    let hour = digits(at + 4..at + 6);
    let minute = digits(at + 6..at + 8);
    let second = digits(at + 8..at + 10);
    if !(year >= 1 && (1..=12).contains(&month) && (1..=31).contains(&day) && hour < 24 && minute < 60 && second < 60) {
        return bad();
    }
    // LocalDateTime.of: день должен существовать в месяце.
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if day > days[(month - 1) as usize] {
        return bad();
    }
    let epoch = days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second;
    Ok(epoch.to_string())
}

/// Снимок фактов сертификата.
#[derive(Debug, Clone)]
struct Facts {
    subject: Name,
    issuer: Name,
    times: [String; 2],
    spki: String,
    /// OID, critical (0/1), hex значения.
    extensions: Vec<(String, u8, String)>,
}

/// Разобранный сертификат: факты и части для проверки подписи.
#[derive(Debug, Clone)]
struct Parsed {
    facts: Facts,
    version: u8,
    tbs: Vec<u8>,
    algorithm: Vec<u8>,
    signature: Vec<u8>,
}

fn certificate(source: &[u8]) -> Result<Parsed, Bad> {
    let root = parse(source)?;
    let top = sequence(&root, Some(3))?;
    let (tbs, outer, signature) = (&top[0], &top[1], &top[2]);
    algorithm(outer)?;
    tag(Some(signature), 3)?;
    let sig = content(source, signature);
    if !(sig.len() > 1 && sig[0] == 0) {
        return bad();
    }
    let mut fields: Vec<&Node> = sequence(tbs, None)?.iter().collect();
    let mut version = 0;
    if fields.first().is_some_and(|field| field.tag == 0xa0) {
        let explicit = fields.remove(0);
        if explicit.children.len() != 1 {
            return bad();
        }
        let value = tag(explicit.children.first(), 2)?;
        let s = content(source, value);
        if !(s.len() == 1 && (s[0] == 1 || s[0] == 2)) {
            return bad();
        }
        version = s[0];
    }
    if fields.len() < 6 {
        return bad();
    }
    let (serial, inner, issuer, validity, subject, spki) = (fields[0], fields[1], fields[2], fields[3], fields[4], fields[5]);
    tag(Some(serial), 2)?;
    algorithm(inner)?;
    if encoded(source, outer) != encoded(source, inner) {
        return bad();
    }
    let issuer = name(source, issuer)?;
    if issuer.is_empty() {
        return bad();
    }
    let subject = name(source, subject)?;
    let period = sequence(validity, Some(2))?;
    let times = [time(source, &period[0])?, time(source, &period[1])?];
    if times[0].parse::<i64>().unwrap_or(0) > times[1].parse::<i64>().unwrap_or(0) {
        return bad();
    }
    let key = sequence(spki, Some(2))?;
    algorithm(&key[0])?;
    tag(Some(&key[1]), 3)?;
    if !(key[1].length > 1 && content(source, &key[1])[0] == 0) {
        return bad();
    }
    let mut extensions = Vec::new();
    let mut seen = HashSet::new();
    let mut previous = 0;
    for field in &fields[6..] {
        let order = match field.tag {
            0x81 => 1,
            0x82 => 2,
            0xa3 => 3,
            _ => 0,
        };
        if order <= previous {
            return bad();
        }
        previous = order;
        if order < 3 {
            if version == 0 {
                return bad();
            }
            bit_string(content(source, field))?;
            continue;
        }
        if version != 2 {
            return bad();
        }
        if field.children.len() != 1 {
            return bad();
        }
        let list = sequence(&field.children[0], None)?;
        if list.is_empty() {
            return bad();
        }
        if list.len() > 128 {
            return limit();
        }
        for item in list {
            let mut parts: Vec<&Node> = sequence(item, None)?.iter().collect();
            if parts.len() != 2 && parts.len() != 3 {
                return bad();
            }
            let identifier = parts.remove(0);
            tag(Some(identifier), 6)?;
            let identifier = oid(content(source, identifier))?;
            if !seen.insert(identifier.clone()) {
                return bad();
            }
            let mut critical = 0;
            if parts.len() == 2 {
                let flag = parts.remove(0);
                tag(Some(flag), 1)?;
                if content(source, flag)[0] != 0xff {
                    return bad();
                }
                critical = 1;
            }
            tag(parts.first().copied(), 4)?;
            extensions.push((identifier, critical, hex(content(source, parts[0]))));
        }
    }
    Ok(Parsed {
        facts: Facts {
            subject,
            issuer,
            times,
            spki: hex(encoded(source, spki)),
            extensions,
        },
        version,
        tbs: encoded(source, tbs).to_vec(),
        algorithm: encoded(source, outer).to_vec(),
        signature: sig[1..].to_vec(),
    })
}

// ---- снимок: канонический JSON ----

fn names_json(out: &mut String, groups: &Name) {
    out.push('[');
    for (index, group) in groups.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('[');
        for (member, (oid, tag, value)) in group.iter().enumerate() {
            if member > 0 {
                out.push(',');
            }
            out.push_str(&format!("[\"{oid}\",{tag},\"{value}\"]"));
        }
        out.push(']');
    }
    out.push(']');
}

fn json(facts: &Facts) -> String {
    let mut out = String::from("{\"extensions\":[");
    for (index, (oid, critical, value)) in facts.extensions.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&format!("[\"{oid}\",{critical},\"{value}\"]"));
    }
    out.push_str("],\"issuer\":");
    names_json(&mut out, &facts.issuer);
    out.push_str(&format!(",\"spki\":\"{}\",\"subject\":", facts.spki));
    names_json(&mut out, &facts.subject);
    out.push_str(&format!(",\"times\":[{},{}],\"version\":1}}", facts.times[0], facts.times[1]));
    out
}

/// Значение минимального JSON снимка.
#[derive(Debug, Clone)]
enum Json {
    List(Vec<Json>),
    Map(HashMap<String, Json>),
    Text(String),
    Number(String),
}

struct Reader<'a> {
    text: &'a [char],
    at: usize,
}

impl Reader<'_> {
    fn skip(&mut self) {
        while self.at < self.text.len() && self.text[self.at].is_whitespace() {
            self.at += 1;
        }
    }

    fn peek(&self) -> Result<char, Bad> {
        self.text.get(self.at).copied().ok_or(Bad("invalid"))
    }

    fn take(&mut self) -> Result<char, Bad> {
        let c = self.peek()?;
        self.at += 1;
        Ok(c)
    }

    fn value(&mut self) -> Result<Json, Bad> {
        self.skip();
        let c = self.peek()?;
        if c == '[' {
            self.at += 1;
            let mut items = Vec::new();
            self.skip();
            if self.peek()? == ']' {
                self.at += 1;
                return Ok(Json::List(items));
            }
            loop {
                items.push(self.value()?);
                self.skip();
                match self.take()? {
                    ']' => return Ok(Json::List(items)),
                    ',' => {}
                    _ => return bad(),
                }
            }
        }
        if c == '{' {
            self.at += 1;
            let mut entries = HashMap::new();
            self.skip();
            if self.peek()? == '}' {
                self.at += 1;
                return Ok(Json::Map(entries));
            }
            loop {
                let Json::Text(key) = self.value()? else {
                    return bad();
                };
                self.skip();
                if self.take()? != ':' {
                    return bad();
                }
                let value = self.value()?;
                entries.insert(key, value);
                self.skip();
                match self.take()? {
                    '}' => return Ok(Json::Map(entries)),
                    ',' => {}
                    _ => return bad(),
                }
            }
        }
        if c == '"' {
            self.at += 1;
            let mut out: Vec<u16> = Vec::new();
            loop {
                let next = self.take()?;
                if next == '"' {
                    return Ok(Json::Text(String::from_utf16_lossy(&out)));
                }
                if next == '\\' {
                    let escape = self.take()?;
                    match escape {
                        'u' => {
                            let digits: String = self.text.get(self.at..self.at + 4).ok_or(Bad("invalid"))?.iter().collect();
                            out.push(u16::from_str_radix(&digits, 16).map_err(|_| Bad("invalid"))?);
                            self.at += 4;
                        }
                        'n' => out.push(10),
                        't' => out.push(9),
                        'r' => out.push(13),
                        'b' => out.push(8),
                        'f' => out.push(12),
                        other => {
                            let mut buffer = [0u16; 2];
                            out.extend_from_slice(other.encode_utf16(&mut buffer));
                        }
                    }
                } else {
                    let mut buffer = [0u16; 2];
                    out.extend_from_slice(next.encode_utf16(&mut buffer));
                }
            }
        }
        let start = self.at;
        if c == '-' {
            self.at += 1;
        }
        // Character.isDigit: десятичные цифры Unicode (Nd).
        while self.at < self.text.len() && crate::data::category(u32::from(self.text[self.at])) == "Nd" {
            self.at += 1;
        }
        if self.at == start || (self.at == start + 1 && c == '-') {
            return bad();
        }
        let spelled: String = self.text[start..self.at].iter().collect();
        // BigInteger понимает цифры Unicode; канонический текст — ASCII.
        let ascii: String = spelled
            .chars()
            .map(|ch| if ch == '-' { '-' } else { char::from(b'0' + ch.to_digit(10).map_or_else(|| unicode_digit(ch), |d| d as u8)) })
            .collect();
        let value = IntValue::parse(&ascii).ok_or(Bad("invalid"))?;
        Ok(Json::Number(value.to_string()))
    }
}

fn unicode_digit(ch: char) -> u8 {
    // Цифры Nd идут блоками по десять от нуля блока.
    let point = u32::from(ch);
    let mut zero = point;
    while zero > 0 && crate::data::category(zero - 1) == "Nd" && point - (zero - 1) < 10 {
        zero -= 1;
    }
    (point - zero) as u8
}

fn is_oid(text: &str) -> bool {
    let mut parts = text.split('.');
    matches!(parts.next(), Some("0" | "1" | "2"))
        && text.contains('.')
        && parts.all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

fn is_hexes(text: &str) -> bool {
    text.len() % 2 == 0 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn small_number(value: &Json, maximum: i64) -> Option<u8> {
    match value {
        Json::Number(text) => text.parse::<i64>().ok().filter(|v| (0..=maximum).contains(v)).map(|v| v as u8),
        _ => None,
    }
}

fn snapshot_name(value: Option<&Json>) -> Result<Name, Bad> {
    let Some(Json::List(groups)) = value else {
        return bad();
    };
    if groups.len() > 128 {
        return bad();
    }
    let mut out = Vec::new();
    for group in groups {
        let Json::List(members) = group else {
            return bad();
        };
        if members.is_empty() || members.len() > 32 {
            return bad();
        }
        let mut attributes = Vec::new();
        for attribute in members {
            let Json::List(parts) = attribute else {
                return bad();
            };
            if parts.len() != 3 {
                return bad();
            }
            let (Json::Text(identifier), Some(tag), Json::Text(value)) = (&parts[0], small_number(&parts[1], 255), &parts[2]) else {
                return bad();
            };
            if !(is_oid(identifier) && identifier.chars().count() <= 1024 && is_hexes(value)) {
                return bad();
            }
            attributes.push((identifier.clone(), tag, value.clone()));
        }
        out.push(attributes);
    }
    Ok(out)
}

fn read_facts(source: &str) -> Result<Facts, Bad> {
    let text: Vec<char> = source.chars().collect();
    if source.encode_utf16().count() > 4_194_304 {
        return bad();
    }
    let mut reader = Reader { text: &text, at: 0 };
    let Json::Map(map) = reader.value()? else {
        return bad();
    };
    let version_ok = match map.get("version") {
        Some(Json::Number(n)) => n == "1",
        Some(Json::Text(t)) => t == "1",
        _ => false,
    };
    let Some(Json::List(times)) = map.get("times") else {
        return bad();
    };
    let spki = match map.get("spki") {
        Some(Json::Text(spki)) if !spki.is_empty() && is_hexes(spki) => spki.clone(),
        _ => return bad(),
    };
    if !version_ok || times.len() != 2 {
        return bad();
    }
    let mut spelled = [String::new(), String::new()];
    for (index, time) in times.iter().enumerate() {
        let value = match time {
            Json::Number(n) => n.clone(),
            Json::Text(t) => t.clone(),
            _ => String::new(),
        };
        let digits = value.strip_prefix('-').unwrap_or(&value);
        if !((1..=12).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit())) {
            return bad();
        }
        spelled[index] = value;
    }
    let subject = snapshot_name(map.get("subject"))?;
    let issuer = snapshot_name(map.get("issuer"))?;
    let Some(Json::List(list)) = map.get("extensions") else {
        return bad();
    };
    if list.len() > 128 {
        return bad();
    }
    let mut extensions = Vec::new();
    for extension in list {
        let Json::List(parts) = extension else {
            return bad();
        };
        if parts.len() != 3 {
            return bad();
        }
        let (Json::Text(identifier), Some(critical), Json::Text(value)) = (&parts[0], small_number(&parts[1], 1), &parts[2]) else {
            return bad();
        };
        if !(is_oid(identifier) && identifier.chars().count() <= 1024 && is_hexes(value)) {
            return bad();
        }
        extensions.push((identifier.clone(), critical, value.clone()));
    }
    if reader.at != text.len() {
        reader.skip();
        if reader.at != text.len() {
            return bad();
        }
    }
    Ok(Facts {
        subject,
        issuer,
        times: spelled,
        spki,
        extensions,
    })
}

fn facts(native: Native, source: &str) -> Result<Facts, Error> {
    read_facts(source).map_err(|_| failure(DOMAIN, "invalid_input", native, "invalid private certificate snapshot"))
}

// ---- наблюдатели расширений ----

fn extension(facts: &Facts, oid: &str) -> Result<Option<Vec<u8>>, Bad> {
    let mut found = None;
    for (identifier, _, value) in &facts.extensions {
        if identifier == oid {
            if found.is_some() {
                return bad();
            }
            found = Some(unhex(value));
        }
    }
    Ok(found)
}

fn san(facts: &Facts) -> Result<Vec<(String, String)>, Bad> {
    let mut names = Vec::new();
    let Some(source) = extension(facts, "2.5.29.17")? else {
        return Ok(names);
    };
    let node = parse(&source)?;
    if node.tag != 48 || node.children.is_empty() {
        return bad();
    }
    if node.children.len() > 256 {
        return limit();
    }
    for name in &node.children {
        let number = name.number;
        if !(name.cls == 2 && number <= 8) {
            return bad();
        }
        let constructed = matches!(number, 0 | 3 | 4 | 5);
        if name.constructed != constructed {
            return bad();
        }
        let value = content(&source, name);
        if number == 2 {
            if value.is_empty() || value.iter().any(|b| !(0x20..=0x7e).contains(b)) {
                return bad();
            }
            if value.len() > 253 {
                return limit();
            }
            names.push(("DNS".to_string(), value.iter().map(|b| char::from(*b)).collect()));
        } else if number == 7 {
            if value.len() != 4 && value.len() != 16 {
                return bad();
            }
            names.push(("IP".to_string(), ip_text(value)));
        }
    }
    Ok(names)
}

/// [ca "0"/"1", глубина или ""].
fn constraints(facts: &Facts) -> Result<(bool, Option<i64>), Bad> {
    let Some(source) = extension(facts, "2.5.29.19")? else {
        return Ok((false, None));
    };
    let node = parse(&source)?;
    if node.tag != 48 {
        return bad();
    }
    let mut fields: Vec<&Node> = node.children.iter().collect();
    let mut ca = false;
    let mut depth = None;
    if fields.first().is_some_and(|field| field.tag == 1) {
        let flag = fields.remove(0);
        if content(&source, flag)[0] != 0xff {
            return bad();
        }
        ca = true;
    }
    if !fields.is_empty() {
        if !(fields.len() == 1 && fields[0].tag == 2 && ca) {
            return bad();
        }
        let encoded = content(&source, fields[0]);
        if encoded[0] & 128 != 0 {
            return bad();
        }
        if encoded.len() > 8 {
            return limit();
        }
        let value = encoded.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b));
        if value > i64::MAX as u64 {
            return limit();
        }
        depth = Some(value as i64);
    }
    Ok((ca, depth))
}

fn usages(facts: &Facts) -> Result<Vec<u32>, Bad> {
    let mut bits = Vec::new();
    if let Some(source) = extension(facts, "2.5.29.15")? {
        let node = parse(&source)?;
        if node.tag != 3 {
            return bad();
        }
        let value = content(&source, &node);
        let unused = u32::from(value[0]);
        let count = (value.len() as i64 - 1) * 8 - i64::from(unused);
        if !(count > 0 && count <= 9 && (value[value.len() - 1] & (1u8 << unused)) != 0) {
            return bad();
        }
        for index in 0..count as usize {
            if value[1 + index / 8] & (128 >> (index % 8)) != 0 {
                bits.push(index as u32);
            }
        }
    }
    Ok(bits)
}

fn purposes(facts: &Facts) -> Result<Vec<String>, Bad> {
    let mut oids = Vec::new();
    let Some(source) = extension(facts, "2.5.29.37")? else {
        return Ok(oids);
    };
    let node = parse(&source)?;
    if node.tag != 48 || node.children.is_empty() {
        return bad();
    }
    if node.children.len() > 128 {
        return limit();
    }
    for child in &node.children {
        if child.tag != 6 {
            return bad();
        }
        oids.push(oid(content(&source, child))?);
    }
    Ok(oids)
}

fn observation(native: Native, error: Bad, message: &str) -> Error {
    failure(DOMAIN, if error.0 == "limit" { "resource_exhausted" } else { "invalid_data" }, native, message)
}

fn units(bytes: &[u8], width: usize) -> Vec<u32> {
    bytes
        .chunks(width)
        .map(|chunk| chunk.iter().fold(0u32, |acc, b| (acc << 8) | u32::from(*b)))
        .collect()
}

/// Текст значения атрибута имени по тегу строки DER.
fn attribute_text(tag: u8, s: &[u8]) -> Result<String, Bad> {
    match tag {
        12 => String::from_utf8(s.to_vec()).map_err(|_| Bad("invalid")),
        19 => {
            let printable = |b: &u8| b.is_ascii_alphanumeric() || b" '()+,-./:=?".contains(b);
            if s.iter().all(printable) {
                Ok(s.iter().map(|b| char::from(*b)).collect())
            } else {
                bad()
            }
        }
        22 => {
            if s.iter().any(|b| *b >= 128) {
                return bad();
            }
            Ok(s.iter().map(|b| char::from(*b)).collect())
        }
        30 => {
            if s.len() % 2 != 0 {
                return bad();
            }
            units(s, 2)
                .into_iter()
                .map(|unit| char::from_u32(unit).ok_or(Bad("invalid")))
                .collect()
        }
        28 => {
            if s.len() % 4 != 0 {
                return bad();
            }
            units(s, 4)
                .into_iter()
                .map(|point| char::from_u32(point).ok_or(Bad("invalid")))
                .collect()
        }
        _ => Err(Bad("unsupported")),
    }
}

fn decoded_name(groups: &Name) -> Result<Value, Bad> {
    let mut result = Vec::new();
    let mut cost = 0usize;
    for group in groups {
        let mut members = Vec::new();
        for (identifier, tag, value) in group {
            let text = attribute_text(*tag, &unhex(value))?;
            let length = text.chars().count();
            if length > 4096 {
                return limit();
            }
            cost += identifier.chars().count() + length;
            if cost > 65536 {
                return limit();
            }
            members.push(string_array([identifier.clone(), text]));
        }
        result.push(array(members));
    }
    Ok(array(result))
}

// ---- декодирование ключа (`CertificateFactory` + `getPublicKey` JDK) ----

fn host_check(native: Native, der: &[u8]) -> Result<(), Error> {
    let parsed = certificate(der).map_err(|_| failure(DOMAIN, "invalid_data", native, "certificate or public key rejected by backend"))?;
    key_info(&parsed.facts)
        .map(|_| ())
        .map_err(|_| failure(DOMAIN, "invalid_data", native, "certificate or public key rejected by backend"))
}

/// Ключ сертификата: `rsa <bits>`, `ec <bits>`, `ed25519`, …
fn key_info(facts: &Facts) -> Result<String, ()> {
    let spki = unhex(&facts.spki);
    okkama_native::call("x509-key", &[&spki])
        .map(|text| String::from_utf8_lossy(&text).into_owned())
        .map_err(|_| ())
}

// ---- системные корни ----

fn root_files(native: Native) -> Result<Vec<String>, Error> {
    let environment = crate::host::environment();
    let file_override = environment.get("SSL_CERT_FILE").filter(|v| !v.is_empty());
    let directory_override = environment.get("SSL_CERT_DIR").filter(|v| !v.is_empty());
    let mut files = Vec::new();
    if let Some(file) = file_override {
        files.push(file.clone());
    } else {
        for candidate in [
            "/etc/ssl/cert.pem",
            "/etc/ssl/certs/ca-certificates.crt",
            "/etc/pki/tls/certs/ca-bundle.crt",
            "/etc/ssl/ca-bundle.pem",
            "/usr/local/etc/openssl/cert.pem",
            "/opt/homebrew/etc/openssl@3/cert.pem",
        ] {
            if std::path::Path::new(candidate).is_file() {
                files.push(candidate.to_string());
                break;
            }
        }
    }
    let directories: Vec<String> = match directory_override {
        // String.split(":") Java отбрасывает хвостовые пустые части.
        Some(list) => {
            let mut parts: Vec<String> = list.split(':').map(str::to_string).collect();
            while parts.last().is_some_and(String::is_empty) {
                parts.pop();
            }
            parts
        }
        None if files.is_empty() && std::path::Path::new("/etc/ssl/certs").is_dir() => vec!["/etc/ssl/certs".to_string()],
        None => Vec::new(),
    };
    if directories.len() > 32 {
        return Err(failure(DOMAIN, "resource_exhausted", native, "CA directory count limit"));
    }
    for directory in directories {
        let Ok(listing) = std::fs::read_dir(&directory) else {
            return Err(failure(DOMAIN, "not_found", native, "cannot open default CA directory"));
        };
        let mut entries: Vec<String> = listing.filter_map(|entry| entry.ok()).map(|entry| entry.file_name().to_string_lossy().into_owned()).collect();
        entries.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        for entry in entries {
            let (stem, suffix) = entry.split_once('.').unwrap_or((&entry, ""));
            let hashed = stem.len() == 8
                && stem.bytes().all(|b| b.is_ascii_hexdigit())
                && !suffix.is_empty()
                && suffix.bytes().all(|b| b.is_ascii_digit());
            if hashed {
                files.push(format!("{directory}/{entry}"));
                if files.len() > 2048 {
                    return Err(failure(DOMAIN, "resource_exhausted", native, "CA file count limit"));
                }
            }
        }
    }
    Ok(files)
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let value = chunk.iter().enumerate().fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        for index in 0..4 {
            if index <= chunk.len() {
                out.push(char::from(BASE64[((value >> (18 - 6 * index)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let trimmed = text.trim_end_matches('=');
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    for byte in trimmed.bytes() {
        let value = BASE64.iter().position(|b| *b == byte)? as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

/// `-----BEGIN ([A-Z0-9 -]+)-----` с позиции `from`: начало, метка, конец.
fn pem_begin(text: &[u8], from: usize) -> Option<(usize, String, usize)> {
    let marker = b"-----BEGIN ";
    let label = |b: u8| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b' ' || b == b'-';
    let mut at = from;
    while at + marker.len() <= text.len() {
        if text[at..].starts_with(marker) {
            let start = at + marker.len();
            let mut run = start;
            while run < text.len() && label(text[run]) {
                run += 1;
            }
            for end in (start + 1..=run).rev() {
                if text[end..].starts_with(b"-----") {
                    let kind = String::from_utf8_lossy(&text[start..end]).into_owned();
                    return Some((at, kind, end + 5));
                }
            }
        }
        at += 1;
    }
    None
}

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    (from..=haystack.len().saturating_sub(needle.len())).find(|&at| haystack[at..].starts_with(needle))
}

fn system_certificates(native: Native) -> Result<Value, Error> {
    let files = root_files(native)?;
    let mut roots = Vec::new();
    let mut seen = HashSet::new();
    let mut total = 0usize;
    for path in files {
        let file = crate::host::path(&path);
        if !file.is_file() {
            return Err(failure(DOMAIN, "invalid_input", native, "default CA source must be a regular file"));
        }
        let raw = std::fs::read(&file).map_err(|error| failure(DOMAIN, crate::host::code(&error), native, "cannot read default CA file"))?;
        total += raw.len();
        if raw.len() > 8_388_608 || total > 16_777_216 {
            return Err(failure(DOMAIN, "resource_exhausted", native, "CA source byte limit"));
        }
        let mut from = 0;
        while let Some((_, kind, end)) = pem_begin(&raw, from) {
            let closing = format!("-----END {kind}-----");
            let Some(close) = find(&raw, closing.as_bytes(), end) else {
                return Err(failure(DOMAIN, "invalid_data", native, "unclosed default CA PEM block"));
            };
            from = close + closing.len();
            if kind == "TRUSTED CERTIFICATE" {
                return Err(failure(DOMAIN, "unsupported", native, "auxiliary certificate trust cannot be flattened"));
            }
            if kind != "CERTIFICATE" && kind != "X509 CERTIFICATE" {
                continue;
            }
            let encoded: String = raw[end..close]
                .iter()
                .filter(|b| !matches!(b, b' ' | b'\t' | b'\r' | b'\n' | 0x0c))
                .map(|b| char::from(*b))
                .collect();
            let body = encoded.trim_end_matches('=');
            let valid = !encoded.is_empty()
                && encoded.len() % 4 == 0
                && encoded.len() - body.len() <= 2
                && body.bytes().all(|b| BASE64.contains(&b));
            if !valid {
                return Err(failure(DOMAIN, "invalid_data", native, "invalid default CA base64"));
            }
            let der = base64_decode(&encoded).unwrap_or_default();
            if base64_encode(&der) != encoded {
                return Err(failure(DOMAIN, "invalid_data", native, "noncanonical default CA base64"));
            }
            if der.len() > 1_048_576 {
                return Err(failure(DOMAIN, "resource_exhausted", native, "CA certificate byte limit"));
            }
            if !seen.insert(der.clone()) {
                continue;
            }
            if roots.len() >= 1024 {
                return Err(failure(DOMAIN, "resource_exhausted", native, "CA certificate count limit"));
            }
            if let Err(error) = certificate(&der) {
                let code = if error.0 == "limit" { "resource_exhausted" } else { "invalid_data" };
                return Err(failure(DOMAIN, code, native, "invalid bounded default CA certificate"));
            }
            host_check(native, &der)?;
            roots.push(byte_array(der));
        }
    }
    if roots.is_empty() {
        return Err(failure(DOMAIN, "not_found", native, "no actual default CA certificates"));
    }
    Ok(array(roots))
}

// ---- проверка цепочки ----

/// Исход проверки, не связанный с данными: `permission_denied`.
enum Stop {
    Rejected(&'static str),
    Bad(Bad),
    Unexpected,
}

impl From<Bad> for Stop {
    fn from(error: Bad) -> Self {
        Stop::Bad(error)
    }
}

/// Рациональное время `at`: числитель и положительный знаменатель.
fn rational(time: &str) -> (IntValue, IntValue) {
    let parse = |text: &str| IntValue::parse(text).unwrap_or_else(|| IntValue::from_i64(0));
    if let Some((numerator, denominator)) = time.split_once('/') {
        return (parse(numerator), parse(denominator));
    }
    if let Some((whole, fraction)) = time.split_once('.') {
        let mut scale = IntValue::from_i64(1);
        for _ in 0..fraction.len() {
            scale = integer::multiply(&scale, &IntValue::from_i64(10));
        }
        let whole_value = parse(whole);
        let part = parse(fraction);
        let part = if whole_value.is_negative() || time.starts_with('-') { part.negated() } else { part };
        return (integer::add(&integer::multiply(&whole_value, &scale), &part), scale);
    }
    (parse(time), IntValue::from_i64(1))
}

fn time_failure(facts: &Facts, at: &(IntValue, IntValue)) -> Option<&'static str> {
    let seconds = |text: &str| integer::multiply(&IntValue::parse(text).unwrap_or_else(|| IntValue::from_i64(0)), &at.1);
    if integer::compare(&at.0, &seconds(&facts.times[0])).is_lt() {
        return Some("not_yet_valid");
    }
    if integer::compare(&at.0, &seconds(&facts.times[1])).is_gt() {
        return Some("expired");
    }
    None
}

fn dns_matches(facts: &Facts, host: &str) -> Result<bool, Bad> {
    for (kind, value) in san(facts)? {
        if kind != "DNS" {
            continue;
        }
        let name = value.to_lowercase();
        if name == host {
            return Ok(true);
        }
        let Some(suffix) = name.strip_prefix("*.").filter(|suffix| !suffix.is_empty() && !suffix.contains('*')) else {
            continue;
        };
        let Ok((labels, width)) = suffix_parts(suffix) else {
            continue;
        };
        if labels.len() <= width || labels.join(".") != suffix {
            continue;
        }
        if let Some(dot) = host.find('.') {
            if dot > 0 && host[dot + 1..] == *suffix {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn ip_matches(facts: &Facts, address: &[u8]) -> Result<bool, Bad> {
    let wanted = ip_text(address);
    Ok(san(facts)?.into_iter().any(|(kind, value)| kind == "IP" && value == wanted))
}

const KNOWN_CRITICAL: [&str; 11] = [
    "2.5.29.15", "2.5.29.17", "2.5.29.19", "2.5.29.37", "2.5.29.30", "2.5.29.32", "2.5.29.36", "2.5.29.54", "2.5.29.35",
    "2.5.29.14", "2.5.29.33",
];

fn critical(facts: &Facts) -> bool {
    facts.extensions.iter().any(|(oid, critical, _)| *critical == 1 && !KNOWN_CRITICAL.contains(&oid.as_str()))
}

fn present(facts: &Facts, oid: &str) -> bool {
    facts.extensions.iter().any(|(identifier, _, _)| identifier == oid)
}

fn critical_extension(facts: &Facts, oid: &str) -> bool {
    facts
        .extensions
        .iter()
        .find(|(identifier, _, _)| identifier == oid)
        .is_some_and(|(_, critical, _)| *critical == 1)
}

/// Каноническое имя X500Principal (RFC 2253 canonical JDK): группы как
/// множества `oid=значение`, строки — NFKD, нижний регистр, пробелы свёрнуты.
fn canonical(name: &Name) -> Vec<Vec<String>> {
    name.iter()
        .map(|group| {
            let mut members: Vec<String> = group
                .iter()
                .map(|(identifier, tag, value)| {
                    let raw = unhex(value);
                    let text = match tag {
                        20 | 27 => Some(raw.iter().map(|b| char::from(*b)).collect::<String>()),
                        12 | 19 | 22 | 30 | 28 => attribute_text(*tag, &raw).ok(),
                        _ => None,
                    };
                    match text {
                        Some(text) => {
                            let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
                            let lowered = collapsed.to_uppercase().to_lowercase();
                            format!("{identifier}={}", lowered.nfkd().collect::<String>())
                        }
                        None => {
                            let mut der = vec![*tag];
                            if raw.len() < 128 {
                                der.push(raw.len() as u8);
                            } else {
                                let length = (raw.len() as u32).to_be_bytes();
                                let skip = length.iter().take_while(|b| **b == 0).count();
                                der.push(0x80 | (4 - skip) as u8);
                                der.extend(&length[skip..]);
                            }
                            der.extend(&raw);
                            format!("{identifier}=#{}", hex(&der))
                        }
                    }
                })
                .collect();
            members.sort();
            members
        })
        .collect()
}

/// Сертификат пути со всем, что нужно построителю.
struct Candidate {
    der: Vec<u8>,
    parsed: Parsed,
    subject: Vec<Vec<String>>,
    issuer: Vec<Vec<String>>,
    ski: Option<Vec<u8>>,
    aki: Option<Vec<u8>>,
}

impl Candidate {
    fn new(der: Vec<u8>, parsed: Parsed) -> Self {
        let subject = canonical(&parsed.facts.subject);
        let issuer = canonical(&parsed.facts.issuer);
        let ski = extension(&parsed.facts, "2.5.29.14")
            .ok()
            .flatten()
            .and_then(|source| parse(&source).ok().filter(|node| node.tag == 4).map(|node| content(&source, &node).to_vec()));
        let aki = extension(&parsed.facts, "2.5.29.35").ok().flatten().and_then(|source| {
            let node = parse(&source).ok()?;
            let key = node.children.iter().find(|child| child.tag == 0x80)?;
            Some(content(&source, key).to_vec())
        });
        Self {
            der,
            parsed,
            subject,
            issuer,
            ski,
            aki,
        }
    }

    fn self_issued(&self) -> bool {
        self.subject == self.issuer
    }

    fn facts(&self) -> &Facts {
        &self.parsed.facts
    }

    /// Подпись этого сертификата ключом `issuer` (`X509Certificate.verify`).
    fn signed_by(&self, issuer: &Candidate) -> bool {
        let spki = unhex(&issuer.parsed.facts.spki);
        matches!(
            okkama_native::call("x509-verify", &[&spki, &self.parsed.algorithm, &self.parsed.tbs, &self.parsed.signature]),
            Ok(result) if result == [1]
        )
    }
}

fn self_signed(candidate: &Candidate) -> bool {
    candidate.self_issued() && candidate.signed_by(candidate)
}

/// Ограничения AlgorithmChecker JDK (`jdk.certpath.disabledAlgorithms`).
fn algorithm_permitted(candidate: &Candidate) -> bool {
    let (algorithm, _) = signature_algorithm(&candidate.parsed.algorithm);
    if matches!(algorithm.as_str(), "1.2.840.113549.1.1.2" | "1.2.840.113549.1.1.4") {
        return false;
    }
    key_permitted(candidate)
}

fn key_permitted(candidate: &Candidate) -> bool {
    let Ok(info) = key_info(candidate.facts()) else {
        return false;
    };
    let bits = |prefix: &str| info.strip_prefix(prefix).and_then(|bits| bits.parse::<u32>().ok());
    if let Some(size) = bits("rsa ") {
        return size >= 1024;
    }
    if let Some(size) = bits("ec ") {
        return size >= 224;
    }
    true
}

fn signature_algorithm(der: &[u8]) -> (String, ()) {
    let identifier = parse(der)
        .ok()
        .and_then(|node| node.children.first().cloned())
        .and_then(|first| oid(content(der, &first)).ok())
        .unwrap_or_default();
    (identifier, ())
}

/// Расширения, которые понимают checkers PKIX JDK.
const PKIX_SUPPORTED: [&str; 9] = [
    "2.5.29.15", "2.5.29.37", "2.5.29.17", "2.5.29.19", "2.5.29.30", "2.5.29.32", "2.5.29.33", "2.5.29.36", "2.5.29.54",
];

/// Имена сертификата для NameConstraints: тип GeneralName и значение.
#[derive(Debug, Clone)]
enum General {
    Dns(String),
    Ip(Vec<u8>),
    Directory(Vec<Vec<String>>),
    Email(String),
}

#[derive(Debug, Clone, Default)]
struct Subtrees {
    permitted: Vec<General>,
    excluded: Vec<General>,
    /// Типы, у которых есть permitted-поддеревья (в т. ч. неподдержанные).
    permitted_kinds: HashSet<u32>,
}

fn general_name(source: &[u8], node: &Node) -> Option<General> {
    let value = content(source, node);
    match (node.cls, node.number) {
        (2, 1) => Some(General::Email(String::from_utf8_lossy(value).to_lowercase())),
        (2, 2) => Some(General::Dns(String::from_utf8_lossy(value).to_lowercase())),
        (2, 7) => Some(General::Ip(value.to_vec())),
        (2, 4) => {
            let inner = node.children.first()?;
            let name = name(source, inner).ok()?;
            Some(General::Directory(canonical(&name)))
        }
        _ => None,
    }
}

fn name_constraints(facts: &Facts) -> Result<Option<Subtrees>, Bad> {
    let Some(source) = extension(facts, "2.5.29.30")? else {
        return Ok(None);
    };
    let node = parse(&source)?;
    let mut trees = Subtrees::default();
    for part in &node.children {
        for subtree in &part.children {
            let Some(base) = subtree.children.first() else {
                return bad();
            };
            let general = general_name(&source, base);
            if part.tag == 0xa0 {
                trees.permitted_kinds.insert(base.number);
                if let Some(general) = general {
                    trees.permitted.push(general);
                }
            } else if let Some(general) = general {
                trees.excluded.push(general);
            }
        }
    }
    Ok(Some(trees))
}

fn dns_within(name: &str, base: &str) -> bool {
    name == base || (name.len() > base.len() && name.ends_with(base) && name.as_bytes()[name.len() - base.len() - 1] == b'.')
}

fn within(name: &General, base: &General) -> Option<bool> {
    match (name, base) {
        (General::Dns(name), General::Dns(base)) => Some(base.is_empty() || dns_within(name, base)),
        (General::Ip(address), General::Ip(range)) => {
            if range.len() != address.len() * 2 {
                return Some(false);
            }
            let (network, mask) = range.split_at(address.len());
            Some(address.iter().zip(network).zip(mask).all(|((a, n), m)| a & m == n & m))
        }
        (General::Directory(name), General::Directory(base)) => Some(base.len() <= name.len() && name[..base.len()] == base[..]),
        (General::Email(mailbox), General::Email(base)) => {
            let host = mailbox.rsplit_once('@').map_or(mailbox.as_str(), |(_, host)| host);
            Some(if base.contains('@') {
                mailbox == base
            } else if base.starts_with('.') {
                host.ends_with(base.as_str())
            } else {
                host == base
            })
        }
        _ => None,
    }
}

fn kind_of(name: &General) -> u32 {
    match name {
        General::Email(_) => 1,
        General::Dns(_) => 2,
        General::Directory(_) => 4,
        General::Ip(_) => 7,
    }
}

fn names_for_constraints(candidate: &Candidate) -> Vec<General> {
    let mut out = Vec::new();
    if !candidate.facts().subject.is_empty() {
        out.push(General::Directory(candidate.subject.clone()));
    }
    let mut has_dns = false;
    if let Ok(Some(source)) = extension(candidate.facts(), "2.5.29.17") {
        if let Ok(node) = parse(&source) {
            for child in &node.children {
                if let Some(general) = general_name(&source, child) {
                    has_dns |= matches!(general, General::Dns(_));
                    out.push(general);
                }
            }
        }
    }
    for group in &candidate.facts().subject {
        for (identifier, tag, value) in group {
            let Ok(text) = attribute_text(*tag, &unhex(value)) else {
                continue;
            };
            if identifier == "1.2.840.113549.1.9.1" {
                out.push(General::Email(text.to_lowercase()));
            }
            // Как JDK: без SAN dNSName имя хоста из CN проверяется как DNS/IP.
            if identifier == "2.5.4.3" && !has_dns {
                if let Some(address) = parse_ipv4(&text) {
                    out.push(General::Ip(address.to_vec()));
                } else if hostname_like(&text) {
                    out.push(General::Dns(text.to_lowercase()));
                }
            }
        }
    }
    out
}

fn hostname_like(text: &str) -> bool {
    !text.is_empty()
        && text.split('.').all(|label| {
            !label.is_empty()
                && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                && !label.starts_with('-')
                && !label.ends_with('-')
        })
}

fn satisfies(names: &[General], trees: &Subtrees) -> bool {
    for name in names {
        if trees.excluded.iter().any(|base| within(name, base) == Some(true)) {
            return false;
        }
        if trees.permitted_kinds.contains(&kind_of(name)) && !trees.permitted.iter().any(|base| within(name, base) == Some(true)) {
            return false;
        }
    }
    true
}

/// Проверка пути PKIX (BasicChecker, AlgorithmChecker, KeyChecker,
/// ConstraintsChecker, критические расширения): `path` — от листа к якорю
/// не включительно, `anchor` — якорь.
fn valid_path(pool: &[Candidate], path: &[usize], anchor: usize) -> Result<bool, Bad> {
    if !key_permitted(&pool[anchor]) {
        return Ok(false);
    }
    let length = path.len();
    let mut maximum = length as i64;
    let mut constraints: Vec<Subtrees> = Vec::new();
    let mut previous = anchor;
    for step in 1..=length {
        let current = &pool[path[length - step]];
        if current.issuer != pool[previous].subject || !current.signed_by(&pool[previous]) || !algorithm_permitted(current) {
            return Ok(false);
        }
        let facts = current.facts();
        if step < length {
            if let Ok(usage) = usages(facts) {
                if present(facts, "2.5.29.15") && !usage.contains(&5) {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
            let path_length = if current.parsed.version < 2 {
                if step == 1 && current.self_issued() { i64::MAX } else { -1 }
            } else {
                match constraints_of(facts)? {
                    (true, Some(depth)) => depth,
                    (true, None) => i64::MAX,
                    (false, _) => -1,
                }
            };
            if path_length == -1 {
                return Ok(false);
            }
            if !current.self_issued() {
                if maximum <= 0 {
                    return Ok(false);
                }
                maximum -= 1;
            }
            maximum = maximum.min(path_length);
        }
        if step == length || !current.self_issued() {
            let names = names_for_constraints(current);
            if constraints.iter().any(|trees| !satisfies(&names, trees)) {
                return Ok(false);
            }
        }
        if let Some(trees) = name_constraints(facts)? {
            constraints.push(trees);
        }
        let unresolved = facts
            .extensions
            .iter()
            .any(|(oid, critical, _)| *critical == 1 && !PKIX_SUPPORTED.contains(&oid.as_str()));
        if unresolved {
            return Ok(false);
        }
        previous = path[length - step];
    }
    Ok(true)
}

fn constraints_of(facts: &Facts) -> Result<(bool, Option<i64>), Bad> {
    constraints(facts)
}

/// Кандидаты-издатели `current`: имя издателя и AKI/SKI, якоря первыми.
fn issuers(pool: &[Candidate], current: usize, anchors: &[usize], intermediates: &[usize]) -> Vec<usize> {
    let wanted = &pool[current].issuer;
    let aki = pool[current].aki.as_ref();
    anchors
        .iter()
        .chain(intermediates)
        .copied()
        .filter(|&index| {
            index != current
                && pool[index].subject == *wanted
                && match (aki, &pool[index].ski) {
                    (Some(aki), Some(ski)) => aki == ski,
                    _ => true,
                }
        })
        .collect()
}

/// Поиск в глубину от листа к якорю (`SunCertPathBuilder`).
fn search(
    pool: &[Candidate],
    path: &mut Vec<usize>,
    anchors: &[usize],
    intermediates: &[usize],
    found: &mut Option<(Vec<usize>, usize)>,
) -> Result<(), Bad> {
    if found.is_some() || path.len() > 30 {
        return Ok(());
    }
    let current = *path.last().expect("leaf");
    for candidate in issuers(pool, current, anchors, intermediates) {
        if anchors.contains(&candidate) {
            if valid_path(pool, path, candidate)? {
                *found = Some((path.clone(), candidate));
                return Ok(());
            }
            continue;
        }
        if path.iter().any(|&known| pool[known].der == pool[candidate].der) {
            continue;
        }
        path.push(candidate);
        search(pool, path, anchors, intermediates, found)?;
        path.pop();
        if found.is_some() {
            return Ok(());
        }
    }
    Ok(())
}

/// Причина без пути: подпись найденного по имени издателя или недоверие.
fn reason(pool: &[Candidate], leaf: usize, all: &[usize], anchors: &[usize]) -> &'static str {
    let mut current = leaf;
    for _ in 0..32 {
        let mut named = false;
        let mut next = None;
        for &candidate in all {
            if candidate == current || pool[candidate].subject != pool[current].issuer {
                continue;
            }
            named = true;
            if pool[current].signed_by(&pool[candidate]) {
                next = Some(candidate);
                break;
            }
        }
        let Some(next) = next else {
            return if named { "invalid_signature" } else { "untrusted" };
        };
        if anchors.contains(&next) {
            return "invalid_chain";
        }
        current = next;
    }
    "invalid_chain"
}

/// Профиль RFC 5280 под `X509_V_FLAG_X509_STRICT` OpenSSL.
fn strict(candidate: &Candidate) -> Result<(), Stop> {
    let facts = candidate.facts();
    let (ca, _) = constraints(facts)?;
    if ca && (!critical_extension(facts, "2.5.29.19") || !present(facts, "2.5.29.15") || !present(facts, "2.5.29.14")) {
        return Err(Stop::Rejected("invalid_chain"));
    }
    if !candidate.self_issued() && !present(facts, "2.5.29.35") {
        return Err(Stop::Rejected("invalid_chain"));
    }
    if facts.subject.is_empty() && !critical_extension(facts, "2.5.29.17") {
        return Err(Stop::Rejected("invalid_chain"));
    }
    Ok(())
}

fn purpose(facts: &Facts, wanted: &str, leaf: bool) -> Result<(), Stop> {
    let eku = purposes(facts)?;
    if !eku.is_empty() && !eku.iter().any(|oid| oid == wanted || oid == "2.5.29.37.0") {
        return Err(Stop::Rejected("invalid_purpose"));
    }
    let usage = usages(facts)?;
    if leaf {
        if !usage.is_empty() {
            let signature = usage.contains(&0);
            let encipher = usage.contains(&2);
            let agreement = usage.contains(&4);
            let server = wanted == "1.3.6.1.5.5.7.3.1";
            if !(signature || (server && (encipher || agreement))) {
                return Err(Stop::Rejected("invalid_purpose"));
            }
        }
    } else {
        if !constraints(facts)?.0 {
            return Err(Stop::Rejected("invalid_chain"));
        }
        if !usage.is_empty() && !usage.contains(&5) {
            return Err(Stop::Rejected("invalid_chain"));
        }
    }
    Ok(())
}

fn verify_failure(native: Native, code: &str, message: &str) -> Error {
    raise(VERIFY, code, native.symbol(), message)
}

fn time_shape(time: &str) -> bool {
    let body = time.strip_prefix('-').unwrap_or(time);
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    if let Some((whole, fraction)) = body.split_once('.') {
        return digits(whole) && digits(fraction);
    }
    if let Some((whole, denominator)) = body.split_once('/') {
        return digits(whole) && digits(denominator) && !denominator.starts_with('0');
    }
    digits(body)
}

fn verify_chain(native: Native, args: &[Value]) -> Result<Value, Error> {
    let chain: Vec<Vec<u8>> = items(&args[0])?.iter().map(bytes).collect::<Result<_, _>>()?;
    let roots: Vec<Vec<u8>> = items(&args[1])?.iter().map(bytes).collect::<Result<_, _>>()?;
    let (kind, host, time, wanted) = (string(&args[2])?, string(&args[3])?, string(&args[4])?, string(&args[5])?);
    if chain.len() > 32 || roots.len() > 1024 {
        return Err(verify_failure(native, "resource_exhausted", "verification certificate count limit"));
    }
    let mut total = 0usize;
    for (group, cap) in [(&chain, 8_388_608usize), (&roots, 16_777_216usize)] {
        let mut sum = 0usize;
        for der in group {
            total += der.len();
            sum += der.len();
            if der.len() > 1_048_576 || total > 25_165_824 || sum > cap {
                return Err(verify_failure(native, "resource_exhausted", "verification DER byte limit"));
            }
        }
    }
    let host_shape = host.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b':' | b'-'));
    if chain.is_empty()
        || time.encode_utf16().count() > 1024
        || !time_shape(time)
        || host.is_empty()
        || host.encode_utf16().count() > 253
        || !host_shape
        || !(kind == "DNS" || kind == "IP")
    {
        return Err(verify_failure(native, "invalid_input", "invalid verification inputs"));
    }
    if wanted != "1.3.6.1.5.5.7.3.1" && wanted != "1.3.6.1.5.5.7.3.2" {
        return Err(verify_failure(native, "unsupported", "unsupported certificate purpose"));
    }
    let mut reference = host.to_string();
    let mut address = Vec::new();
    if kind == "DNS" {
        reference = hostname_ascii(host).map_err(|_| verify_failure(native, "invalid_input", "invalid DNS reference identity"))?;
        if let Some(stripped) = reference.strip_suffix('.') {
            reference = stripped.to_string();
        }
    } else {
        let parsed = if host.contains(':') { parse_ipv6(host).map(|a| a.to_vec()) } else { parse_ipv4(host).map(|a| a.to_vec()) };
        address = parsed.ok_or_else(|| verify_failure(native, "invalid_data", "invalid certificate verification data"))?;
    }
    let at = rational(time);
    match verified(&chain, &roots, kind, &reference, &address, &at, wanted) {
        Ok(path) => Ok(array(path.into_iter().map(byte_array).collect())),
        Err(Stop::Rejected(reason)) => Err(verify_failure(native, "permission_denied", reason)),
        Err(Stop::Bad(error)) if error.0 == "limit" => Err(verify_failure(native, "resource_exhausted", "verification limit")),
        Err(Stop::Bad(_)) => Err(verify_failure(native, "invalid_data", "invalid certificate verification data")),
        Err(Stop::Unexpected) => Err(verify_failure(native, "unexpected", "verification backend failure")),
    }
}

fn verified(
    chain: &[Vec<u8>],
    roots: &[Vec<u8>],
    kind: &str,
    host: &str,
    address: &[u8],
    at: &(IntValue, IntValue),
    wanted: &str,
) -> Result<Vec<Vec<u8>>, Stop> {
    let mut pool: Vec<Candidate> = Vec::new();
    let mut leaf = None;
    let mut intermediates = Vec::new();
    let mut anchors = Vec::new();
    let mut seen = HashSet::new();
    for (group, items) in [chain, roots].into_iter().enumerate() {
        for (index, der) in items.iter().enumerate() {
            let parsed = certificate(der)?;
            if let Some(failed) = time_failure(&parsed.facts, at) {
                if group == 0 && index == 0 {
                    return Err(Stop::Rejected(failed));
                }
                continue;
            }
            // CertificateFactory JDK: ключ должен декодироваться.
            if key_info(&parsed.facts).is_err() {
                return Err(Stop::Bad(Bad("invalid")));
            }
            let candidate = Candidate::new(der.clone(), parsed);
            let position = pool.len();
            if group == 1 {
                // OpenSSL без PARTIAL_CHAIN: якорь — самоподписанный корень;
                // прочие корни лишь достраивают путь.
                if seen.insert(der.clone()) {
                    if self_signed(&candidate) {
                        anchors.push(position);
                    } else {
                        intermediates.push(position);
                    }
                    pool.push(candidate);
                }
                continue;
            }
            if index == 0 {
                let matched = if kind == "DNS" { dns_matches(candidate.facts(), host)? } else { ip_matches(candidate.facts(), address)? };
                if !matched {
                    return Err(Stop::Rejected("invalid_name"));
                }
                if critical(candidate.facts()) {
                    return Err(Stop::Rejected("unknown_critical_extension"));
                }
                leaf = Some(position);
            } else {
                intermediates.push(position);
            }
            pool.push(candidate);
        }
    }
    let Some(leaf) = leaf else {
        return Err(Stop::Unexpected);
    };
    if anchors.is_empty() {
        return Err(Stop::Rejected("untrusted"));
    }
    let mut path: Vec<usize> = Vec::new();
    if anchors.iter().any(|&anchor| pool[anchor].der == pool[leaf].der) {
        path.push(leaf);
    } else {
        let mut found = None;
        let mut start = vec![leaf];
        search(&pool, &mut start, &anchors, &intermediates, &mut found)?;
        match found {
            Some((built, anchor)) => {
                path.extend(built);
                path.push(anchor);
            }
            None => {
                let all: Vec<usize> = intermediates.iter().chain(&anchors).copied().collect();
                return Err(Stop::Rejected(reason(&pool, leaf, &all, &anchors)));
            }
        }
    }
    if path.len() > 32 {
        return Err(Stop::Bad(Bad("limit")));
    }
    let mut out = Vec::new();
    let mut path_bytes = 0usize;
    for (index, &position) in path.iter().enumerate() {
        let candidate = &pool[position];
        path_bytes += candidate.der.len();
        if path_bytes > 8_388_608 {
            return Err(Stop::Bad(Bad("limit")));
        }
        if let Some(failed) = time_failure(candidate.facts(), at) {
            return Err(Stop::Rejected(failed));
        }
        if index < path.len() - 1 {
            let facts = candidate.facts();
            constraints(facts)?;
            usages(facts)?;
            purposes(facts)?;
            san(facts)?;
            if critical(facts) {
                return Err(Stop::Rejected("unknown_critical_extension"));
            }
            strict(candidate)?;
            purpose(facts, wanted, index == 0)?;
        }
        out.push(candidate.der.clone());
    }
    Ok(out)
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    let name = native.name();
    match name {
        "certificate_facts" => {
            let origin = items(&args[0])?;
            if origin.len() > 1_048_576 {
                return Err(failure(DOMAIN, "resource_exhausted", native, "certificate byte limit"));
            }
            let der = bytes(&args[0])?;
            let parsed = certificate(&der).map_err(|error| observation(native, error, "invalid bounded DER certificate"))?;
            host_check(native, &der)?;
            Ok(text(json(&parsed.facts)))
        }
        "certificate_times" => {
            let facts = facts(native, string(&args[0])?)?;
            Ok(string_array(facts.times))
        }
        "certificate_spki" => Ok(byte_array(unhex(&facts(native, string(&args[0])?)?.spki))),
        "certificate_names" => {
            let facts = facts(native, string(&args[0])?)?;
            let groups = if args[1].as_bool()? { &facts.issuer } else { &facts.subject };
            decoded_name(groups).map_err(|error| {
                let code = match error.0 {
                    "unsupported" => "unsupported",
                    "limit" => "resource_exhausted",
                    _ => "invalid_data",
                };
                failure(DOMAIN, code, native, "certificate name cannot be decoded")
            })
        }
        "certificate_san" => {
            let facts = facts(native, string(&args[0])?)?;
            let names = san(&facts).map_err(|error| observation(native, error, "invalid bounded certificate subjectAltName"))?;
            Ok(array(names.into_iter().map(|(kind, value)| string_array([kind, value])).collect()))
        }
        "certificate_constraints" => {
            let facts = facts(native, string(&args[0])?)?;
            let (ca, depth) = constraints(&facts).map_err(|error| observation(native, error, "invalid bounded certificate extension"))?;
            Ok(string_array([if ca { "1" } else { "0" }.to_string(), depth.map(|d| d.to_string()).unwrap_or_default()]))
        }
        "certificate_key_usages" => {
            let facts = facts(native, string(&args[0])?)?;
            let bits = usages(&facts).map_err(|error| observation(native, error, "invalid bounded certificate extension"))?;
            Ok(array(bits.into_iter().map(|bit| Value::Int(IntValue::from_i64(i64::from(bit)))).collect()))
        }
        "certificate_purposes" => {
            let facts = facts(native, string(&args[0])?)?;
            let oids = purposes(&facts).map_err(|error| observation(native, error, "invalid bounded certificate extension"))?;
            Ok(string_array(oids))
        }
        "certificate_extensions" => {
            let facts = facts(native, string(&args[0])?)?;
            Ok(array(
                facts
                    .extensions
                    .into_iter()
                    .map(|(oid, critical, value)| string_array([oid, critical.to_string(), value]))
                    .collect(),
            ))
        }
        "system_certificates" => system_certificates(native),
        "verify_chain" => verify_chain(native, args),
        _ => Err(Error::Unsupported(format!("native provider is not implemented: {}", native.symbol()))),
    }
}
