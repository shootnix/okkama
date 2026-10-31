//! The Okkama lexer.
//!
//! [`lex`] turns a source text into tokens plus lexical diagnostics. It never
//! panics and always terminates: every loop iteration consumes at least one
//! character, and every scanner for a multi-character construct (strings,
//! chars, fenced strings, comments) stops at the end of the input.
//!
//! The scanner follows the reference lexer (`okkama-pl/lib/Okkama/Lexer.pm`)
//! token for token. Notable rules:
//!
//! * operators use longest match with one character of lookahead, e.g.
//!   `<=`/`<-`/`<<`, `..`/`..=`/`...`, `>`/`>=`/`>>`/`>>>`;
//! * `endl` is an exact alias for the string literal `"\n"`;
//! * identifiers are ASCII; any other character outside literals and
//!   comments is an "unexpected character" error;
//! * integer literals keep their digits as text (see
//!   [`IntLiteral`](crate::syntax::token::IntLiteral)); only based literals
//!   are range-checked, because the grammar limits them to `INT64_MAX`;
//! * carriage returns inside fenced raw strings are dropped from the decoded
//!   value (language-owner decision D-030, "as in Go"); the lexeme keeps them.
//!
//! After an error the lexer resynchronises locally (it finishes the current
//! literal or skips the bad character) so one mistake produces one
//! diagnostic instead of a cascade.

use crate::diagnostics::Diagnostic;
use crate::source::Span;
use crate::syntax::token::{
    FloatLiteral, IntLiteral, Radix, StrLiteral, StringForm, Token, TokenKind, TokenValue,
};

#[cfg(test)]
mod tests;

/// The result of lexing one source text.
#[derive(Clone, Debug, Default)]
pub struct LexOutput {
    /// All tokens, always terminated by exactly one `Eof` token.
    pub tokens: Vec<Token>,
    /// Lexical errors in source order. Empty for a valid source.
    pub diagnostics: Vec<Diagnostic>,
}

impl LexOutput {
    pub fn has_errors(&self) -> bool {
        !self.diagnostics.is_empty()
    }
}

/// Lexes `text`.
///
/// Texts longer than `u32::MAX` bytes cannot be addressed by spans; callers
/// create a [`SourceFile`](crate::source::SourceFile) first, which rejects
/// such texts. For robustness the lexer reports them as a diagnostic too.
pub fn lex(text: &str) -> LexOutput {
    if u32::try_from(text.len()).is_err() {
        return LexOutput {
            tokens: vec![Token::new(TokenKind::Eof, Span::empty(0))],
            diagnostics: vec![Diagnostic::lexical(
                "Source text is too large (more than 4 GiB).",
                Span::empty(0),
            )],
        };
    }
    let mut lexer = Lexer {
        text,
        bytes: text.as_bytes(),
        start: 0,
        pos: 0,
        tokens: Vec::new(),
        diagnostics: Vec::new(),
    };
    lexer.run();
    LexOutput {
        tokens: lexer.tokens,
        diagnostics: lexer.diagnostics,
    }
}

