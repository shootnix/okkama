//! `runtime/idna` и `runtime/public_suffix` — перенос `IdnaNatives` JVM:
//! UTS #46 (nontransitional, STD3, без unassigned) как Net::IDN::UTS46 2.591
//! на тех же таблицах (`idna.dat`) и Public Suffix List (`psl.dat`).
use super::Error;
use super::support::{failure, string, text};
use crate::data;
use crate::{Value, abi::Native};
use std::collections::{HashMap, HashSet};
use unicode_normalization::UnicodeNormalization;

/// Ошибка UTS #46 (croak модуля): провайдер превращает её в отказ.
struct Invalid;

type Checked<T> = Result<T, Invalid>;

struct Tables {
    status: Vec<(u32, u32, &'static str)>,
    mapping: HashMap<u32, Vec<u32>>,
    bidi: Vec<(u32, u32, &'static str)>,
    joining: Vec<(u32, u32, &'static str)>,
}

fn tables() -> &'static Tables {
    static TABLES: std::sync::OnceLock<Tables> = std::sync::OnceLock::new();
    TABLES.get_or_init(|| {
        let text = data::idna();
        let sorted = |tag| {
            let mut ranges = data::ranges(text, tag);
            ranges.sort_by_key(|(from, _, _)| *from);
            ranges
        };
        let mut mapping = HashMap::new();
        for line in text.lines().filter(|line| line.starts_with("P ")) {
            let mut parts = line.split(' ').skip(1);
            let source = u32::from_str_radix(parts.next().expect("source"), 16).expect("hex");
            let target = parts
                .map(|part| u32::from_str_radix(part, 16).expect("hex"))
                .collect();
            mapping.insert(source, target);
        }
        Tables {
            status: sorted("S"),
            mapping,
            bidi: sorted("B"),
            joining: sorted("J"),
        }
    })
}

fn status(point: u32) -> &'static str {
    data::find(&tables().status, point).unwrap_or("X")
}

fn bidi(point: u32) -> &'static str {
    data::find(&tables().bidi, point).unwrap_or("L")
}

fn joining(point: u32) -> &'static str {
    data::find(&tables().joining, point).unwrap_or("U")
}

fn unassigned(point: u32) -> bool {
    data::category(point) == "Cn"
}

// ---- Punycode (RFC 3492) ----

const BASE: i64 = 36;
const TMIN: i64 = 1;
const TMAX: i64 = 26;
const SKEW: i64 = 38;
const DAMP: i64 = 700;
const INITIAL_BIAS: i64 = 72;
const INITIAL_N: i64 = 128;

fn adapt(delta: i64, points: i64, first: bool) -> i64 {
    let mut delta = if first { delta / DAMP } else { delta / 2 };
    delta += delta / points;
    let mut k = 0;
    while delta > ((BASE - TMIN) * TMAX) / 2 {
        delta /= BASE - TMIN;
        k += BASE;
    }
    k + (BASE - TMIN + 1) * delta / (delta + SKEW)
}

fn digit(value: i64) -> char {
    char::from(if value < 26 {
        b'a' + value as u8
    } else {
        b'0' + (value - 26) as u8
    })
}

fn threshold(k: i64, bias: i64) -> i64 {
    if k <= bias {
        TMIN
    } else if k >= bias + TMAX {
        TMAX
    } else {
        k - bias
    }
}

pub(crate) fn encode(input: &[u32]) -> Result<String, ()> {
    let mut output = String::new();
    let mut basic = 0;
    for point in input {
        if *point < 0x80 {
            output.push(char::from_u32(*point).expect("ascii"));
            basic += 1;
        }
    }
    let mut handled = basic;
    if basic > 0 {
        output.push('-');
    }
    let (mut n, mut delta, mut bias) = (INITIAL_N, 0i64, INITIAL_BIAS);
    while handled < input.len() {
        let m = input
            .iter()
            .map(|point| i64::from(*point))
            .filter(|point| *point >= n)
            .min()
            .expect("unhandled points remain");
        delta += (m - n) * (handled as i64 + 1);
        if delta > i64::from(i32::MAX) {
            return Err(());
        }
        n = m;
        for point in input {
            let point = i64::from(*point);
            if point < n {
                delta += 1;
            }
            if point == n {
                let mut q = delta;
                let mut k = BASE;
                loop {
                    let t = threshold(k, bias);
                    if q < t {
                        break;
                    }
                    output.push(digit(t + (q - t) % (BASE - t)));
                    q = (q - t) / (BASE - t);
                    k += BASE;
                }
                output.push(digit(q));
                bias = adapt(delta, handled as i64 + 1, handled == basic);
                delta = 0;
                handled += 1;
            }
        }
        delta += 1;
        n += 1;
    }
    Ok(output)
}

