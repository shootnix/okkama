use super::{Diagnostic, DiagnosticKind as K, Limits, Span};

#[derive(Clone, Debug)]
pub(crate) struct Token { pub text: String, pub quoted: bool, pub span: Span }

/// Lexing is iterative, with a bounded delimiter stack. `#` is a symbol byte.
pub(crate) fn lex(input: &str, offset: usize, limits: Limits) -> Result<Vec<Token>, Diagnostic> {
    let mut tokens = Vec::new();
    let mut pos = offset;
    let mut stack = Vec::new();
    let bytes = input.as_bytes();
    while pos < bytes.len() {
        let start = pos;
        let b = bytes[pos];
        if b == b' ' || b == b'\t' { pos += 1; continue; }
        if b == b'/' && bytes.get(pos + 1) == Some(&b'/') {
            while pos < bytes.len() && bytes[pos] != b'\n' { pos += 1; }
            continue;
        }
        if b == b'\r' && bytes.get(pos + 1) != Some(&b'\n') {
            return Err(Diagnostic::new(K::Lexical, Span { start, end: start + 1 }, "bare CR"));
        }
        if b == b'\r' { pos += 1; continue; }
        let (text, quoted) = if b == b'"' {
            pos += 1;
            let mut out = String::new();
            loop {
                let Some(c) = input[pos..].chars().next() else {
                    return Err(Diagnostic::new(K::Lexical, Span { start, end: pos }, "unterminated string"));
                };
                pos += c.len_utf8();
                match c {
                    '"' => break,
                    '\n' | '\r' => return Err(Diagnostic::new(K::Lexical, Span { start, end: pos }, "newline in string")),
                    '\\' => {
                        let Some(escaped) = input[pos..].chars().next() else {
                            return Err(Diagnostic::new(K::Lexical, Span { start, end: pos }, "unfinished escape"));
                        };
                        pos += escaped.len_utf8();
                        out.push(match escaped {
                            'n' => '\n', 'r' => '\r', 't' => '\t', '0' => '\0', '\\' => '\\', '"' => '"',
                            'u' => {
                                if bytes.get(pos) != Some(&b'{') { return Err(Diagnostic::new(K::Lexical, Span { start, end: pos }, "expected Unicode escape")); }
                                pos += 1;
                                let hex_start = pos;
                                while pos < bytes.len() && bytes[pos].is_ascii_hexdigit() && pos - hex_start < 6 { pos += 1; }
                                let scalar = u32::from_str_radix(&input[hex_start..pos], 16).ok().and_then(char::from_u32);
                                if bytes.get(pos) != Some(&b'}') || scalar.is_none() { return Err(Diagnostic::new(K::Lexical, Span { start, end: pos }, "invalid Unicode escape")); }
                                pos += 1;
                                scalar.ok_or_else(|| Diagnostic::new(K::Lexical, Span { start, end: pos }, "invalid scalar"))?
                            }
                            _ => return Err(Diagnostic::new(K::Lexical, Span { start, end: pos }, "unknown escape")),
                        });
                    }
                    c if c.is_control() => return Err(Diagnostic::new(K::Lexical, Span { start, end: pos }, "control character in string")),
                    c => out.push(c),
                }
                if pos - start > limits.max_literal_bytes { return Err(Diagnostic::new(K::Limit, Span { start, end: pos }, "literal length limit")); }
            }
            (out, true)
        } else if b == b'@' {
            pos += 1;
            while let Some(c) = input[pos..].chars().next() {
                if c.is_alphanumeric() || "_.#:".contains(c) { pos += c.len_utf8(); } else { break; }
            }
            (input[start..pos].to_owned(), false)
        } else if b"()[]{},:=!;<>\n".contains(&b) {
            pos += 1;
            (char::from(b).to_string(), false)
        } else if b == b'-' && bytes.get(pos + 1) == Some(&b'>') {
            pos += 2;
            ("->".into(), false)
        } else {
            while pos < bytes.len() && !bytes[pos].is_ascii_whitespace() && !b"()[]{},:=!;<>\"".contains(&bytes[pos]) {
                if bytes[pos] == b'/' && bytes.get(pos + 1) == Some(&b'/') { break; }
                // Colons belong to qualified function symbols, including ::.
                pos += 1;
            }
            if pos == start { return Err(Diagnostic::new(K::Lexical, Span { start, end: start + 1 }, "unexpected character")); }
            if pos - start > limits.max_literal_bytes { return Err(Diagnostic::new(K::Limit, Span { start, end: pos }, "token length limit")); }
            let text = &input[start..pos];
            if text.chars().any(|c| c.is_control() || c == '\u{feff}') { return Err(Diagnostic::new(K::Lexical, Span { start, end: pos }, "invalid token")); }
            (text.to_owned(), false)
        };
        if pos - start > limits.max_literal_bytes { return Err(Diagnostic::new(K::Limit, Span { start, end: pos }, "token length limit")); }
        if !quoted {
            match text.as_str() {
                "(" | "[" | "{" | "<" => {
                    stack.push(text.clone());
                    if stack.len() > limits.max_nesting.min(128) { return Err(Diagnostic::new(K::Limit, Span { start, end: pos }, "nesting limit")); }
                }
                ")" | "]" | "}" | ">" => {
                    let expected = match text.as_str() { ")" => "(", "]" => "[", "}" => "{", _ => "<" };
                    if stack.pop().as_deref() != Some(expected) { return Err(Diagnostic::new(K::Syntax, Span { start, end: pos }, "mismatched delimiter")); }
                }
                _ => {}
            }
        }
        if tokens.len() >= limits.max_tokens { return Err(Diagnostic::new(K::Limit, Span { start, end: pos }, "token count limit")); }
        tokens.push(Token { text, quoted, span: Span { start, end: pos } });
    }
    if !stack.is_empty() { return Err(Diagnostic::new(K::Syntax, Span { start: pos, end: pos }, "unclosed delimiter")); }
    tokens.push(Token { text: "<eof>".into(), quoted: false, span: Span { start: pos, end: pos } });
    Ok(tokens)
}