struct Lexer<'a> {
    text: &'a str,
    bytes: &'a [u8],
    /// Start of the token being scanned.
    start: usize,
    /// Next unread byte.
    pos: usize,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl Lexer<'_> {
    fn run(&mut self) {
        while !self.at_end() {
            self.start = self.pos;
            self.scan_token();
        }
        let end = self.offset(self.text.len());
        self.tokens
            .push(Token::new(TokenKind::Eof, Span::empty(end)));
    }

    // ----- cursor helpers -------------------------------------------------

    fn at_end(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    /// The byte at `pos`, or 0 at the end. Every byte the scanner dispatches
    /// on is ASCII, so byte-level lookahead is enough; multi-byte characters
    /// are only ever consumed as a whole by `advance_char`.
    fn peek(&self) -> u8 {
        self.bytes.get(self.pos).copied().unwrap_or(0)
    }

    fn peek_next(&self) -> u8 {
        self.bytes.get(self.pos + 1).copied().unwrap_or(0)
    }

    fn eat(&mut self, expected: u8) -> bool {
        if self.peek() == expected && !self.at_end() {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// Consumes and returns one complete character (`None` at the end).
    fn advance_char(&mut self) -> Option<char> {
        let ch = self.text[self.pos..].chars().next()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn offset(&self, index: usize) -> u32 {
        // `lex` rejected texts whose length does not fit into u32.
        index as u32
    }

    fn span_from(&self, start: usize) -> Span {
        Span::new(self.offset(start), self.offset(self.pos))
    }

    fn add(&mut self, kind: TokenKind) {
        let span = self.span_from(self.start);
        self.tokens.push(Token::new(kind, span));
    }

    fn add_value(&mut self, kind: TokenKind, value: TokenValue) {
        let span = self.span_from(self.start);
        self.tokens.push(Token {
            kind,
            span,
            value: Some(value),
        });
    }

    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.diagnostics.push(Diagnostic::lexical(message, span));
    }

    // ----- dispatch -------------------------------------------------------

    fn scan_token(&mut self) {
        use TokenKind as K;
        let byte = self.peek();
        if !byte.is_ascii() {
            // Non-ASCII text is only valid inside literals and comments.
            let ch = self.advance_char().unwrap_or('\u{fffd}');
            let span = self.span_from(self.start);
            self.error(
                format!(
                    "Unexpected character {} (U+{:04X}).",
                    describe(ch),
                    ch as u32
                ),
                span,
            );
            return;
        }
        self.pos += 1;
        match byte {
            b'+' => self.add(K::Plus),
            b'-' => {
                let kind = if self.eat(b'>') { K::Arrow } else { K::Minus };
                self.add(kind);
            }
            b'*' => {
                let kind = if self.eat(b'*') { K::StarStar } else { K::Star };
                self.add(kind);
            }
            b'%' => self.add(K::Percent),
            b'@' => self.add(K::At),
            b'#' => self.add(K::Hash),
            b'&' => {
                let kind = if self.eat(b'&') { K::AmpAmp } else { K::Amp };
                self.add(kind);
            }
            b'|' => {
                let kind = if self.eat(b'|') { K::PipePipe } else { K::Pipe };
                self.add(kind);
            }
            b'~' => self.add(K::Tilde),
            b'/' => {
                if self.eat(b'/') {
                    self.line_comment();
                } else if self.eat(b'*') {
                    self.block_comment();
                } else {
                    self.add(K::Slash);
                }
            }
            b'"' => self.string(),
            b'\'' => self.char_literal(),
            b'`' => {
                if self.bytes[self.pos..].starts_with(b"``") {
                    self.pos += 2;
                    self.fenced_string();
                } else {
                    let span = self.span_from(self.start);
                    self.error(
                        "Unexpected character \"`\"; fenced raw strings use three backticks.",
                        span,
                    );
                }
            }
            b'(' => self.add(K::LeftParen),
            b')' => self.add(K::RightParen),
            b'{' => self.add(K::LeftBrace),
            b'}' => self.add(K::RightBrace),
            b'[' => self.add(K::LeftBracket),
            b']' => self.add(K::RightBracket),
            b';' => self.add(K::Semicolon),
            b',' => self.add(K::Comma),
            b'.' => {
                let kind = if self.eat(b'.') {
                    if self.eat(b'.') {
                        K::Ellipsis
                    } else if self.eat(b'=') {
                        K::DotDotEqual
                    } else {
                        K::DotDot
                    }
                } else {
                    K::Dot
                };
                self.add(kind);
            }
            b':' => {
                let kind = if self.eat(b':') {
                    K::DoubleColon
                } else {
                    K::Colon
                };
                self.add(kind);
            }
            b'?' => {
                let kind = if self.eat(b'.') {
                    K::QuestionDot
                } else {
                    K::Question
                };
                self.add(kind);
            }
            b'^' => self.add(K::Caret),
            b'!' => {
                let kind = if self.eat(b'=') {
                    K::BangEqual
                } else {
                    K::Bang
                };
                self.add(kind);
            }
            b'=' => {
                let kind = if self.eat(b'=') {
                    K::EqualEqual
                } else {
                    K::Equal
                };
                self.add(kind);
            }
            b'<' => {
                // The order matters only for documentation: each alternative
                // looks at a different second character.
                let kind = if self.eat(b'=') {
                    K::LessEqual
                } else if self.eat(b'-') {
                    K::StreamWrite
                } else if self.eat(b'<') {
                    K::ShiftLeft
                } else {
                    K::Less
                };
                self.add(kind);
            }
            b'>' => {
                // `>>` and `>>>` stay single tokens here; the type parser
                // splits them when it closes nested generic argument lists.
                let kind = if self.eat(b'=') {
                    K::GreaterEqual
                } else if self.eat(b'>') {
                    if self.eat(b'>') {
                        K::ShiftUnsigned
                    } else {
                        K::ShiftRight
                    }
                } else {
                    K::Greater
                };
                self.add(kind);
            }
            b' ' | b'\r' | b'\t' | b'\n' => {}
            b'0'..=b'9' => self.number(byte),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.word(),
            other => {
                let span = self.span_from(self.start);
                self.error(
                    format!(
                        "Unexpected character {} (U+{:04X}).",
                        describe(other as char),
                        other
                    ),
                    span,
                );
            }
        }
    }

    // ----- comments -------------------------------------------------------

    fn line_comment(&mut self) {
        // The newline itself is left for the main loop.
        match self.bytes[self.pos..].iter().position(|&b| b == b'\n') {
            Some(length) => self.pos += length,
            None => self.pos = self.bytes.len(),
        }
    }

    fn block_comment(&mut self) {
        // Block comments do not nest: the first `*/` closes the comment.
        match find(&self.bytes[self.pos..], b"*/") {
            Some(length) => self.pos += length + 2,
            None => {
                self.pos = self.bytes.len();
                let span = self.span_from(self.start);
                self.error("Unterminated block comment.", span);
            }
        }
    }

    // ----- numbers ----------------------------------------------------------

    fn number(&mut self, first: u8) {
        if first == b'0' && matches!(self.peek(), b'b' | b'B' | b'x' | b'X') {
            self.based_number();
            return;
        }
        let mut whole = String::new();
        whole.push(first as char);
        self.digits_into(&mut whole);

        // A float needs digits after the dot; `1.` and `1.x` leave the dot
        // for member access and ranges (`1..2`).
        if self.peek() == b'.' && self.peek_next().is_ascii_digit() {
            self.pos += 1;
            let mut fraction = String::new();
            self.digits_into(&mut fraction);
            self.add_value(
                TokenKind::Float,
                TokenValue::Float(FloatLiteral { whole, fraction }),
            );
            return;
        }
        self.add_value(
            TokenKind::Int,
            TokenValue::Int(IntLiteral {
                radix: Radix::Decimal,
                digits: whole,
            }),
        );
    }

    /// Consumes `[0-9]` digits and `_` separators that sit between two
    /// digits. A `_` that is not followed by a digit ends the number, so
    /// `1__0` lexes as `1` followed by the identifier `__0`.
    fn digits_into(&mut self, out: &mut String) {
        loop {
            let byte = self.peek();
            if byte.is_ascii_digit() {
                out.push(byte as char);
                self.pos += 1;
            } else if byte == b'_' && self.peek_next().is_ascii_digit() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn based_number(&mut self) {
        let marker = self.peek();
        self.pos += 1;
        // Like the reference lexer, consume the whole alphanumeric run first
        // and validate it afterwards, so `0b102` or `0xFG` is one bad token.
        while self.peek().is_ascii_alphanumeric() || self.peek() == b'_' {
            self.pos += 1;
        }
        let text = &self.text[self.start..self.pos];
        let body = &text[2..];
        let (radix, valid_digit): (Radix, fn(u8) -> bool) = match marker {
            b'b' | b'B' => (Radix::Binary, |b| b == b'0' || b == b'1'),
            _ => (Radix::Hexadecimal, |b: u8| b.is_ascii_hexdigit()),
        };
        // `digit+ ( "_" digit+ )*`: no leading, trailing or doubled `_`.
        let well_formed = !body.is_empty()
            && body
                .split('_')
                .all(|group| !group.is_empty() && group.bytes().all(valid_digit));
        // `0x1.5` is not a based float; reject it instead of lexing `.5`.
        let float_like = self.peek() == b'.' && self.peek_next().is_ascii_digit();
        let digits: String = body.chars().filter(|&c| c != '_').collect();
        let literal = IntLiteral { radix, digits };
        let span = self.span_from(self.start);
        if !well_formed || float_like {
            let text = text.to_string();
            self.error(format!("Invalid integer literal {text}."), span);
        } else if literal
            .to_u128()
            .is_none_or(|value| value > i64::MAX as u128)
        {
            let text = text.to_string();
            self.error(
                format!("Integer literal {text} exceeds the signed 64-bit maximum."),
                span,
            );
        }
        self.add_value(TokenKind::Int, TokenValue::Int(literal));
    }

    // ----- strings and chars -------------------------------------------------

    fn string(&mut self) {
        let mut value = String::new();
        let mut valid = true;
        loop {
            if self.at_end() {
                let span = self.span_from(self.start);
                self.error("Unterminated string.", span);
                return;
            }
            match self.peek() {
                b'"' => {
                    self.pos += 1;
                    if valid {
                        self.add_value(
                            TokenKind::String,
                            TokenValue::Str(StrLiteral {
                                value,
                                form: StringForm::Quoted,
                            }),
                        );
                    }
                    return;
                }
                b'\\' => {
                    let escape_start = self.pos;
                    self.pos += 1;
                    match self.advance_char() {
                        None => {
                            let span = self.span_from(escape_start);
                            self.error("Unfinished escape sequence in string literal.", span);
                            return;
                        }
                        Some('u') => match self.unicode_escape() {
                            Ok(decoded) => value.push(decoded),
                            Err(problem) => {
                                valid = false;
                                let span = self.span_from(escape_start);
                                self.error(format!("{problem} in string literal."), span);
                            }
                        },
                        Some(escape) => match decode_escape(escape, '"') {
                            Some(decoded) => value.push(decoded),
                            None => {
                                // Keep scanning to the closing quote so the
                                // rest of the literal is not lexed as code.
                                valid = false;
                                let span = self.span_from(escape_start);
                                self.error(
                                    format!(
                                        "Unknown escape sequence \\{escape} in string literal."
                                    ),
                                    span,
                                );
                            }
                        },
                    }
                }
                // Quoted strings may span lines; newlines are kept verbatim.
                _ => {
                    if let Some(ch) = self.advance_char() {
                        value.push(ch);
                    }
                }
            }
        }
    }

    fn char_literal(&mut self) {
        let mut value = String::new();
        let mut valid = true;
        loop {
            if self.at_end() {
                let span = self.span_from(self.start);
                self.error("Unterminated char.", span);
                return;
            }
            match self.peek() {
                b'\'' => {
                    self.pos += 1;
                    if !valid {
                        return;
                    }
                    // Escapes are decoded before the one-character rule, so
                    // '\n' is one character and 'ab' is two.
                    let mut chars = value.chars();
                    match (chars.next(), chars.next()) {
                        (Some(ch), None) => self.add_value(TokenKind::Char, TokenValue::Char(ch)),
                        _ => {
                            let span = self.span_from(self.start);
                            self.error("Char literal must contain exactly one character.", span);
                        }
                    }
                    return;
                }
                b'\n' => {
                    // The newline is left for the main loop: the next line
                    // lexes normally.
                    let span = self.span_from(self.start);
                    self.error("Unterminated char.", span);
                    return;
                }
                b'\\' => {
                    let escape_start = self.pos;
                    self.pos += 1;
                    match self.advance_char() {
                        None => {
                            let span = self.span_from(escape_start);
                            self.error("Unfinished escape sequence in char literal.", span);
                            return;
                        }
                        Some('u') => match self.unicode_escape() {
                            Ok(decoded) => value.push(decoded),
                            Err(problem) => {
                                valid = false;
                                let span = self.span_from(escape_start);
                                self.error(format!("{problem} in char literal."), span);
                            }
                        },
                        Some(escape) => match decode_escape(escape, '\'') {
                            Some(decoded) => value.push(decoded),
                            None => {
                                valid = false;
                                let span = self.span_from(escape_start);
                                self.error(
                                    format!("Unknown escape sequence \\{escape} in char literal."),
                                    span,
                                );
                            }
                        },
                    }
                }
                _ => {
                    if let Some(ch) = self.advance_char() {
                        value.push(ch);
                    }
                }
            }
        }
    }

    /// Decodes the rest of `\u{HEX}` after the `u`: one to six hexadecimal
    /// digits naming a Unicode scalar value. Only the braces and the digits
    /// are consumed, so a malformed escape never swallows the closing quote.
    fn unicode_escape(&mut self) -> Result<char, String> {
        const MALFORMED: &str = "Unicode escape must be \\u{HEX} with 1 to 6 hexadecimal digits";
        if !self.eat(b'{') {
            return Err(MALFORMED.into());
        }
        let digits_start = self.pos;
        while self.peek().is_ascii_hexdigit() {
            self.pos += 1;
        }
        let digits = &self.text[digits_start..self.pos];
        if digits.is_empty() || digits.len() > 6 || !self.eat(b'}') {
            return Err(MALFORMED.into());
        }
        let code = u32::from_str_radix(digits, 16).expect("hexadecimal digits");
        char::from_u32(code)
            .ok_or_else(|| format!("Unicode escape \\u{{{digits}}} is not a Unicode scalar value"))
    }

    fn fenced_string(&mut self) {
        let content_start = self.pos;
        // The first following exact triple backtick closes the literal.
        match find(&self.bytes[self.pos..], b"```") {
            Some(length) => {
                let content = &self.text[content_start..content_start + length];
                self.pos = content_start + length + 3;
                // D-030: CR characters are discarded from the value as in Go
                // raw strings; the token span still covers them.
                let value: String = content.chars().filter(|&c| c != '\r').collect();
                self.add_value(
                    TokenKind::String,
                    TokenValue::Str(StrLiteral {
                        value,
                        form: StringForm::Fenced,
                    }),
                );
            }
            None => {
                self.pos = self.bytes.len();
                let span = self.span_from(self.start);
                self.error("Unterminated fenced raw string.", span);
            }
        }
    }

    // ----- words --------------------------------------------------------------

    fn word(&mut self) {
        while self.peek().is_ascii_alphanumeric() || self.peek() == b'_' {
            self.pos += 1;
        }
        let text = &self.text[self.start..self.pos];
        if text == "endl" {
            self.add_value(
                TokenKind::String,
                TokenValue::Str(StrLiteral {
                    value: "\n".to_string(),
                    form: StringForm::Endl,
                }),
            );
            return;
        }
        let kind = TokenKind::keyword(text).unwrap_or(TokenKind::Identifier);
        self.add(kind);
    }
}

/// Decodes the character after a backslash. `quote` is the delimiter of the
/// literal being scanned (`"` for strings, `'` for chars); the other quote
/// character is not an escape.
fn decode_escape(escape: char, quote: char) -> Option<char> {
    match escape {
        'n' => Some('\n'),
        'r' => Some('\r'),
        't' => Some('\t'),
        '\\' => Some('\\'),
        c if c == quote => Some(c),
        _ => None,
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn describe(ch: char) -> String {
    if ch.is_control() || ch.is_whitespace() {
        format!("{:?}", ch)
    } else {
        format!("\"{ch}\"")
    }
}