pub(crate) fn decode(input: &str) -> Result<Vec<u32>, ()> {
    if !input.is_ascii() {
        return Err(());
    }
    let bytes = input.as_bytes();
    let mut output: Vec<u32> = Vec::new();
    let delimiter = input.rfind('-');
    let mut position = 0;
    match delimiter {
        Some(at) if at > 0 => {
            output.extend(bytes[..at].iter().map(|b| u32::from(*b)));
            position = at + 1;
        }
        Some(0) => position = 1,
        _ => {}
    }
    let (mut n, mut i, mut bias) = (INITIAL_N, 0i64, INITIAL_BIAS);
    while position < bytes.len() {
        let old = i;
        let mut w = 1i64;
        let mut k = BASE;
        loop {
            let c = *bytes.get(position).ok_or(())?;
            position += 1;
            let value = match c {
                b'0'..=b'9' => i64::from(c - b'0') + 26,
                b'a'..=b'z' => i64::from(c - b'a'),
                b'A'..=b'Z' => i64::from(c - b'A'),
                _ => return Err(()),
            };
            i += value * w;
            if i > i64::from(i32::MAX) {
                return Err(());
            }
            let t = threshold(k, bias);
            if value < t {
                break;
            }
            w *= BASE - t;
            if w > i64::from(i32::MAX) {
                return Err(());
            }
            k += BASE;
        }
        let count = output.len() as i64 + 1;
        bias = adapt(i - old, count, old == 0);
        n += i / count;
        if n > 0x10FFFF {
            return Err(());
        }
        i %= count;
        output.insert(i as usize, n as u32);
        i += 1;
    }
    Ok(output)
}

// ---- UTS #46 ----

fn string_of(points: &[u32]) -> String {
    points.iter().filter_map(|point| char::from_u32(*point)).collect()
}

fn points(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).collect()
}

fn nfc(text: &str) -> String {
    text.nfc().collect()
}

fn is_mark(point: u32) -> bool {
    data::category(point).starts_with('M')
}

fn validate_label(label: &[u32], assume_nfc: bool) -> Checked<()> {
    let text = string_of(label);
    if !assume_nfc && text != nfc(&text) {
        return Err(Invalid);
    }
    if label.len() >= 4 && label[2] == '-' as u32 && label[3] == '-' as u32 {
        return Err(Invalid);
    }
    if label.first() == Some(&('-' as u32)) || label.last() == Some(&('-' as u32)) {
        return Err(Invalid);
    }
    if label.contains(&('.' as u32)) {
        return Err(Invalid);
    }
    if label.first().is_some_and(|point| is_mark(*point)) {
        return Err(Invalid);
    }
    if label.iter().any(|point| unassigned(*point)) {
        return Err(Invalid);
    }
    if label
        .iter()
        .any(|point| matches!(status(*point), "V3" | "I" | "M" | "M3" | "X"))
    {
        return Err(Invalid);
    }
    Ok(())
}

/// Конец метки: [ends…][NSM или unassigned]*.
fn ends_with(label: &[u32], ends: &[&str]) -> bool {
    let mut index = label.len() as i64 - 1;
    while index >= 0
        && (bidi(label[index as usize]) == "NSM" || unassigned(label[index as usize]))
    {
        if ends.contains(&bidi(label[index as usize])) {
            return true;
        }
        index -= 1;
    }
    index >= 0 && ends.contains(&bidi(label[index as usize]))
}

