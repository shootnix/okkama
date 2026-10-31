//! `runtime/time_format` (шаблоны `%Y-%m-%d …`) и `runtime/time_zone`
//! (встроенный tzdb 2026c, `tzdb.dat`) — перенос `TimeNatives` JVM, включая
//! правило POSIX после последнего перехода.
use super::support::{array, failed, int, int_value, java_long, string, text};
use super::Error;
use crate::{IntValue, Value, abi::Native};
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

// ===================== runtime/time_format =====================

fn width(directive: char) -> i64 {
    match directive {
        'Y' => 4,
        'm' | 'd' | 'H' | 'M' | 'S' => 2,
        'f' => 9,
        _ => -1,
    }
}

/// Литерал или поле, смещение начала.
enum Token {
    Literal(String),
    Field(char, usize),
}

fn tokens(pattern: &str) -> Result<Vec<Token>, usize> {
    let text: Vec<char> = pattern.chars().collect();
    let mut out = Vec::new();
    let mut index = 0;
    while index < text.len() {
        if text[index] != '%' {
            let start = index;
            while index < text.len() && text[index] != '%' {
                index += 1;
            }
            out.push(Token::Literal(text[start..index].iter().collect()));
            continue;
        }
        let Some(directive) = text.get(index + 1).copied() else {
            return Err(index);
        };
        if directive == '%' {
            out.push(Token::Literal("%".into()));
        } else if width(directive) > 0 {
            out.push(Token::Field(directive, index));
        } else {
            return Err(index);
        }
        index += 2;
    }
    Ok(out)
}

fn pattern_invalid_offset(pattern: &str, local: bool, parsing: bool) -> i64 {
    let tokens = match tokens(pattern) {
        Ok(tokens) => tokens,
        Err(offset) => return offset as i64,
    };
    let mut seen = std::collections::HashSet::new();
    for token in &tokens {
        if let Token::Field(field, at) = token {
            if !local && "HMSf".contains(*field) {
                return *at as i64;
            }
            seen.insert(*field);
        }
    }
    if parsing {
        let required: &[char] = if local {
            &['Y', 'm', 'd', 'H', 'M', 'S']
        } else {
            &['Y', 'm', 'd']
        };
        if required.iter().any(|field| !seen.contains(field)) {
            return pattern.chars().count() as i64;
        }
    }
    -1
}

fn format(native: Native, pattern: &str, values: &HashMap<char, IntValue>) -> Result<String, Error> {
    let tokens = tokens(pattern).map_err(|_| failed(native))?;
    let mut out = String::new();
    for token in tokens {
        match token {
            Token::Literal(literal) => out.push_str(&literal),
            Token::Field(field, _) => {
                let value = values.get(&field).ok_or_else(|| failed(native))?;
                let negative = value.is_negative();
                let digits = if negative { value.negated() } else { value.clone() }.to_string();
                let size = width(field) - i64::from(negative);
                out.push_str(if negative { "-" } else { "" });
                for _ in digits.len() as i64..size {
                    out.push('0');
                }
                out.push_str(&digits);
            }
        }
    }
    Ok(out)
}

fn parse(origin: &str, pattern: &str) -> Result<HashMap<char, IntValue>, usize> {
    let origin: Vec<char> = origin.chars().collect();
    let tokens = tokens(pattern).map_err(|_| 0usize)?;
    let mut offset = 0;
    let mut values: HashMap<char, IntValue> = HashMap::new();
    for token in tokens {
        match token {
            Token::Literal(literal) => {
                for point in literal.chars() {
                    if origin.get(offset) != Some(&point) {
                        return Err(offset);
                    }
                    offset += 1;
                }
            }
            Token::Field(field, _) => {
                let size = width(field) as usize;
                if offset + size > origin.len() {
                    return Err(offset);
                }
                let digits: String = origin[offset..offset + size].iter().collect();
                if !digits.chars().all(|c| c.is_ascii_digit()) {
                    return Err(offset);
                }
                let value = IntValue::from_decimal_digits(&digits).expect("digits");
                if values.get(&field).is_some_and(|known| *known != value) {
                    return Err(offset);
                }
                values.insert(field, value);
                offset += size;
            }
        }
    }
    if offset != origin.len() {
        return Err(offset);
    }
    values.entry('f').or_insert_with(|| IntValue::from_i64(0));
    Ok(values)
}

// ===================== runtime/time_zone =====================

