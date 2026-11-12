//! JSON items cut from a stream (#136): the elements of one top-level array,
//! or values one after another (NDJSON and other whitespace-separated
//! sequences). The splitter only finds where each item ends, keeping strings
//! and nesting in mind across the pieces it is fed; `std/json` parses each
//! item's bytes with the ordinary parser. Memory stays at one item and the
//! unread rest of the last piece.

use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Nothing significant seen yet: `[` makes it an array, anything else a
    /// sequence.
    Unknown,
    Array,
    Sequence,
    /// After the array's `]`: only whitespace may follow.
    Done,
}

/// The value being scanned: where it starts in `pending` and what is open.
#[derive(Debug, Default, Clone, Copy)]
struct Value {
    start: usize,
    depth: usize,
    string: bool,
    escaped: bool,
    scalar: bool,
}

#[derive(Debug)]
pub struct Splitter {
    pending: Vec<u8>,
    /// Absolute offset of `pending[0]` in the stream.
    offset: u64,
    /// How far `pending` has been looked at.
    cursor: usize,
    mode: Mode,
    /// In an array: the next significant byte must be `,` or `]`.
    separator: bool,
    /// In an array: an item must follow (after `,`), so `]` is refused.
    required: bool,
    value: Option<Value>,
    finished: bool,
    maximum: usize,
    item: Vec<u8>,
    error: Option<(u64, String)>,
}

/// What `next` found.
#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    Item,
    More,
    End,
    Error,
}

