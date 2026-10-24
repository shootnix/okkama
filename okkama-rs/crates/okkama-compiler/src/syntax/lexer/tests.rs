//! Lexer unit tests. Many cases are ported from `okkama-pl/t/lexer/*.t`.

use super::*;
use crate::source::SourceFile;

/// `KIND lexeme` for every token, including EOF.
fn kinds_and_lexemes(text: &str) -> Vec<String> {
    let output = lex(text);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics for {text:?}: {:?}",
        output.diagnostics
    );
    output
        .tokens
        .iter()
        .map(|token| format!("{} {}", token.kind.name(), &text[token.span.range()]))
        .collect()
}

fn kinds(text: &str) -> Vec<&'static str> {
    lex(text).tokens.iter().map(|t| t.kind.name()).collect()
}

fn single(text: &str) -> Token {
    let output = lex(text);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(output.tokens.len(), 2, "expected one token plus EOF");
    output.tokens[0].clone()
}

fn errors(text: &str) -> Vec<String> {
    lex(text)
        .diagnostics
        .iter()
        .map(|d| d.message.clone())
        .collect()
}

fn string_value(token: &Token) -> (&str, StringForm) {
    match &token.value {
        Some(TokenValue::Str(literal)) => (literal.value.as_str(), literal.form),
        other => panic!("not a string token: {other:?}"),
    }
}

#[test]
fn active_keywords() {
    let text = "module import as take native object does role fake enum test main has allow \
                private async await confirm else panic throw defer try catch for loop next last \
                if match case self true false return spawn destroy";
    assert_eq!(
        kinds(text),
        vec![
            "MODULE",
            "IMPORT",
            "AS",
            "TAKE",
            "NATIVE",
            "OBJECT",
            "DOES",
            "ROLE",
            "FAKE",
            "ENUM",
            "TEST",
            "IDENTIFIER",
            "HAS",
            "IDENTIFIER",
            "PRIVATE",
            "ASYNC",
            "AWAIT",
            "CONFIRM",
            "ELSE",
            "PANIC",
            "THROW",
            "DEFER",
            "TRY",
            "CATCH",
            "FOR",
            "LOOP",
            "NEXT",
            "LAST",
            "IF",
            "MATCH",
            "CASE",
            "SELF",
            "TRUE",
            "FALSE",
            "RETURN",
            "SPAWN",
            "DESTROY",
            "EOF",
        ]
    );
}

#[test]
fn reserved_words_tokenize_distinctly() {
    assert_eq!(
        kinds("isa super none null nil abstract protected with finally ?. ? **"),
        vec![
            "ISA",
            "SUPER",
            "NONE",
            "NULL",
            "NIL",
            "ABSTRACT",
            "PROTECTED",
            "WITH",
            "FINALLY",
            "QUESTION_DOT",
            "QUESTION",
            "STAR_STAR",
            "EOF"
        ]
    );
}

#[test]
fn contextual_and_former_words_are_identifiers() {
    for word in [
        "cap",
        "view",
        "void",
        "projected",
        "main",
        "any",
        "optional",
        "setters",
        "mutable",
        "var",
        "while",
        "allow",
        "xor",
        "native_clock",
        "nativeProvider",
        "withdrawal",
        "endl_value",
    ] {
        assert_eq!(kinds(word), vec!["IDENTIFIER", "EOF"], "{word}");
    }
}

#[test]
fn module_header_punctuation() {
    assert_eq!(
        kinds_and_lexemes("module app/api v1.0.0; @ @@ % : ^ ->"),
        vec![
            "MODULE module",
            "IDENTIFIER app",
            "SLASH /",
            "IDENTIFIER api",
            "IDENTIFIER v1",
            "DOT .",
            "FLOAT 0.0",
            "SEMICOLON ;",
            "AT @",
            "AT @",
            "AT @",
            "PERCENT %",
            "COLON :",
            "CAP ^",
            "ARROW ->",
            "EOF ",
        ]
    );
}

