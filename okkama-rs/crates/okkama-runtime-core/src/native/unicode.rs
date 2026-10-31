//! `runtime/unicode_character`: свойства символов по таблицам Unicode::UCD
//! Perl (`unicode.dat`) и ширина графем для терминала — перенос
//! `UnicodeNatives` JVM.
use super::Error;
use super::support::{array, int_value, string, string_array, text};
use crate::data;
use crate::{Value, abi::Native};
use std::collections::HashMap;
use unicode_segmentation::UnicodeSegmentation;

struct Tables {
    properties: HashMap<&'static str, Vec<(u32, u32, &'static str)>>,
    algorithmic: Vec<(u32, u32, &'static str)>,
    hangul: Vec<(u32, u32)>,
    numeric: HashMap<u32, &'static str>,
    names: HashMap<u32, &'static str>,
}

fn tables() -> &'static Tables {
    static TABLES: std::sync::OnceLock<Tables> = std::sync::OnceLock::new();
    TABLES.get_or_init(|| {
        let mut tables = Tables {
            properties: HashMap::new(),
            algorithmic: data::ranges(data::unicode(), "A"),
            hangul: Vec::new(),
            numeric: HashMap::new(),
            names: HashMap::new(),
        };
        let hex = |text: &str| u32::from_str_radix(text, 16).expect("hex");
        for line in data::unicode().lines() {
            let mut parts = line.splitn(4, ' ');
            match parts.next() {
                Some("R") => {
                    let property = parts.next().expect("property");
                    let from = hex(parts.next().expect("from"));
                    let to = hex(parts.next().expect("to"));
                    tables.properties.entry(property).or_default().push((from, to, ""));
                }
                Some("H") => {
                    let from = hex(parts.next().expect("from"));
                    let to = hex(parts.next().expect("to"));
                    tables.hangul.push((from, to));
                }
                Some("N") => {
                    let point = hex(parts.next().expect("point"));
                    tables.numeric.insert(point, parts.next().expect("value"));
                }
                Some("M") => {
                    let mut name = line.splitn(3, ' ').skip(1);
                    let point = hex(name.next().expect("point"));
                    tables.names.insert(point, name.next().expect("name"));
                }
                _ => {}
            }
        }
        tables
    })
}

fn has(property: &str, point: u32) -> bool {
    tables()
        .properties
        .get(property)
        .is_some_and(|ranges| data::find(ranges, point).is_some())
}

const JAMO_L: [&str; 19] = [
    "G", "GG", "N", "D", "DD", "R", "M", "B", "BB", "S", "SS", "", "J", "JJ", "C", "K", "T", "P", "H",
];
const JAMO_V: [&str; 21] = [
    "A", "AE", "YA", "YAE", "EO", "E", "YEO", "YE", "O", "WA", "WAE", "OE", "YO", "U", "WEO", "WE",
    "WI", "YU", "EU", "YI", "I",
];
const JAMO_T: [&str; 28] = [
    "", "G", "GG", "GS", "N", "NJ", "NH", "D", "L", "LG", "LM", "LB", "LS", "LT", "LP", "LH", "M",
    "B", "BS", "S", "SS", "NG", "J", "C", "K", "T", "P", "H",
];

fn name(point: u32) -> String {
    let tables = tables();
    if let Some(name) = tables.names.get(&point) {
        return name.to_string();
    }
    if let Some(prefix) = data::find(&tables.algorithmic, point) {
        return format!("{}-{point:X}", prefix.replace('_', " "));
    }
    if tables.hangul.iter().any(|(from, to)| (*from..=*to).contains(&point)) {
        let index = (point - 0xAC00) as usize;
        return format!(
            "HANGUL SYLLABLE {}{}{}",
            JAMO_L[index / 588],
            JAMO_V[(index % 588) / 28],
            JAMO_T[index % 28]
        );
    }
    String::new()
}

fn numeric(point: u32) -> Option<f64> {
    let value = tables().numeric.get(&point)?;
    if let Some((numerator, denominator)) = value.split_once('/') {
        if let (Ok(numerator), Ok(denominator)) = (numerator.parse::<f64>(), denominator.parse::<f64>()) {
            return Some(numerator / denominator);
        }
    }
    value.parse().ok()
}

fn width(cluster: &str) -> i64 {
    let points: Vec<u32> = cluster.chars().map(u32::from).collect();
    let visible: Vec<u32> = points
        .iter()
        .copied()
        .filter(|point| !has("M", *point) && !has("DI", *point))
        .collect();
    if visible.is_empty() {
        return 0;
    }
    let regional = points.iter().filter(|point| has("RI", **point)).count();
    let fe0f = points.contains(&0xFE0F);
    let fe0e = points.contains(&0xFE0E);
    let zwj = points.contains(&0x200D);
    let emoji = points.iter().any(|point| has("E", *point));
    let pictographic = points.iter().any(|point| has("EP", *point));
    let presentation = points.iter().any(|point| has("EPres", *point));
    // (?s).*[0-9#*]\x{FE0F}?\x{20E3}.*
    let keycap = points.windows(2).enumerate().any(|(index, pair)| {
        let base = |point: u32| char::from_u32(point).is_some_and(|c| c.is_ascii_digit() || c == '#' || c == '*');
        (base(pair[0]) && pair[1] == 0x20E3)
            || (base(pair[0]) && pair[1] == 0xFE0F && points.get(index + 2) == Some(&0x20E3))
    });
    if regional >= 2 || keycap || (fe0f && emoji) || (zwj && pictographic) || (!fe0e && presentation) {
        return 2;
    }
    if visible.iter().any(|point| has("W", *point) || has("F", *point)) {
        return 2;
    }
    1
}

fn character(value: &Value) -> Result<u32, Error> {
    Ok(u32::from(value.as_char()?))
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    let operation = native.name();
    match operation {
        "character_name" => Ok(text(name(character(&args[0])?))),
        "category_code" => Ok(text(data::category(character(&args[0])?))),
        "character_combining_class" => Ok(int_value(i64::from(data::combining_class(character(&args[0])?)))),
        "character_is_numeric" => Ok(Value::Bool(numeric(character(&args[0])?).is_some())),
        "character_numeric_value" => Ok(Value::Float(numeric(character(&args[0])?).unwrap_or(0.0))),
        "display_clusters" => Ok(string_array(string(&args[0])?.graphemes(true))),
        "display_widths" => Ok(array(
            string(&args[0])?
                .graphemes(true)
                .map(|cluster| int_value(width(cluster)))
                .collect(),
        )),
        "display_invalid_index" => {
            let index = string(&args[0])?
                .chars()
                .position(|c| matches!(data::category(u32::from(c)), "Cc" | "Zl" | "Zp"))
                .map_or(-1, |index| index as i64);
            Ok(int_value(index))
        }
        _ => Err(Error::Unsupported(format!(
            "native provider is not implemented: {}",
            native.symbol()
        ))),
    }
}
