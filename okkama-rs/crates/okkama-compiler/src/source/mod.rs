//! Source text, byte spans and human-facing positions.
//!
//! Every position inside the compiler is a byte offset into the UTF-8 source
//! text. Byte offsets are cheap, unambiguous and slice-compatible with `&str`.
//! Human-facing line and column numbers are derived from them only when a
//! diagnostic or a printer needs them, using one rule for the whole crate:
//!
//! * lines are separated by LF (`\n`) only; a CR (`\r`) is an ordinary
//!   character of the line it ends, so CRLF files get the same line numbers as
//!   LF files and a lone CR does not start a line;
//! * lines and columns are 1-based;
//! * a column counts Unicode scalar values from the start of the line, so a
//!   tab, a CR, `é` and `🙂` each occupy exactly one column.

use std::fmt;

/// A half-open byte range `start..end` in one source text.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub const fn new(start: u32, end: u32) -> Self {
        Span { start, end }
    }

    /// An empty span located at `offset` (used for the end of input).
    pub const fn empty(offset: u32) -> Self {
        Span {
            start: offset,
            end: offset,
        }
    }

    /// The smallest span covering both `self` and `other`.
    pub fn to(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    pub const fn len(self) -> u32 {
        self.end - self.start
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    pub fn range(self) -> std::ops::Range<usize> {
        self.start as usize..self.end as usize
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

/// A 1-based line and column (columns count Unicode scalar values).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LineColumn {
    pub line: u32,
    pub column: u32,
}

impl fmt::Display for LineColumn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// One source text with its display name and a precomputed line index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    name: String,
    text: String,
    /// Byte offset of the first byte of every line.
    line_starts: Vec<u32>,
}

/// Error returned when a source text is too large for 32-bit spans.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceTooLarge {
    pub len: usize,
}

impl fmt::Display for SourceTooLarge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "source text has {} bytes; at most {} bytes are supported",
            self.len,
            u32::MAX
        )
    }
}

impl std::error::Error for SourceTooLarge {}

impl SourceFile {
    /// Creates a source file. Fails only when the text does not fit into
    /// 32-bit byte offsets (4 GiB).
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> Result<Self, SourceTooLarge> {
        let text = text.into();
        if u32::try_from(text.len()).is_err() {
            return Err(SourceTooLarge { len: text.len() });
        }
        let mut line_starts = vec![0];
        for (index, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                // `index + 1` fits: the whole text length fits into u32.
                line_starts.push((index + 1) as u32);
            }
        }
        Ok(SourceFile {
            name: name.into(),
            text,
            line_starts,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn len(&self) -> u32 {
        self.text.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// The exact source spelling covered by `span`.
    ///
    /// Spans produced by the lexer and parser always lie on character
    /// boundaries; a foreign span that does not yields an empty string
    /// instead of panicking.
    pub fn slice(&self, span: Span) -> &str {
        self.text.get(span.range()).unwrap_or("")
    }

    /// Number of lines (a trailing LF starts one more, empty line).
    pub fn line_count(&self) -> u32 {
        self.line_starts.len() as u32
    }

    /// Converts a byte offset into a 1-based line and column.
    ///
    /// Offsets past the end are clamped to the end of the text; an offset in
    /// the middle of a multi-byte character is attributed to that character.
    pub fn location(&self, offset: u32) -> LineColumn {
        let offset = offset.min(self.len());
        let line_index = match self.line_starts.binary_search(&offset) {
            Ok(index) => index,
            Err(index) => index - 1,
        };
        let line_start = self.line_starts[line_index] as usize;
        let prefix = &self.text.as_bytes()[line_start..offset as usize];
        // Count characters by their leading bytes; continuation bytes
        // (0b10xx_xxxx) do not start a new scalar value.
        let column = prefix.iter().filter(|byte| (**byte & 0xC0) != 0x80).count() as u32 + 1;
        LineColumn {
            line: line_index as u32 + 1,
            column,
        }
    }

    /// The text of a 1-based line without its LF and without a final CR.
    pub fn line_text(&self, line: u32) -> &str {
        let Some(&start) = self.line_starts.get(line.saturating_sub(1) as usize) else {
            return "";
        };
        let end = self
            .line_starts
            .get(line as usize)
            .map_or(self.len(), |next| next - 1);
        let text = &self.text[start as usize..end as usize];
        text.strip_suffix('\r').unwrap_or(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(text: &str) -> SourceFile {
        SourceFile::new("test.ok", text).unwrap()
    }

    #[test]
    fn locations_count_scalar_values_and_split_on_lf() {
        let source = file("ab\n\tc\r\nйx🙂y\n");
        assert_eq!(source.location(0), LineColumn { line: 1, column: 1 });
        assert_eq!(source.location(2), LineColumn { line: 1, column: 3 });
        assert_eq!(source.location(3), LineColumn { line: 2, column: 1 });
        // The tab occupies exactly one column.
        assert_eq!(source.location(4), LineColumn { line: 2, column: 2 });
        // CR belongs to line 2; the next line starts after LF.
        assert_eq!(source.location(5), LineColumn { line: 2, column: 3 });
        assert_eq!(source.location(7), LineColumn { line: 3, column: 1 });
        // "й" is two bytes, "🙂" is four bytes; each is one column.
        let y = source.text().find('y').unwrap() as u32;
        assert_eq!(source.location(y), LineColumn { line: 3, column: 4 });
        assert_eq!(source.line_count(), 4);
    }

    #[test]
    fn lone_cr_does_not_start_a_line() {
        let source = file("a\rb");
        assert_eq!(source.location(2), LineColumn { line: 1, column: 3 });
    }

    #[test]
    fn line_text_strips_line_terminators() {
        let source = file("first\r\nsecond\nthird");
        assert_eq!(source.line_text(1), "first");
        assert_eq!(source.line_text(2), "second");
        assert_eq!(source.line_text(3), "third");
        assert_eq!(source.line_text(4), "");
    }

    #[test]
    fn offsets_past_the_end_are_clamped() {
        let source = file("ab");
        assert_eq!(source.location(99), LineColumn { line: 1, column: 3 });
    }

    #[test]
    fn slice_never_panics_on_foreign_spans() {
        let source = file("é");
        assert_eq!(source.slice(Span::new(0, 1)), "");
        assert_eq!(source.slice(Span::new(0, 2)), "é");
        assert_eq!(source.slice(Span::new(5, 9)), "");
    }
}