#[test]
fn longest_match_for_overlapping_operators() {
    assert_eq!(
        kinds("& && | || ~ ^ << >> >>> <- < <= > >= -> :: : ?. ? ** * == = != !"),
        vec![
            "AMP",
            "AMP_AMP",
            "PIPE",
            "PIPE_PIPE",
            "TILDE",
            "CAP",
            "SHIFT_LEFT",
            "SHIFT_RIGHT",
            "SHIFT_UNSIGNED",
            "STREAM_WRITE",
            "LESS",
            "LESS_EQUAL",
            "GREATER",
            "GREATER_EQUAL",
            "ARROW",
            "DOUBLE_COLON",
            "COLON",
            "QUESTION_DOT",
            "QUESTION",
            "STAR_STAR",
            "STAR",
            "EQUAL_EQUAL",
            "EQUAL",
            "BANG_EQUAL",
            "BANG",
            "EOF"
        ]
    );
    assert_eq!(
        kinds("... .. ..= ."),
        vec!["ELLIPSIS", "DOT_DOT", "DOT_DOT_EQUAL", "DOT", "EOF"]
    );
    // Adjacent punctuation is never dropped or merged incorrectly.
    assert_eq!(
        kinds(".... ..... ...="),
        vec![
            "ELLIPSIS", "DOT", "ELLIPSIS", "DOT_DOT", "ELLIPSIS", "EQUAL", "EOF"
        ]
    );
    assert_eq!(
        kinds("<<< <<= <-="),
        vec![
            "SHIFT_LEFT",
            "LESS",
            "SHIFT_LEFT",
            "EQUAL",
            "STREAM_WRITE",
            "EQUAL",
            "EOF"
        ]
    );
    assert_eq!(kinds(">>="), vec!["SHIFT_RIGHT", "EQUAL", "EOF"]);
}

#[test]
fn stream_write_versus_less_than_negative() {
    assert_eq!(
        kinds("a<-b; a < -b;"),
        vec![
            "IDENTIFIER",
            "STREAM_WRITE",
            "IDENTIFIER",
            "SEMICOLON",
            "IDENTIFIER",
            "LESS",
            "MINUS",
            "IDENTIFIER",
            "SEMICOLON",
            "EOF"
        ]
    );
}

#[test]
fn ampersand_forms_and_projection_hash() {
    assert_eq!(
        kinds("& && &Object() #projected(of) # projected"),
        vec![
            "AMP",
            "AMP_AMP",
            "AMP",
            "IDENTIFIER",
            "LEFT_PAREN",
            "RIGHT_PAREN",
            "HASH",
            "IDENTIFIER",
            "LEFT_PAREN",
            "IDENTIFIER",
            "RIGHT_PAREN",
            "HASH",
            "IDENTIFIER",
            "EOF"
        ]
    );
}

#[test]
fn integer_literals_keep_digits_and_radix() {
    let check = |text: &str, radix: Radix, digits: &str| {
        let token = single(text);
        assert_eq!(token.kind, TokenKind::Int, "{text}");
        assert_eq!(
            token.value,
            Some(TokenValue::Int(IntLiteral {
                radix,
                digits: digits.into()
            })),
            "{text}"
        );
    };
    check("0", Radix::Decimal, "0");
    check("42", Radix::Decimal, "42");
    check("007", Radix::Decimal, "007");
    check("1_000_000", Radix::Decimal, "1000000");
    check("0b1010", Radix::Binary, "1010");
    check("0B11", Radix::Binary, "11");
    check("0x2A", Radix::Hexadecimal, "2A");
    check("0XfF", Radix::Hexadecimal, "fF");
    check(
        "0X7FFF_FFFF_FFFF_FFFF",
        Radix::Hexadecimal,
        "7FFFFFFFFFFFFFFF",
    );
    // Decimal literals are not limited by a machine type in the lexer.
    check(
        "123456789012345678901234567890",
        Radix::Decimal,
        "123456789012345678901234567890",
    );
}