impl Splitter {
    pub fn new(maximum: usize) -> Self {
        Splitter {
            pending: Vec::new(),
            offset: 0,
            cursor: 0,
            mode: Mode::Unknown,
            separator: false,
            required: false,
            value: None,
            finished: false,
            maximum,
            item: Vec::new(),
            error: None,
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.pending.extend_from_slice(bytes);
    }

    /// The stream has ended: a scalar at the very end is complete.
    pub fn finish(&mut self) {
        self.finished = true;
    }

    pub fn item(&self) -> &[u8] {
        &self.item
    }

    pub fn error(&self) -> Option<&(u64, String)> {
        self.error.as_ref()
    }

    fn fail(&mut self, at: usize, message: impl Into<String>) -> Step {
        self.error = Some((self.offset + at as u64, message.into()));
        Step::Error
    }

    /// Drop what has been consumed, so memory stays bounded.
    fn consume(&mut self, upto: usize) {
        self.pending.drain(..upto);
        self.offset += upto as u64;
        self.cursor -= upto;
    }

    fn skip_whitespace(&mut self) {
        while self.cursor < self.pending.len() && matches!(self.pending[self.cursor], b' ' | b'\t' | b'\n' | b'\r') {
            self.cursor += 1;
        }
        let upto = self.cursor;
        self.consume(upto);
    }

    pub fn advance(&mut self) -> Step {
        if self.error.is_some() {
            return Step::Error;
        }
        loop {
            if self.value.is_some() {
                return self.scan();
            }
            self.skip_whitespace();
            let Some(&byte) = self.pending.get(self.cursor) else {
                return match (self.finished, self.mode) {
                    (false, _) => Step::More,
                    (true, Mode::Unknown | Mode::Sequence | Mode::Done) => Step::End,
                    (true, Mode::Array) => self.fail(self.cursor, "the array is not closed"),
                };
            };
            match self.mode {
                Mode::Unknown if byte == b'[' => {
                    self.mode = Mode::Array;
                    self.cursor += 1;
                    let upto = self.cursor;
                    self.consume(upto);
                }
                Mode::Unknown => self.mode = Mode::Sequence,
                Mode::Done => return self.fail(self.cursor, "data after the array"),
                Mode::Array if self.separator => {
                    match byte {
                        b',' => {
                            self.separator = false;
                            self.required = true;
                        }
                        b']' => self.mode = Mode::Done,
                        _ => return self.fail(self.cursor, "expected , or ] between items"),
                    }
                    self.cursor += 1;
                    let upto = self.cursor;
                    self.consume(upto);
                }
                Mode::Array if byte == b']' => {
                    if self.required {
                        return self.fail(self.cursor, "a comma before ]");
                    }
                    self.mode = Mode::Done;
                    self.cursor += 1;
                    let upto = self.cursor;
                    self.consume(upto);
                }
                Mode::Array | Mode::Sequence => {
                    if self.mode == Mode::Array && byte == b',' {
                        return self.fail(self.cursor, "an empty item");
                    }
                    self.value = Some(Value {
                        start: self.cursor,
                        depth: usize::from(matches!(byte, b'{' | b'[')),
                        string: byte == b'"',
                        escaped: false,
                        scalar: !matches!(byte, b'{' | b'[' | b'"'),
                    });
                    self.cursor += 1;
                }
            }
        }
    }

    /// Scan the open value to its end, or ask for more.
    fn scan(&mut self) -> Step {
        let Some(mut value) = self.value else {
            return Step::More;
        };
        let mut end = None;
        while self.cursor < self.pending.len() {
            let byte = self.pending[self.cursor];
            if value.scalar {
                if matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | b',' | b']' | b'}' | b'[' | b'{' | b'"') {
                    end = Some(self.cursor);
                    break;
                }
            } else if value.string {
                if value.escaped {
                    value.escaped = false;
                } else if byte == b'\\' {
                    value.escaped = true;
                } else if byte == b'"' {
                    value.string = false;
                    if value.depth == 0 {
                        end = Some(self.cursor + 1);
                        self.cursor += 1;
                        break;
                    }
                }
            } else {
                match byte {
                    b'"' => value.string = true,
                    b'{' | b'[' => value.depth += 1,
                    b'}' | b']' => {
                        value.depth -= 1;
                        if value.depth == 0 {
                            end = Some(self.cursor + 1);
                            self.cursor += 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            self.cursor += 1;
            if self.cursor - value.start > self.maximum {
                return self.fail(value.start, format!("an item longer than {} bytes", self.maximum));
            }
        }
        if end.is_none() && value.scalar && self.finished {
            end = Some(self.cursor);
        }
        let Some(end) = end else {
            self.value = Some(value);
            if self.finished {
                return self.fail(value.start, "the stream ends inside an item");
            }
            return Step::More;
        };
        if end - value.start > self.maximum {
            return self.fail(value.start, format!("an item longer than {} bytes", self.maximum));
        }
        self.item = self.pending[value.start..end].to_vec();
        self.value = None;
        self.cursor = end;
        self.consume(end);
        if self.mode == Mode::Array {
            self.separator = true;
            self.required = false;
        }
        Step::Item
    }
}

/// The handle `std/runtime/json` keeps.
#[derive(Debug)]
pub struct SplitState(pub RefCell<Splitter>);

#[cfg(test)]
mod tests {
    use super::*;

    fn items(pieces: &[&str]) -> Result<Vec<String>, String> {
        let mut splitter = Splitter::new(1024);
        let mut out = Vec::new();
        let mut rest = pieces.iter();
        loop {
            match splitter.advance() {
                Step::Item => out.push(String::from_utf8(splitter.item().to_vec()).unwrap()),
                Step::End => return Ok(out),
                Step::Error => return Err(splitter.error().unwrap().1.clone()),
                Step::More => match rest.next() {
                    Some(piece) => splitter.feed(piece.as_bytes()),
                    None => splitter.finish(),
                },
            }
        }
    }

    #[test]
    fn arrays_and_sequences_split_across_pieces() {
        assert_eq!(items(&["[1, {\"a\": [2", ", \"]\"]}, \"x\\\"", "y\" ,true]"]).unwrap(), vec![
            "1",
            "{\"a\": [2, \"]\"]}",
            "\"x\\\"y\"",
            "true"
        ]);
        assert_eq!(items(&["{\"a\":1}\n{\"b\"", ":2}\n3 4", "5\n"]).unwrap(), vec!["{\"a\":1}", "{\"b\":2}", "3", "45"]);
        assert_eq!(items(&["  []  "]).unwrap(), Vec::<String>::new());
        assert_eq!(items(&[""]).unwrap(), Vec::<String>::new());
        assert_eq!(items(&["[1,]"]).unwrap_err(), "a comma before ]");
        assert_eq!(items(&["[1 2]"]).unwrap_err(), "expected , or ] between items");
        assert_eq!(items(&["[1,2"]).unwrap_err(), "the array is not closed");
        assert_eq!(items(&["[1] 2"]).unwrap_err(), "data after the array");
        assert_eq!(items(&["{\"a\": 1"]).unwrap_err(), "the stream ends inside an item");
        let mut long = Splitter::new(4);
        long.feed(b"[\"abcdef\"]");
        long.finish();
        assert_eq!(long.advance(), Step::Error);
    }
}