fn validate_bidi(label: &[u32]) -> Checked<()> {
    let Some(first) = label.first() else {
        return Ok(());
    };
    let first = bidi(*first);
    if first == "L" {
        if label.iter().any(|point| {
            !["L", "EN", "ES", "CS", "ET", "BN", "ON", "NSM"].contains(&bidi(*point))
        }) || !ends_with(label, &["L", "EN"])
        {
            return Err(Invalid);
        }
        return Ok(());
    }
    if first == "R" || first == "AL" {
        let (mut en, mut an) = (false, false);
        for point in label {
            let class = bidi(*point);
            if !["R", "AL", "AN", "EN", "ES", "CS", "ET", "ON", "BN", "NSM"].contains(&class) {
                return Err(Invalid);
            }
            en |= class == "EN";
            an |= class == "AN";
        }
        if !ends_with(label, &["R", "AL", "EN", "AN"]) || (en && an) {
            return Err(Invalid);
        }
        return Ok(());
    }
    Err(Invalid)
}

fn virama(point: u32) -> bool {
    data::combining_class(point) == 9
}

/// Совпадение `(L|D) T* ZWNJ T* (R|D)` с начала `start`.
fn joined_zwnj(label: &[u32], start: usize) -> bool {
    if !matches!(joining(label[start]), "L" | "D") {
        return false;
    }
    let mut index = start + 1;
    while index < label.len() && joining(label[index]) == "T" {
        index += 1;
    }
    if index >= label.len() || label[index] != 0x200C {
        return false;
    }
    index += 1;
    while index < label.len() && joining(label[index]) == "T" {
        index += 1;
    }
    index < label.len() && matches!(joining(label[index]), "R" | "D")
}

fn validate_contextj(label: &[u32]) -> Checked<()> {
    for start in 0..label.len() {
        if virama(label[start]) && label.get(start + 1) == Some(&0x200C) {
            break;
        }
        if joined_zwnj(label, start) {
            break;
        }
        if label[start] == 0x200C {
            return Err(Invalid);
        }
    }
    for start in 0..label.len() {
        if virama(label[start]) && label.get(start + 1) == Some(&0x200D) {
            break;
        }
        if label[start] == 0x200D {
            return Err(Invalid);
        }
    }
    Ok(())
}

/// `uts46_to_ascii` / `uts46_to_unicode` (STD3, nontransitional).
fn process(input: &str, ascii: bool) -> Checked<String> {
    let source = points(input);
    if source
        .iter()
        .any(|point| matches!(status(*point), "X" | "V3" | "M3"))
    {
        return Err(Invalid);
    }
    let mut mapped = Vec::new();
    for point in &source {
        match status(*point) {
            "I" => {}
            "M" => mapped.extend(tables().mapping.get(point).cloned().unwrap_or_default()),
            _ => mapped.push(*point),
        }
    }
    let label = nfc(&string_of(&mapped));
    let mut labels: Vec<&str> = label.split('.').collect();
    let rooted = labels.last().is_some_and(|last| last.is_empty());
    if rooted {
        labels.pop();
    }
    let mut is_bidi = false;
    let mut decoded = Vec::new();
    for part in labels {
        let value = if part.len() > 4 && part[..4].eq_ignore_ascii_case("xn--") && part[4..].is_ascii()
        {
            let value = decode(&part[4..]).map_err(|_| Invalid)?;
            validate_label(&value, false)?;
            value
        } else {
            let value = points(part);
            validate_label(&value, true)?;
            value
        };
        if value
            .iter()
            .any(|point| matches!(bidi(*point), "R" | "AL" | "AN"))
        {
            is_bidi = true;
        }
        decoded.push(value);
    }
    let mut out = Vec::new();
    for value in decoded {
        if is_bidi {
            validate_bidi(&value)?;
        }
        validate_contextj(&value)?;
        let mut text = string_of(&value);
        if ascii && value.iter().any(|point| *point >= 0x80) {
            if value.len() > 63 - 4 {
                return Err(Invalid);
            }
            text = format!("xn--{}", encode(&value).map_err(|_| Invalid)?);
        }
        if text.is_empty() || (ascii && text.chars().count() > 63) {
            return Err(Invalid);
        }
        out.push(text);
    }
    let domain = out.join(".");
    if domain.is_empty() || (ascii && domain.chars().count() > 253) {
        return Err(Invalid);
    }
    Ok(if rooted { format!("{domain}.") } else { domain })
}

fn dns_label(label: &str) -> bool {
    let bytes = label.as_bytes();
    let edge = |b: &u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    !bytes.is_empty()
        && edge(&bytes[0])
        && edge(&bytes[bytes.len() - 1])
        && bytes.iter().all(|b| edge(b) || *b == b'-')
}