#[test]
fn underscores_only_between_decimal_digits() {
    assert_eq!(
        kinds_and_lexemes("1__0"),
        vec!["INT 1", "IDENTIFIER __0", "EOF "]
    );
    assert_eq!(
        kinds_and_lexemes("1_"),
        vec!["INT 1", "IDENTIFIER _", "EOF "]
    );
}

#[test]
fn malformed_based_literals_are_errors() {
    for text in [
        "0x", "0b", "0b_1", "0b1_", "0b1__0", "0b102", "0xFG", "0x1.5",
    ] {
        let messages = errors(text);
        assert_eq!(messages.len(), 1, "{text}: {messages:?}");
        assert!(
            messages[0].starts_with("Invalid integer literal"),
            "{text}: {messages:?}"
        );
    }
}

#[test]
fn based_literals_are_limited_to_int64_max() {
    assert!(errors("0x7FFFFFFFFFFFFFFF").is_empty());
    assert_eq!(
        errors("0x8000000000000000"),
        vec!["Integer literal 0x8000000000000000 exceeds the signed 64-bit maximum."]
    );
    assert_eq!(
        errors("0x1_0000_0000_0000_0000_0000_0000_0000_0000").len(),
        1
    );
}

#[test]
fn float_literals_need_digits_after_the_dot() {
    let token = single("3.14");
    assert_eq!(token.kind, TokenKind::Float);
    assert_eq!(
        token.value,
        Some(TokenValue::Float(FloatLiteral {
            whole: "3".into(),
            fraction: "14".into()
        }))
    );
    assert_eq!(kinds("1.x"), vec!["INT", "DOT", "IDENTIFIER", "EOF"]);
    assert_eq!(kinds("1..2"), vec!["INT", "DOT_DOT", "INT", "EOF"]);
    assert_eq!(kinds("1.e5"), vec!["INT", "DOT", "IDENTIFIER", "EOF"]);
    assert_eq!(kinds("1_0.2_5"), vec!["FLOAT", "EOF"]);
}