const VERSION: &str = "2026c";

fn little16(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes([bytes[at], bytes[at + 1]]))
}

fn little32(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes")) as usize
}

/// Файлы архива tzdb: центральный каталог zip, deflate или без сжатия.
fn files() -> &'static HashMap<String, Vec<u8>> {
    static FILES: std::sync::OnceLock<HashMap<String, Vec<u8>>> = std::sync::OnceLock::new();
    FILES.get_or_init(|| {
        let archive = crate::data::time_zone_archive();
        let end = (0..archive.len() - 21)
            .rev()
            .find(|at| archive[*at..*at + 4] == [0x50, 0x4b, 0x05, 0x06])
            .expect("zip end record");
        let count = little16(archive, end + 10);
        let mut at = little32(archive, end + 16);
        let mut files = HashMap::new();
        for _ in 0..count {
            let method = little16(archive, at + 10);
            let compressed = little32(archive, at + 20);
            let name_length = little16(archive, at + 28);
            let extra = little16(archive, at + 30);
            let comment = little16(archive, at + 32);
            let local = little32(archive, at + 42);
            let name = String::from_utf8_lossy(&archive[at + 46..at + 46 + name_length]).to_string();
            at += 46 + name_length + extra + comment;
            if name.ends_with('/') {
                continue;
            }
            let data_at = local + 30 + little16(archive, local + 26) + little16(archive, local + 28);
            let data = &archive[data_at..data_at + compressed];
            let content = match method {
                0 => data.to_vec(),
                _ => miniz_oxide::inflate::decompress_to_vec(data).expect("deflate entry"),
            };
            let name = name.strip_prefix("./").unwrap_or(&name).to_string();
            files.insert(name, content);
        }
        files
    })
}

fn zone_name_valid(name: &str) -> bool {
    let part = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'+' | b'.' | b'-'))
    };
    name.split('/').all(part) && !name.split('/').any(|part| part == "..")
}

fn zone_file(name: &str) -> Option<&'static Vec<u8>> {
    zone_name_valid(name).then(|| files().get(name)).flatten()
}

#[derive(Clone, Debug, Default)]
struct Type {
    offset: i64,
    daylight: bool,
    abbreviation: String,
}

#[derive(Debug)]
struct Rule {
    month: i64,
    week: i64,
    weekday: i64,
    seconds: i64,
}

#[derive(Debug)]
struct Footer {
    standard_name: String,
    standard_offset: i64,
    daylight_name: Option<String>,
    daylight_offset: i64,
    start: Option<Rule>,
    end: Option<Rule>,
}

#[derive(Debug)]
struct Zone {
    transitions: Vec<i64>,
    indices: Vec<usize>,
    types: Vec<Type>,
    footer: Option<Footer>,
}

type Checked<T> = Result<T, Error>;

fn trap(message: &str) -> Error {
    Error::Trap(message.into())
}

fn header(bytes: &[u8], offset: usize) -> Checked<[usize; 7]> {
    if bytes.get(offset..offset + 4) != Some(b"TZif") {
        return Err(trap("Invalid bundled TZif header"));
    }
    let mut counts = [0usize; 7];
    counts[0] = usize::from(bytes[offset + 4]);
    for index in 0..6 {
        let at = offset + 20 + index * 4;
        counts[index + 1] = u32::from_be_bytes(bytes[at..at + 4].try_into().expect("four bytes")) as usize;
    }
    Ok(counts)
}

fn block_size(time_size: usize, counts: &[usize; 7]) -> usize {
    let [_, gmt, std, leap, time, kind, chars] = *counts;
    time * time_size + time + kind * 6 + chars + leap * (time_size + 4) + std + gmt
}

fn clock_seconds(text: &str) -> Checked<i64> {
    let (sign, text) = match text.strip_prefix(['+', '-']) {
        Some(rest) => (if text.starts_with('-') { -1 } else { 1 }, rest),
        None => (1, text),
    };
    let parts: Vec<&str> = text.split(':').collect();
    if parts.is_empty() || parts.len() > 3 || parts.iter().any(|part| part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit())) {
        return Err(trap("Invalid bundled POSIX clock value"));
    }
    let number = |index: usize| parts.get(index).map_or(0, |part| part.parse::<i64>().unwrap_or(0));
    Ok(sign * (number(0) * 3600 + number(1) * 60 + number(2)))
}

