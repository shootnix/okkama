//! `runtime/postgres`: the DataRow messages of one result portion become one
//! result page (`db/read.DbPage`) in one call, as on the Sparrow core.
use super::support::{array, byte_array, bytes, failed, handle, int, items, new_handle, text};
use super::Error;
use crate::{IntValue, Value, abi::Native};

/// Cell kinds of a page (the db cell codes): 0 null, 1 boolean, 2 integer,
/// 3 floating, 4 decimal (text), 5 text, 6 bytes.
#[derive(Debug, Default)]
struct PgPage {
    failure: String,
    rows: usize,
    kinds: Vec<i64>,
    values: Vec<i64>,
    floats: Vec<f64>,
    texts: Vec<String>,
    blobs: Vec<Vec<u8>>,
}

fn int32(raw: &[u8], at: usize) -> Option<i32> {
    raw.get(at..at + 4).map(|b| i32::from_be_bytes(b.try_into().expect("four bytes")))
}

impl PgPage {
    fn parse(raw: &[u8], width: usize, oids: &[i64]) -> Self {
        let mut page = PgPage::default();
        if let Err(failure) = page.fill(raw, width, oids) {
            page = PgPage { failure: failure.into(), ..PgPage::default() };
        }
        page
    }

    fn fill(&mut self, raw: &[u8], width: usize, oids: &[i64]) -> Result<(), &'static str> {
        let mut at = 0;
        while at < raw.len() {
            if raw[at] != b'D' {
                return Err("message");
            }
            let length = int32(raw, at + 1).ok_or("truncated")?;
            let end = usize::try_from(length).ok().filter(|&l| l >= 4).map(|l| at + 1 + l).ok_or("truncated")?;
            if end > raw.len() {
                return Err("truncated");
            }
            let count = raw.get(at + 5..at + 7).map(|b| i16::from_be_bytes([b[0], b[1]])).ok_or("truncated")?;
            if count as usize != width {
                return Err("width");
            }
            let mut cursor = at + 7;
            for &oid in oids.iter().take(width) {
                let length = int32(raw, cursor).ok_or("truncated")?;
                cursor += 4;
                if length < 0 {
                    self.cell(0, 0);
                    continue;
                }
                let stop = cursor + length as usize;
                if stop > end {
                    return Err("truncated");
                }
                self.value(&raw[cursor..stop], oid)?;
                cursor = stop;
            }
            if cursor != end {
                return Err("truncated");
            }
            self.rows += 1;
            at = end;
        }
        Ok(())
    }

    fn cell(&mut self, kind: i64, value: i64) {
        self.kinds.push(kind);
        self.values.push(value);
    }

    fn value(&mut self, bytes: &[u8], oid: i64) -> Result<(), &'static str> {
        let text = std::str::from_utf8(bytes).map_err(|_| "utf8")?;
        match oid {
            16 => self.cell(1, i64::from(text == "t")),
            20 | 21 | 23 | 26 => self.cell(2, text.parse().map_err(|_| "integer")?),
            700 | 701 => {
                if matches!(text, "NaN" | "Infinity" | "-Infinity") {
                    return Err("nonfinite");
                }
                let value: f64 = text.parse().map_err(|_| "floating")?;
                self.cell(3, self.floats.len() as i64);
                self.floats.push(value);
            }
            1700 => {
                if matches!(text, "NaN" | "Infinity" | "-Infinity") {
                    return Err("nonfinite");
                }
                self.cell(4, self.texts.len() as i64);
                self.texts.push(text.to_owned());
            }
            17 => {
                let hex = text.strip_prefix("\\x").ok_or("bytea")?.as_bytes();
                if hex.len() % 2 != 0 {
                    return Err("bytea");
                }
                let digit = |c: u8| (c as char).to_digit(16).ok_or("bytea");
                let mut blob = Vec::with_capacity(hex.len() / 2);
                for pair in hex.chunks_exact(2) {
                    blob.push((digit(pair[0])? * 16 + digit(pair[1])?) as u8);
                }
                self.cell(6, self.blobs.len() as i64);
                self.blobs.push(blob);
            }
            _ => {
                self.cell(5, self.texts.len() as i64);
                self.texts.push(text.to_owned());
            }
        }
        Ok(())
    }
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    if native.name() == "pg_page" {
        let width = int(&args[1])?.to_i64().and_then(|w| usize::try_from(w).ok()).ok_or_else(|| failed(native))?;
        let oids: Vec<i64> = items(&args[2])?.iter().map(|v| int(v).ok().and_then(IntValue::to_i64).unwrap_or(0)).collect();
        if oids.len() != width {
            return Err(failed(native));
        }
        return Ok(new_handle(PgPage::parse(&bytes(&args[0])?, width, &oids)));
    }
    let page = handle::<PgPage>(&args[0])?;
    let integers = |values: &[i64]| array(values.iter().map(|&v| Value::Int(IntValue::from_i64(v))).collect());
    Ok(match native.name() {
        "pg_page_failure" => text(page.failure.as_str()),
        "pg_page_rows" => Value::Int(IntValue::from_i64(page.rows as i64)),
        "pg_page_kinds" => integers(&page.kinds),
        "pg_page_values" => integers(&page.values),
        "pg_page_floats" => array(page.floats.iter().map(|&f| Value::Float(f)).collect()),
        "pg_page_texts" => array(page.texts.iter().map(|t| text(t.as_str())).collect()),
        "pg_page_blobs" => array(page.blobs.iter().map(|b| byte_array(b.clone())).collect()),
        _ => return Err(Error::Unsupported(format!("native provider is not implemented: {}", native.symbol()))),
    })
}
