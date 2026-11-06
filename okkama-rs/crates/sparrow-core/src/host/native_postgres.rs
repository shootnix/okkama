//! `runtime/postgres`: the DataRow messages of one result portion, in the
//! PostgreSQL text format, become one result page (`db/read.DbPage`) — the
//! same flat columns a SQLite page has — in one call.
//!
//! The input is the raw messages as they came off the wire (`'D'`, Int32
//! length, Int16 column count, then per column an Int32 length, -1 for NULL,
//! and the bytes). Column type oids pick the cell kind; a malformed page is
//! reported by `pg_page_failure` (empty when the page is whole).
use crate::host::*;
use okkama_runtime_abi::Native;

/// Cell kinds of a page (the db cell codes): 0 null, 1 boolean, 2 integer,
/// 3 floating, 4 decimal (text), 5 text, 6 bytes.
#[derive(Debug, Default)]
pub(crate) struct PgPage {
    failure: String,
    rows: usize,
    kinds: Vec<i64>,
    values: Vec<i64>,
    floats: Vec<f64>,
    texts: Vec<String>,
    blobs: Vec<Vec<u8>>,
}

pub(crate) fn handles(native: Native) -> bool {
    use Native::*;
    matches!(
        native,
        PostgresPgPage
            | PostgresPgPageFailure
            | PostgresPgPageRows
            | PostgresPgPageKinds
            | PostgresPgPageValues
            | PostgresPgPageFloats
            | PostgresPgPageTexts
            | PostgresPgPageBlobs
    )
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

impl Heap {
    pub(crate) fn native_postgres(&mut self, native: Native, args: &[Slot]) -> Result<Outcome, Error> {
        use Native::*;
        if native == PostgresPgPage {
            let small = |heap: &Self, slot: Slot| match heap.integer(slot) {
                Ok(Integer::Small(n)) => Some(n),
                _ => None,
            };
            let width = small(self, args[1]).and_then(|w| usize::try_from(w).ok()).ok_or_else(|| Error("page width".into()))?;
            let oids: Vec<i64> = self.elements(args[2])?.into_iter().map(|s| small(self, s).unwrap_or(0)).collect();
            if oids.len() != width {
                return Err(Error("page types do not match its width".into()));
            }
            let page = PgPage::parse(&self.byte_slice(args[0])?, width, &oids);
            return Ok(Outcome::Value(Slot { tag: HANDLE, bits: self.handles.allocate(Handle::PgPage(Box::new(page)))? }));
        }
        // The page is moved out while its arrays are allocated, then put back.
        let Handle::PgPage(slot) = self.handles.get_mut(args[0].bits)? else {
            return Err(Error("expected a PgPageState".into()));
        };
        let page = std::mem::take(&mut **slot);
        let result = match native {
            PostgresPgPageFailure => self.allocate_text(page.failure.clone()),
            PostgresPgPageRows => Ok(Slot::small(page.rows as i64)),
            PostgresPgPageKinds => self.allocate_array(page.kinds.iter().map(|&k| Slot::small(k)).collect()),
            PostgresPgPageValues => self.allocate_array(page.values.iter().map(|&v| Slot::small(v)).collect()),
            PostgresPgPageFloats => {
                self.allocate_array(page.floats.iter().map(|f| Slot { tag: FLOAT, bits: f.to_bits() }).collect())
            }
            PostgresPgPageTexts => {
                let texts = page.texts.iter().map(|t| self.allocate_text(t.clone())).collect::<Result<Vec<_>, _>>()?;
                self.allocate_array(texts)
            }
            _ => {
                let blobs = page.blobs.iter().map(|b| self.allocate_bytes(b)).collect::<Result<Vec<_>, _>>()?;
                self.allocate_array(blobs)
            }
        };
        let Handle::PgPage(slot) = self.handles.get_mut(args[0].bits)? else { unreachable!() };
        **slot = page;
        result.map(Outcome::Value)
    }
}
