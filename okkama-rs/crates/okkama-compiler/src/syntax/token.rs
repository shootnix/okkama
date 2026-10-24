//! Tokens produced by the lexer.
//!
//! A token stores its kind and its byte span; the exact source spelling is
//! always recovered from the span (`SourceFile::slice`). Only literal tokens
//! carry a decoded value, and even that value keeps the semantic
//! interpretation out of the lexer: integers keep their digit string and
//! radix instead of being squeezed into a Rust machine integer.
//!
//! Token kind names (`TokenKind::name`) intentionally match the token type
//! names of the Perl reference implementation (`Okkama::TokenType`), which
//! keeps token dumps directly comparable.

use std::fmt;

use crate::source::Span;

/// Every token kind the lexer can produce.
///
/// The lexer recognises active syntax *and* removed or deferred spellings
/// (`null`, `?.`, `**`, `isa`, ...). The latter are never accepted by the
/// parser; they exist so that it can explain why the code is invalid instead
/// of treating the word as an ordinary identifier.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TokenKind {
    // Literals and names.
    Int,
    Float,
    String,
    Char,
    Identifier,

    // Arithmetic.
    Plus,
    Minus,
    Slash,
    Star,
    /// `**` — removed exponent operator (reserved).
    StarStar,
    Percent,

    // Bitwise, logic and punctuation.
    Hash,
    Amp,
    AmpAmp,
    Pipe,
    PipePipe,
    Tilde,
    Arrow,
    ShiftLeft,
    ShiftRight,
    ShiftUnsigned,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Semicolon,
    Comma,
    Dot,
    DotDot,
    DotDotEqual,
    Ellipsis,
    Colon,
    DoubleColon,
    /// `?` — removed optional-type marker (reserved).
    Question,
    /// `?.` — removed optional chaining (reserved).
    QuestionDot,
    /// `^` — successor marker and bitwise XOR.
    Caret,
    At,

    // Comparison, equality and stream write.
    Bang,
    BangEqual,
    Equal,
    EqualEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    /// `<-`
    StreamWrite,

    // Keywords of the active language.
    Else,
    False,
    For,
    Loop,
    If,
    True,
    Let,
    Object,
    Is,
    As,
    Does,
    Role,
    Import,
    Has,
    Confirm,
    Private,
    Enum,
    Match,
    Case,
    Defer,
    Try,
    Catch,
    Next,
    Last,
    Async,
    Await,
    Spawn,
    Panic,
    Throw,
    Return,
    SelfKw,
    Module,
    Take,
    Native,
    Test,
    Fake,
    Destroy,

    // Removed or deferred words (reserved: always rejected by the parser).
    Isa,
    Super,
    None,
    Null,
    Nil,
    Abstract,
    Protected,
    With,
    Finally,

    /// End of input. Always the last token, with an empty span.
    Eof,
}

impl TokenKind {
    /// The kind for an exact keyword spelling, if the word is a keyword.
    ///
    /// Keyword-ness depends on the exact text only. Contextual words such as
    /// `cap`, `view`, `void`, `projected`, `main` or `any` are identifiers;
    /// the parser gives them meaning by position.
    pub fn keyword(text: &str) -> Option<TokenKind> {
        use TokenKind::*;
        Some(match text {
            "else" => Else,
            "false" => False,
            "for" => For,
            "loop" => Loop,
            "if" => If,
            "true" => True,
            "let" => Let,
            "object" => Object,
            "is" => Is,
            "as" => As,
            "does" => Does,
            "role" => Role,
            "import" => Import,
            "has" => Has,
            "confirm" => Confirm,
            "private" => Private,
            "enum" => Enum,
            "match" => Match,
            "case" => Case,
            "defer" => Defer,
            "try" => Try,
            "catch" => Catch,
            "next" => Next,
            "last" => Last,
            "async" => Async,
            "await" => Await,
            "spawn" => Spawn,
            "panic" => Panic,
            "throw" => Throw,
            "return" => Return,
            "self" => SelfKw,
            "module" => Module,
            "take" => Take,
            "native" => Native,
            "test" => Test,
            "fake" => Fake,
            "destroy" => Destroy,
            "isa" => Isa,
            "super" => Super,
            "none" => None,
            "null" => Null,
            "nil" => Nil,
            "abstract" => Abstract,
            "protected" => Protected,
            "with" => With,
            "finally" => Finally,
            _ => return Option::None,
        })
    }