fn take_name(text: &mut &str) -> Checked<String> {
    if let Some(rest) = text.strip_prefix('<') {
        if let Some(end) = rest.find('>').filter(|end| *end > 0) {
            let name = rest[..end].to_string();
            *text = &rest[end + 1..];
            return Ok(name);
        }
    }
    let length = text.bytes().take_while(u8::is_ascii_alphabetic).count();
    if length >= 3 {
        let name = text[..length].to_string();
        *text = &text[length..];
        return Ok(name);
    }
    Err(trap("Invalid bundled POSIX zone name"))
}

fn take_offset(text: &mut &str) -> Checked<i64> {
    // [+-]?[0-9]+(?::[0-9]+(?::[0-9]+)?)?
    let bytes = text.as_bytes();
    let mut end = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let digits = |from: usize| bytes[from..].iter().take_while(|b| b.is_ascii_digit()).count();
    let first = digits(end);
    if first == 0 {
        return Err(trap("Invalid bundled POSIX offset"));
    }
    end += first;
    for _ in 0..2 {
        if bytes.get(end) == Some(&b':') && digits(end + 1) > 0 {
            end += 1 + digits(end + 1);
        } else {
            break;
        }
    }
    let value = clock_seconds(&text[..end])?;
    *text = &text[end..];
    Ok(-value)
}

fn parse_rule(text: &str) -> Checked<Rule> {
    let (date, time) = match text.split_once('/') {
        Some((date, time)) => (date, Some(time)),
        None => (text, None),
    };
    let fields: Vec<&str> = date.strip_prefix('M').map(|rest| rest.split('.').collect()).unwrap_or_default();
    if fields.len() != 3 || fields.iter().any(|f| f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit())) {
        return Err(trap("Unsupported bundled POSIX transition rule"));
    }
    Ok(Rule {
        month: fields[0].parse().unwrap_or(0),
        week: fields[1].parse().unwrap_or(0),
        weekday: fields[2].parse().unwrap_or(0),
        seconds: match time {
            Some(time) => clock_seconds(time)?,
            None => 7200,
        },
    })
}

fn parse_posix(source: &str) -> Checked<Option<Footer>> {
    if source.is_empty() {
        return Ok(None);
    }
    let mut remaining = source;
    let standard_name = take_name(&mut remaining)?;
    let standard_offset = take_offset(&mut remaining)?;
    let mut footer = Footer {
        standard_name,
        standard_offset,
        daylight_name: None,
        daylight_offset: 0,
        start: None,
        end: None,
    };
    if remaining.is_empty() {
        return Ok(Some(footer));
    }
    footer.daylight_name = Some(take_name(&mut remaining)?);
    footer.daylight_offset = footer.standard_offset + 3600;
    if !remaining.starts_with(',') {
        footer.daylight_offset = take_offset(&mut remaining)?;
    }
    let Some(rules) = remaining.strip_prefix(',') else {
        return Err(trap("Invalid bundled POSIX transition list"));
    };
    let Some((start, end)) = rules.split_once(',') else {
        return Err(trap("Invalid bundled POSIX transition list"));
    };
    footer.start = Some(parse_rule(start)?);
    footer.end = Some(parse_rule(end)?);
    Ok(Some(footer))
}

fn load_zone(native: Native, name: &str) -> Checked<Zone> {
    let bytes = zone_file(name).ok_or_else(|| failed(native))?;
    let mut counts = header(bytes, 0)?;
    let mut offset = 44;
    let mut time_size = 4;
    if counts[0] != 0 {
        offset += block_size(4, &counts);
        counts = header(bytes, offset)?;
        offset += 44;
        time_size = 8;
    }
    let (time_count, type_count, char_count) = (counts[4], counts[5], counts[6]);
    let mut transitions = Vec::with_capacity(time_count);
    for _ in 0..time_count {
        transitions.push(if time_size == 8 {
            i64::from_be_bytes(bytes[offset..offset + 8].try_into().expect("eight bytes"))
        } else {
            i64::from(i32::from_be_bytes(bytes[offset..offset + 4].try_into().expect("four bytes")))
        });
        offset += time_size;
    }
    let indices = bytes[offset..offset + time_count].iter().map(|b| usize::from(*b)).collect();
    offset += time_count;
    let mut raw = Vec::with_capacity(type_count);
    for _ in 0..type_count {
        raw.push((
            i64::from(i32::from_be_bytes(bytes[offset..offset + 4].try_into().expect("four bytes"))),
            bytes[offset + 4] != 0,
            usize::from(bytes[offset + 5]),
        ));
        offset += 6;
    }
    // ISO-8859-1: байт — символ.
    let abbreviations: String = bytes[offset..offset + char_count].iter().map(|b| char::from(*b)).collect();
    offset += char_count;
    let types = raw
        .into_iter()
        .map(|(gmt_offset, daylight, start)| {
            let tail: String = abbreviations.chars().skip(start).collect();
            let abbreviation = tail.split('\0').next().unwrap_or("").to_string();
            Type {
                offset: gmt_offset,
                daylight,
                abbreviation,
            }
        })
        .collect();
    offset += counts[3] * (time_size + 4) + counts[2] + counts[1];
    let mut footer = None;
    if time_size == 8 && offset < bytes.len() {
        let remaining: String = bytes[offset..].iter().map(|b| char::from(*b)).collect();
        if let Some(rest) = remaining.strip_prefix('\n') {
            if let Some(line) = rest.split_once('\n').map(|(line, _)| line) {
                footer = parse_posix(line)?;
            }
        }
    }
    Ok(Zone {
        transitions,
        indices,
        types,
        footer,
    })
}

