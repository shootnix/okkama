use super::{Diagnostic, DiagnosticKind as K, Limits, Span};

#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub text: String,
    pub quoted: bool,
    pub character: bool,
    pub span: Span,
}

/// Lexing is iterative, with a bounded delimiter stack. `#` is a symbol byte.
pub(crate) fn lex(input: &str, offset: usize, limits: Limits) -> Result<Vec<Token>, Diagnostic> {
    let mut tokens = Vec::new();
    let mut pos = offset;
    let mut stack = Vec::new();
    let bytes = input.as_bytes();
    while pos < bytes.len() {
        let start = pos;
        let b = bytes[pos];
        let error = |kind, end, message| Diagnostic::new(kind, Span { start, end }, message);
        if b == b' ' || b == b'\t' {
            pos += 1;
            continue;
        }
        if b == b'/' && bytes.get(pos + 1) == Some(&b'/') {
            while pos < bytes.len() && bytes[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }
        if b == b'\r' && bytes.get(pos + 1) != Some(&b'\n') {
            return Err(error(K::Lexical, start + 1, "bare CR"));
        }
        if b == b'\r' {
            pos += 1;
            continue;
        }
        let quoted = b == b'"' || b == b'\'';
        let text = if quoted {
            quoted_text(input, &mut pos, limits)?
        } else if b == b'@' {
            pos += 1;
            while let Some(c) = input[pos..].chars().next() {
                if !c.is_alphanumeric() && !"_.#:".contains(c) {
                    break;
                }
                pos += c.len_utf8();
            }
            input[start..pos].to_owned()
        } else if b == b':' && bytes.get(pos + 1) == Some(&b':') {
            pos += 2;
            "::".into()
        } else if b"()[]{},:=!;<>^\n".contains(&b) {
            pos += 1;
            char::from(b).to_string()
        } else if b == b'-' && bytes.get(pos + 1) == Some(&b'>') {
            pos += 2;
            "->".into()
        } else {
            while pos < bytes.len() && !bytes[pos].is_ascii_whitespace() {
                if bytes[pos] == b':' && bytes.get(pos + 1) == Some(&b':') {
                    pos += 2;
                    continue;
                }
                if b"()[]{},:=!;<>^'\"".contains(&bytes[pos]) {
                    break;
                }
                if bytes[pos] == b'/' && bytes.get(pos + 1) == Some(&b'/') {
                    break;
                }
                pos += 1;
            }
            if pos == start {
                return Err(error(K::Lexical, start + 1, "unexpected character"));
            }
            let text = &input[start..pos];
            if text.chars().any(|c| c.is_control() || c == '\u{feff}') {
                return Err(error(K::Lexical, pos, "invalid token"));
            }
            text.to_owned()
        };
        if pos - start > limits.max_literal_bytes {
            return Err(error(K::Limit, pos, "token length limit"));
        }
        if !quoted {
            match text.as_str() {
                "(" | "[" | "{" | "<" => {
                    stack.push(text.clone());
                    if stack.len() > limits.max_nesting.min(128) {
                        return Err(error(K::Limit, pos, "nesting limit"));
                    }
                }
                ")" | "]" | "}" | ">" => {
                    let expected = match text.as_str() {
                        ")" => "(",
                        "]" => "[",
                        "}" => "{",
                        _ => "<",
                    };
                    if stack.pop().as_deref() != Some(expected) {
                        return Err(error(K::Syntax, pos, "mismatched delimiter"));
                    }
                }
                _ => {}
            }
        }
        if tokens.len() >= limits.max_tokens {
            return Err(error(K::Limit, pos, "token count limit"));
        }
        tokens.push(Token {
            text,
            quoted,
            character: quoted && b == b'\'',
            span: Span { start, end: pos },
        });
    }
    if !stack.is_empty() {
        return Err(Diagnostic::new(K::Syntax, Span { start: pos, end: pos }, "unclosed delimiter"));
    }
    tokens.push(Token {
        text: "<eof>".into(),
        quoted: false,
        character: false,
        span: Span { start: pos, end: pos },
    });
    Ok(tokens)
}

fn quoted_text(input: &str, pos: &mut usize, limits: Limits) -> Result<String, Diagnostic> {
    let bytes = input.as_bytes();
    let start = *pos;
    let quote = char::from(bytes[start]);
    *pos += 1;
    let mut out = String::new();
    loop {
        let error = |message| Diagnostic::new(K::Lexical, Span { start, end: *pos }, message);
        let c = input[*pos..].chars().next().ok_or_else(|| error("unterminated literal"))?;
        *pos += c.len_utf8();
        match c {
            c if c == quote => break,
            '\n' | '\r' => return Err(error_at(start, *pos, "newline in literal")),
            '\\' => {
                let escaped = input[*pos..].chars().next()
                    .ok_or_else(|| error_at(start, *pos, "unfinished escape"))?;
                *pos += escaped.len_utf8();
                out.push(match escaped {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '0' => '\0',
                    '\\' => '\\',
                    '"' => '"',
                    '\'' => '\'',
                    'u' => unicode_escape(input, pos, start)?,
                    _ => return Err(error_at(start, *pos, "unknown escape")),
                });
            }
            c if c.is_control() => return Err(error_at(start, *pos, "control character in literal")),
            c => out.push(c),
        }
        if *pos - start > limits.max_literal_bytes {
            return Err(Diagnostic::new(K::Limit, Span { start, end: *pos }, "literal length limit"));
        }
    }
    Ok(out)
}

fn unicode_escape(input: &str, pos: &mut usize, start: usize) -> Result<char, Diagnostic> {
    let bytes = input.as_bytes();
    if bytes.get(*pos) != Some(&b'{') {
        return Err(error_at(start, *pos, "expected Unicode escape"));
    }
    *pos += 1;
    let hex_start = *pos;
    while *pos < bytes.len() && bytes[*pos].is_ascii_hexdigit() && *pos - hex_start < 6 {
        *pos += 1;
    }
    let scalar = u32::from_str_radix(&input[hex_start..*pos], 16).ok().and_then(char::from_u32);
    if bytes.get(*pos) != Some(&b'}') {
        return Err(error_at(start, *pos, "invalid Unicode escape"));
    }
    *pos += 1;
    scalar.ok_or_else(|| error_at(start, *pos, "invalid Unicode scalar"))
}

fn error_at(start: usize, end: usize, message: &str) -> Diagnostic {
    Diagnostic::new(K::Lexical, Span { start, end }, message)
}