    /// Removed or deferred syntax: the parser rejects these tokens wherever
    /// it meets them, with a dedicated explanation.
    pub fn is_reserved(self) -> bool {
        use TokenKind::*;
        matches!(
            self,
            Isa | Super
                | None
                | Null
                | Nil
                | Abstract
                | Protected
                | With
                | Finally
                | Question
                | QuestionDot
                | StarStar
        )
    }

    /// The upper-case kind name used in token dumps (Perl-compatible).
    pub fn name(self) -> &'static str {
        use TokenKind::*;
        match self {
            Int => "INT",
            Float => "FLOAT",
            String => "STRING",
            Char => "CHAR",
            Identifier => "IDENTIFIER",
            Plus => "PLUS",
            Minus => "MINUS",
            Slash => "SLASH",
            Star => "STAR",
            StarStar => "STAR_STAR",
            Percent => "PERCENT",
            Hash => "HASH",
            Amp => "AMP",
            AmpAmp => "AMP_AMP",
            Pipe => "PIPE",
            PipePipe => "PIPE_PIPE",
            Tilde => "TILDE",
            Arrow => "ARROW",
            ShiftLeft => "SHIFT_LEFT",
            ShiftRight => "SHIFT_RIGHT",
            ShiftUnsigned => "SHIFT_UNSIGNED",
            LeftParen => "LEFT_PAREN",
            RightParen => "RIGHT_PAREN",
            LeftBrace => "LEFT_BRACE",
            RightBrace => "RIGHT_BRACE",
            LeftBracket => "LEFT_BRACKET",
            RightBracket => "RIGHT_BRACKET",
            Semicolon => "SEMICOLON",
            Comma => "COMMA",
            Dot => "DOT",
            DotDot => "DOT_DOT",
            DotDotEqual => "DOT_DOT_EQUAL",
            Ellipsis => "ELLIPSIS",
            Colon => "COLON",
            DoubleColon => "DOUBLE_COLON",
            Question => "QUESTION",
            QuestionDot => "QUESTION_DOT",
            Caret => "CAP",
            At => "AT",
            Bang => "BANG",
            BangEqual => "BANG_EQUAL",
            Equal => "EQUAL",
            EqualEqual => "EQUAL_EQUAL",
            Less => "LESS",
            LessEqual => "LESS_EQUAL",
            Greater => "GREATER",
            GreaterEqual => "GREATER_EQUAL",
            StreamWrite => "STREAM_WRITE",
            Else => "ELSE",
            False => "FALSE",
            For => "FOR",
            Loop => "LOOP",
            If => "IF",
            True => "TRUE",
            Let => "LET",
            Object => "OBJECT",
            Is => "IS",
            As => "AS",
            Does => "DOES",
            Role => "ROLE",
            Import => "IMPORT",
            Has => "HAS",
            Confirm => "CONFIRM",
            Private => "PRIVATE",
            Enum => "ENUM",
            Match => "MATCH",
            Case => "CASE",
            Defer => "DEFER",
            Try => "TRY",
            Catch => "CATCH",
            Next => "NEXT",
            Last => "LAST",
            Async => "ASYNC",
            Await => "AWAIT",
            Spawn => "SPAWN",
            Panic => "PANIC",
            Throw => "THROW",
            Return => "RETURN",
            SelfKw => "SELF",
            Module => "MODULE",
            Take => "TAKE",
            Native => "NATIVE",
            Test => "TEST",
            Fake => "FAKE",
            Destroy => "DESTROY",
            Isa => "ISA",
            Super => "SUPER",
            None => "NONE",
            Null => "NULL",
            Nil => "NIL",
            Abstract => "ABSTRACT",
            Protected => "PROTECTED",
            With => "WITH",
            Finally => "FINALLY",
            Eof => "EOF",
        }
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The radix of an integer literal.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Radix {
    Binary,
    Decimal,
    Hexadecimal,
}

impl Radix {
    pub fn value(self) -> u32 {
        match self {
            Radix::Binary => 2,
            Radix::Decimal => 10,
            Radix::Hexadecimal => 16,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Radix::Binary => "binary",
            Radix::Decimal => "decimal",
            Radix::Hexadecimal => "hexadecimal",
        }
    }
}

/// An integer literal: its radix and its digits without prefix and without
/// `_` separators. The digits are kept as text; whether the value fits a
/// runtime `int` is decided later (the lexer only enforces the based-literal
/// limit that the grammar states explicitly).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntLiteral {
    pub radix: Radix,
    pub digits: String,
}