#[test]
fn string_escapes_decode_and_keep_lexeme() {
    for (text, expected) in [
        (r#""\n""#, "\n"),
        (r#""\r""#, "\r"),
        (r#""\t""#, "\t"),
        (r#""\"""#, "\""),
        (r#""\\""#, "\\"),
        (r#""\\n""#, "\\n"),
        (
            r#""say \"hello\"\nnext\\line""#,
            "say \"hello\"\nnext\\line",
        ),
        ("\"multi\nline\"", "multi\nline"),
        ("\"Unicode é 🙂\"", "Unicode é 🙂"),
    ] {
        let token = single(text);
        assert_eq!(token.kind, TokenKind::String);
        assert_eq!(
            string_value(&token),
            (expected, StringForm::Quoted),
            "{text}"
        );
        assert_eq!(&text[token.span.range()], text);
    }
}

#[test]
fn char_escapes_decode_before_the_length_check() {
    for (text, expected) in [
        (r"'\n'", '\n'),
        (r"'\r'", '\r'),
        (r"'\t'", '\t'),
        (r"'\''", '\''),
        (r"'\\'", '\\'),
        ("'\"'", '"'),
        ("'é'", 'é'),
        ("'🙂'", '🙂'),
    ] {
        let token = single(text);
        assert_eq!(token.kind, TokenKind::Char, "{text}");
        assert_eq!(token.value, Some(TokenValue::Char(expected)), "{text}");
    }
}

#[test]
fn literal_errors() {
    assert_eq!(
        errors(r#""\q""#),
        vec!["Unknown escape sequence \\q in string literal."]
    );
    assert_eq!(
        errors(r"'\q'"),
        vec!["Unknown escape sequence \\q in char literal."]
    );
    assert_eq!(
        errors("\"value\\"),
        vec!["Unfinished escape sequence in string literal."]
    );
    assert_eq!(
        errors("'x\\"),
        vec!["Unfinished escape sequence in char literal."]
    );
    assert_eq!(errors("\"open"), vec!["Unterminated string."]);
    assert_eq!(
        errors("'ab'"),
        vec!["Char literal must contain exactly one character."]
    );
    assert_eq!(
        errors("''"),
        vec!["Char literal must contain exactly one character."]
    );
    assert_eq!(
        errors("'a\n'"),
        vec!["Unterminated char.", "Unterminated char."]
    );
    assert_eq!(
        errors("'\\\"'"),
        vec!["Unknown escape sequence \\\" in char literal."]
    );
    // Combining sequences are two scalar values, hence two characters.
    assert_eq!(errors("'e\u{301}'").len(), 1);
}

#[test]
fn bad_escape_does_not_turn_the_rest_into_code() {
    let output = lex(r#""\q and more" x"#);
    assert_eq!(output.diagnostics.len(), 1);
    let kinds: Vec<_> = output.tokens.iter().map(|t| t.kind.name()).collect();
    assert_eq!(kinds, vec!["IDENTIFIER", "EOF"]);
}

#[test]
fn fenced_raw_strings() {
    let token = single("```{\"name\":\"Ada\"}```");
    assert_eq!(
        string_value(&token),
        ("{\"name\":\"Ada\"}", StringForm::Fenced)
    );
    let empty = single("``````");
    assert_eq!(string_value(&empty), ("", StringForm::Fenced));
    let text = "```\n{\n    \"quote\": \"\\\"\",\n    \"c\": \"// and /* are text */\"\n}\n```";
    let token = single(text);
    assert_eq!(
        string_value(&token).0,
        "\n{\n    \"quote\": \"\\\"\",\n    \"c\": \"// and /* are text */\"\n}\n"
    );
    assert_eq!(&text[token.span.range()], text);
}

#[test]
fn fenced_raw_strings_discard_carriage_returns() {
    let text = "```a\r\nb```";
    let token = single(text);
    assert_eq!(string_value(&token), ("a\nb", StringForm::Fenced));
    // The lexeme still covers every source byte.
    assert_eq!(&text[token.span.range()], text);
}

#[test]
fn fenced_raw_string_errors() {
    assert_eq!(
        errors("```unfinished\nvalue"),
        vec!["Unterminated fenced raw string."]
    );
    let output = lex("` value");
    assert_eq!(output.diagnostics.len(), 1);
    assert!(
        output.diagnostics[0]
            .message
            .contains("Unexpected character")
    );
    assert_eq!(
        output
            .tokens
            .iter()
            .map(|t| t.kind.name())
            .collect::<Vec<_>>(),
        vec!["IDENTIFIER", "EOF"]
    );
    assert_eq!(errors("``x").len(), 2);
}

#[test]
fn endl_is_an_exact_newline_alias() {
    let output = lex("endl endl_value \"endl\"");
    let kinds: Vec<_> = output.tokens.iter().map(|t| t.kind.name()).collect();
    assert_eq!(kinds, vec!["STRING", "IDENTIFIER", "STRING", "EOF"]);
    assert_eq!(string_value(&output.tokens[0]), ("\n", StringForm::Endl));
    assert_eq!(
        string_value(&output.tokens[2]),
        ("endl", StringForm::Quoted)
    );
}

#[test]
fn comments_are_skipped_and_do_not_nest() {
    assert_eq!(
        kinds("a // ^ xor ~ \"x\nb /* c /* d */ e */"),
        vec![
            "IDENTIFIER",
            "IDENTIFIER",
            "IDENTIFIER",
            "STAR",
            "SLASH",
            "EOF"
        ]
    );
    assert_eq!(
        errors("a /* never closed"),
        vec!["Unterminated block comment."]
    );
    assert_eq!(
        kinds("\"^ & | << >> <-\" // ^ xor ~\n xor"),
        vec!["STRING", "IDENTIFIER", "EOF"]
    );
}

#[test]
fn unexpected_characters_are_reported_and_skipped() {
    let output = lex("a $ b é c \u{feff}");
    assert_eq!(output.diagnostics.len(), 3);
    let kinds: Vec<_> = output.tokens.iter().map(|t| t.kind.name()).collect();
    assert_eq!(kinds, vec!["IDENTIFIER", "IDENTIFIER", "IDENTIFIER", "EOF"]);
    // The non-ASCII character is skipped as a whole character.
    assert_eq!(output.diagnostics[1].span, Span::new(6, 8));
}

#[test]
fn whitespace_includes_cr_and_tabs_only() {
    assert_eq!(kinds("a\r\n\tb"), vec!["IDENTIFIER", "IDENTIFIER", "EOF"]);
    assert_eq!(errors("a\u{0c}b").len(), 1, "form feed is not whitespace");
}

#[test]
fn token_spans_map_to_lines_and_columns() {
    let text = "module a v1.0.0;\r\n  sink\n  <- \"é\" 🙂x";
    let source = SourceFile::new("t.ok", text).unwrap();
    let output = lex(text);
    let positions: Vec<_> = output
        .tokens
        .iter()
        .map(|t| {
            let at = source.location(t.span.start);
            format!("{} {}:{}", t.kind.name(), at.line, at.column)
        })
        .collect();
    assert_eq!(
        positions,
        vec![
            "MODULE 1:1",
            "IDENTIFIER 1:8",
            "IDENTIFIER 1:10",
            "DOT 1:12",
            "FLOAT 1:13",
            "SEMICOLON 1:16",
            "IDENTIFIER 2:3",
            "STREAM_WRITE 3:3",
            "STRING 3:6",
            "IDENTIFIER 3:11",
            "EOF 3:12",
        ]
    );
    // The emoji is an unexpected character at 3:10.
    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(source.location(output.diagnostics[0].span.start).column, 10);
}

#[test]
fn line_accounting_across_multiline_literals_and_comments() {
    let text = "```first\nsecond```\n/* a\nb */ \"x\ny\" next";
    let source = SourceFile::new("t.ok", text).unwrap();
    let output = lex(text);
    let next = &output.tokens[2];
    assert_eq!(next.kind, TokenKind::Next);
    assert_eq!(source.location(next.span.start).line, 5);
}

#[test]
fn eof_is_always_last_with_an_empty_span() {
    for text in ["", "a", "\"open", "/* open", "```open", "é"] {
        let output = lex(text);
        let last = output.tokens.last().unwrap();
        assert_eq!(last.kind, TokenKind::Eof, "{text:?}");
        assert_eq!(last.span, Span::empty(text.len() as u32), "{text:?}");
        assert_eq!(
            output
                .tokens
                .iter()
                .filter(|t| t.kind == TokenKind::Eof)
                .count(),
            1
        );
    }
}

#[test]
fn spans_are_contiguous_with_source_spelling() {
    let text = "a.b(1, \"x\")...[k: v]";
    let output = lex(text);
    let joined: String = output
        .tokens
        .iter()
        .map(|t| &text[t.span.range()])
        .collect::<Vec<_>>()
        .join("");
    assert_eq!(joined, text.replace(' ', ""));
}

#[test]
fn lexer_terminates_on_arbitrary_bytes() {
    // A small deterministic pseudo-random sweep over printable and special
    // characters; the property is "no panic, EOF last".
    let alphabet: Vec<char> = "ab_09.'\"`\\/*\n\r\t <>=-!&|^~@#%:;,()[]{}é🙂\u{0}"
        .chars()
        .collect();
    let mut state: u64 = 0x2545_f491_4f6c_dd1d;
    for _ in 0..2000 {
        let mut text = String::new();
        for _ in 0..(state % 40) {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            text.push(alphabet[(state % alphabet.len() as u64) as usize]);
        }
        let output = lex(&text);
        assert_eq!(output.tokens.last().map(|t| t.kind), Some(TokenKind::Eof));
        for token in &output.tokens {
            assert!(text.get(token.span.range()).is_some(), "{text:?}");
        }
    }
}