fn zone(native: Native, name: &str) -> Checked<Rc<Zone>> {
    thread_local! {
        static CACHE: std::cell::RefCell<HashMap<String, Rc<Zone>>> = std::cell::RefCell::new(HashMap::new());
    }
    if let Some(zone) = CACHE.with(|cache| cache.borrow().get(name).cloned()) {
        return Ok(zone);
    }
    let zone = Rc::new(load_zone(native, name)?);
    CACHE.with(|cache| cache.borrow_mut().insert(name.to_string(), zone.clone()));
    Ok(zone)
}

fn leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn month_days(year: i64, month: i64) -> i64 {
    match month {
        2 => {
            if leap(year) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn ordinal(year: i64, month: i64, day: i64) -> i64 {
    let previous = year.wrapping_sub(1);
    let mut before = (367 * month - 362) / 12;
    if month > 2 {
        before += if leap(year) { -1 } else { -2 };
    }
    previous
        .wrapping_mul(365)
        .wrapping_add(previous / 4)
        .wrapping_sub(previous / 100)
        .wrapping_add(previous / 400)
        .wrapping_add(before)
        .wrapping_add(day)
}

fn rule_day(year: i64, rule: &Rule) -> i64 {
    let first_monday = (ordinal(year, rule.month, 1) - 1).rem_euclid(7);
    let first_sunday_number = (first_monday + 1) % 7;
    let mut day = 1 + (rule.weekday - first_sunday_number + 7).rem_euclid(7) + (rule.week - 1) * 7;
    if day > month_days(year, rule.month) {
        day -= 7;
    }
    day
}

fn transition(year: i64, rule: &Rule, offset_before: i64) -> i64 {
    let day = rule_day(year, rule);
    (ordinal(year, rule.month, day) - 719_163) * 86_400 + rule.seconds - offset_before
}

fn year_from_seconds(seconds: i64) -> i64 {
    let target = seconds.div_euclid(86_400) + 719_163;
    let (mut low, mut high) = (1, 9999);
    while low < high {
        let middle = (low + high + 1) / 2;
        if ordinal(middle, 1, 1) <= target {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    low
}

fn observation(native: Native, name: &str, seconds: i64) -> Checked<Type> {
    let zone = zone(native, name)?;
    let transitions = &zone.transitions;
    if let (Some(last), Some(footer)) = (transitions.last(), &zone.footer) {
        if seconds > *last {
            let Some(daylight_name) = &footer.daylight_name else {
                return Ok(Type {
                    offset: footer.standard_offset,
                    daylight: false,
                    abbreviation: footer.standard_name.clone(),
                });
            };
            let year = year_from_seconds(seconds);
            let start = transition(year, footer.start.as_ref().expect("rules"), footer.standard_offset);
            let end = transition(year, footer.end.as_ref().expect("rules"), footer.daylight_offset);
            let daylight = if start < end {
                seconds >= start && seconds < end
            } else {
                seconds >= start || seconds < end
            };
            return Ok(Type {
                offset: if daylight { footer.daylight_offset } else { footer.standard_offset },
                daylight,
                abbreviation: if daylight { daylight_name.clone() } else { footer.standard_name.clone() },
            });
        }
    }
    let found = transitions.partition_point(|transition| *transition <= seconds);
    let index = if found > 0 { zone.indices[found - 1] } else { 0 };
    Ok(zone.types[index].clone())
}

fn floor_seconds(value: &Value) -> Result<i64, Error> {
    let rat = value.as_rat()?;
    let (quotient, remainder) =
        crate::integer::divide(rat.numerator(), rat.denominator()).expect("positive denominator");
    let quotient = if !remainder.is_zero() && rat.numerator().is_negative() {
        crate::integer::subtract(&quotient, &IntValue::from_i64(1))
    } else {
        quotient
    };
    Ok(java_long(&quotient))
}

fn candidates(native: Native, name: &str, args: &[Value]) -> Checked<Vec<i64>> {
    let field = |index: usize| -> Result<i64, Error> { Ok(java_long(int(&args[index])?)) };
    let local = (ordinal(field(1)?, field(2)?, field(3)?) - 719_163) * 86_400
        + field(4)? * 3600
        + field(5)? * 60
        + field(6)?;
    let zone = zone(native, name)?;
    let mut offsets: BTreeSet<i64> = zone.types.iter().map(|kind| kind.offset).collect();
    if let Some(footer) = &zone.footer {
        offsets.insert(footer.standard_offset);
        if footer.daylight_name.is_some() {
            offsets.insert(footer.daylight_offset);
        }
    }
    let mut out = Vec::new();
    for offset in offsets {
        let instant = local - offset;
        if observation(native, name, instant)?.offset == offset {
            out.push(instant);
        }
    }
    out.sort_unstable();
    Ok(out)
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    let operation = native.name();
    match operation {
        "time_pattern_invalid_offset" => Ok(int_value(pattern_invalid_offset(
            string(&args[0])?,
            args[1].as_bool()?,
            args[2].as_bool()?,
        ))),
        "time_format_date" | "time_format_local" => {
            let fields: &[char] = if operation == "time_format_date" {
                &['Y', 'm', 'd']
            } else {
                &['Y', 'm', 'd', 'H', 'M', 'S', 'f']
            };
            let mut values = HashMap::new();
            for (index, field) in fields.iter().enumerate() {
                values.insert(*field, int(&args[index])?.clone());
            }
            Ok(text(format(native, string(&args[fields.len()])?, &values)?))
        }
        "time_parse_invalid_offset" => Ok(int_value(
            parse(string(&args[0])?, string(&args[1])?).err().map_or(-1, |offset| offset as i64),
        )),
        "time_parse_parts" => {
            let values = parse(string(&args[0])?, string(&args[1])?).map_err(|_| failed(native))?;
            let fields: &[char] = if args[2].as_bool()? {
                &['Y', 'm', 'd', 'H', 'M', 'S', 'f']
            } else {
                &['Y', 'm', 'd']
            };
            let out = fields
                .iter()
                .map(|field| values.get(field).cloned().map(Value::Int).ok_or_else(|| failed(native)))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(array(out))
        }
        "timezone_version" => Ok(text(VERSION)),
        "timezone_known" => Ok(Value::Bool(zone_file(string(&args[0])?).is_some())),
        "timezone_offset" | "timezone_daylight" | "timezone_abbreviation" => {
            let observed = observation(native, string(&args[0])?, floor_seconds(&args[1])?)?;
            Ok(match operation {
                "timezone_offset" => int_value(observed.offset),
                "timezone_daylight" => Value::Bool(observed.daylight),
                _ => text(observed.abbreviation),
            })
        }
        "timezone_local_candidates" => Ok(int_value(
            candidates(native, string(&args[0])?, args)?.len() as i64,
        )),
        "timezone_local_instant" => {
            let found = candidates(native, string(&args[0])?, args)?;
            let later = args[8].as_bool()?;
            let seconds = if later { found.last() } else { found.first() }.copied().ok_or_else(|| failed(native))?;
            let billion = IntValue::from_i64(1_000_000_000);
            let numerator = crate::integer::add(
                &crate::integer::multiply(&IntValue::from_i64(seconds), &billion),
                int(&args[7])?,
            );
            let rat = crate::rat::Rat::new(&numerator, &billion).map_err(|message| Error::Trap(message))?;
            Ok(Value::Rat(Rc::new(rat)))
        }
        _ => Err(Error::Unsupported(format!(
            "native provider is not implemented: {}",
            native.symbol()
        ))),
    }
}