impl IntLiteral {
    /// The exact value, or `None` when it exceeds `u128`.
    pub fn to_u128(&self) -> Option<u128> {
        let radix = self.radix.value();
        self.digits.chars().try_fold(0u128, |acc, ch| {
            let digit = ch.to_digit(radix)?;
            acc.checked_mul(u128::from(radix))?
                .checked_add(u128::from(digit))
        })
    }
}

/// A float literal `digits.digits`, with `_` separators removed.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FloatLiteral {
    pub whole: String,
    pub fraction: String,
}

impl FloatLiteral {
    /// The normalised spelling `whole.fraction`.
    pub fn text(&self) -> String {
        format!("{}.{}", self.whole, self.fraction)
    }
}

/// How a string literal was written. All forms produce the same runtime
/// `string` value category; the form is kept because it is a real syntactic
/// difference (and it explains the decoded value).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum StringForm {
    /// `"..."` with escapes.
    Quoted,
    /// ```` ```...``` ```` raw text without escapes.
    Fenced,
    /// The bare word `endl`, an exact alias of `"\n"`.
    Endl,
}

impl StringForm {
    pub fn as_str(self) -> &'static str {
        match self {
            StringForm::Quoted => "quoted",
            StringForm::Fenced => "fenced",
            StringForm::Endl => "endl",
        }
    }
}

/// A decoded string literal.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StrLiteral {
    pub value: String,
    pub form: StringForm,
}

/// The decoded value of a literal token.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TokenValue {
    Int(IntLiteral),
    Float(FloatLiteral),
    Str(StrLiteral),
    Char(char),
}

/// One token: kind, byte span and (for literals) the decoded value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub value: Option<TokenValue>,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Token {
            kind,
            span,
            value: None,
        }
    }
}

/// `[A-Za-z_][A-Za-z0-9_]*` — the shape shared by identifiers and keywords.
pub fn is_word(text: &str) -> bool {
    let mut chars = text.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `[a-z_][A-Za-z0-9_]*` — lower-case names (fields, parameters, actions,
/// module segments).
pub fn is_lower_word(text: &str) -> bool {
    is_word(text) && text.starts_with(|c: char| c.is_ascii_lowercase() || c == '_')
}

/// `[A-Z][A-Za-z0-9_]*` — type names.
pub fn is_type_word(text: &str) -> bool {
    is_word(text) && text.starts_with(|c: char| c.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_are_exact_spellings() {
        assert_eq!(TokenKind::keyword("native"), Some(TokenKind::Native));
        assert_eq!(TokenKind::keyword("native_clock"), Option::None);
        assert_eq!(TokenKind::keyword("Native"), Option::None);
        assert_eq!(TokenKind::keyword("cap"), Option::None);
        assert_eq!(TokenKind::keyword("void"), Option::None);
        assert_eq!(TokenKind::keyword("destroy"), Some(TokenKind::Destroy));
    }

    #[test]
    fn reserved_set_is_exactly_the_removed_syntax() {
        assert!(TokenKind::Null.is_reserved());
        assert!(TokenKind::QuestionDot.is_reserved());
        assert!(TokenKind::StarStar.is_reserved());
        assert!(TokenKind::Finally.is_reserved());
        assert!(!TokenKind::Destroy.is_reserved());
        assert!(!TokenKind::Test.is_reserved());
        assert!(!TokenKind::Identifier.is_reserved());
    }

    #[test]
    fn word_shapes() {
        assert!(is_word("_x1"));
        assert!(!is_word("1x"));
        assert!(!is_word(""));
        assert!(is_lower_word("total_cents"));
        assert!(is_lower_word("_private"));
        assert!(!is_lower_word("Cash"));
        assert!(is_type_word("Cash"));
        assert!(!is_type_word("_Cash"));
    }

    #[test]
    fn integer_values_are_exact_and_overflow_aware() {
        let hex = IntLiteral {
            radix: Radix::Hexadecimal,
            digits: "7FFFFFFFFFFFFFFF".into(),
        };
        assert_eq!(hex.to_u128(), Some(i64::MAX as u128));
        let huge = IntLiteral {
            radix: Radix::Decimal,
            digits: "9".repeat(60),
        };
        assert_eq!(huge.to_u128(), Option::None);
    }
}