/// `_ascii` Perl-provider: UTS #46 и проверки DNS и A-меток.
pub(crate) fn hostname_ascii(source: &str) -> Result<String, ()> {
    let size = source.chars().count();
    if size == 0 || size > 1024 {
        return Err(());
    }
    let ascii = process(source, true).map_err(|_| ())?;
    let unrooted = ascii.strip_suffix('.').unwrap_or(&ascii);
    if unrooted.is_empty() || unrooted.len() > 253 {
        return Err(());
    }
    let labels: Vec<&str> = unrooted.split('.').collect();
    if labels
        .iter()
        .any(|label| label.len() > 63 || !dns_label(label))
    {
        return Err(());
    }
    let raw = source.replace(['。', '．', '｡'], ".");
    let mut supplied: Vec<&str> = raw.split('.').collect();
    if supplied.last().is_some_and(|last| last.is_empty()) {
        supplied.pop();
    }
    for (at, part) in supplied.iter().enumerate() {
        let label = part.nfkc().collect::<String>().to_lowercase();
        if label.starts_with("xn--") && labels.get(at).is_none_or(|known| *known != label) {
            return Err(());
        }
    }
    Ok(ascii)
}

// ---- runtime/public_suffix ----

struct Rules {
    version: &'static str,
    exact: HashSet<&'static str>,
    wildcard: HashSet<&'static str>,
    exception: HashSet<&'static str>,
}

fn rules() -> &'static Rules {
    static RULES: std::sync::OnceLock<Rules> = std::sync::OnceLock::new();
    RULES.get_or_init(|| {
        let mut lines = data::public_suffixes().lines();
        let version = lines.next().unwrap_or("");
        let mut rules = Rules {
            version,
            exact: HashSet::new(),
            wildcard: HashSet::new(),
            exception: HashSet::new(),
        };
        for line in lines.filter(|line| !line.is_empty()) {
            if let Some(rule) = line.strip_prefix('!') {
                rules.exception.insert(rule);
            } else if let Some(rule) = line.strip_prefix("*.") {
                rules.wildcard.insert(rule);
            } else {
                rules.exact.insert(line);
            }
        }
        rules
    })
}

/// Метки домена и ширина публичного суффикса.
pub(crate) fn suffix_parts(source: &str) -> Result<(Vec<String>, usize), ()> {
    let rules = rules();
    let ascii = hostname_ascii(source)?;
    let ascii = ascii.strip_suffix('.').unwrap_or(&ascii);
    let labels: Vec<String> = ascii
        .split('.')
        .map(str::to_string)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .skip_while(String::is_empty)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let mut best = 1;
    for at in 0..labels.len() {
        let key = labels[at..].join(".");
        let width = labels.len() - at;
        if rules.exception.contains(key.as_str()) {
            return Ok((labels, width - 1));
        }
        if rules.exact.contains(key.as_str()) && width > best {
            best = width;
        }
        if at > 0 && rules.wildcard.contains(key.as_str()) && width + 1 > best {
            best = width + 1;
        }
    }
    Ok((labels, best))
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    let name = native.name();
    match name {
        "hostname_ascii" | "hostname_unicode" => {
            let invalid = || failure("core.idna", "invalid_input", native, "invalid bounded IDNA hostname");
            let ascii = hostname_ascii(string(&args[0])?).map_err(|_| invalid())?;
            if name == "hostname_ascii" {
                return Ok(text(ascii));
            }
            Ok(text(process(&ascii, false).map_err(|_| invalid())?))
        }
        "public_suffix_name" | "registrable_name" => {
            let (labels, mut width) = suffix_parts(string(&args[0])?).map_err(|_| {
                failure(
                    "core.public_suffix",
                    "invalid_input",
                    native,
                    "invalid bounded IDNA hostname",
                )
            })?;
            if name == "registrable_name" {
                if labels.len() <= width {
                    return Err(failure(
                        "core.public_suffix",
                        "not_found",
                        native,
                        "public suffix has no registrable domain",
                    ));
                }
                width += 1;
            }
            Ok(text(labels[labels.len().saturating_sub(width)..].join(".")))
        }
        "public_suffix_version" => Ok(text(rules().version)),
        _ => Err(Error::Unsupported(format!(
            "native provider is not implemented: {}",
            native.symbol()
        ))),
    }
}
