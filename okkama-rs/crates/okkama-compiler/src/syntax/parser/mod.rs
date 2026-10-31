//! The Okkama parser.
//!
//! A hand-written recursive-descent parser. Declarations and statements are
//! parsed by one function per grammar production; expressions use the same
//! precedence ladder as the grammar (`expr` → `logic_or` → ... → `unary` →
//! `postfix` → `primary`), so every Rust function can be read side by side
//! with `docs/okkama-grammar-1.0.md` and the reference parser
//! `okkama-pl/lib/Okkama/Parser.pm`.
//!
//! Design rules:
//!
//! * The parser stops at the first syntax error and returns one structured
//!   [`Diagnostic`]. Error messages deliberately match the reference parser
//!   word for word so the two implementations can be compared mechanically.
//! * The whole input must be consumed: the top-level loop runs until `EOF`,
//!   so a valid prefix followed by garbage is an error.
//! * Recursion is bounded by [`ParseOptions::max_nesting`]; extremely deep
//!   input produces a diagnostic instead of a stack overflow.
//! * Some context-sensitive restrictions that the reference parser enforces
//!   (only `main` at module scope, variadic placement, duplicate generic
//!   parameters, ...) are enforced here too so that accept/reject decisions
//!   agree. Everything that needs name resolution or types is left to the
//!   future semantic analysis.

mod decl;
mod expr;
mod stmt;
mod types;

use crate::diagnostics::{Diagnostic, Found};
use crate::source::Span;
use crate::syntax::ast::{ImportDecl, ModuleDecl, Name, Program, StringLit, Version};
use crate::syntax::token::{Token, TokenKind, TokenValue, is_lower_word, is_type_word, is_word};
use crate::syntax::{ParseMode, ParseOptions};

pub(crate) type PResult<T> = Result<T, Diagnostic>;

/// Parses a token stream produced by [`lex`](crate::syntax::lexer::lex)
/// from the same `text`. The stream must end with an `EOF` token.
pub fn parse_tokens(text: &str, tokens: Vec<Token>, options: ParseOptions) -> PResult<Program> {
    Parser::new(text, tokens, options).parse_program()
}

pub(crate) struct Parser<'s> {
    text: &'s str,
    tokens: Vec<Token>,
    pos: usize,
    mode: ParseMode,
    depth: usize,
    max_depth: usize,
    /// While parsing the body of an external `cap Owner::Name { }`, the owner
    /// name: `Owner::field` then means a private field read.
    external_cap_owner: Option<String>,
    /// While parsing the top level of a `for` source: an arrow there belongs
    /// to the `for`, not to a functional call. Every nested expression
    /// (parentheses, arguments, indices, blocks) clears it.
    for_source: bool,
}

impl<'s> Parser<'s> {
    fn new(text: &'s str, mut tokens: Vec<Token>, options: ParseOptions) -> Self {
        if tokens.last().map(|token| token.kind) != Some(TokenKind::Eof) {
            let end = u32::try_from(text.len()).unwrap_or(u32::MAX);
            tokens.push(Token::new(TokenKind::Eof, Span::empty(end)));
        }
        Parser {
            text,
            tokens,
            pos: 0,
            mode: options.mode,
            depth: 0,
            max_depth: options.max_nesting,
            external_cap_owner: None,
            for_source: false,
        }
    }

    // ===== program shape ===================================================

    /// `program = module_decl import_decl* declaration* EOF`
    fn parse_program(mut self) -> PResult<Program> {
        let module = self.module_declaration()?;
        let mut imports = Vec::new();
        while self.eat(TokenKind::Import) {
            imports.push(self.import_declaration()?);
        }
        let mut items = Vec::new();
        while !self.is_at_end() {
            items.push(self.module_item()?);
        }
        let end = self.peek().span.end;
        Ok(Program {
            span: Span::new(module.span.start, end),
            module,
            imports,
            items,
        })
    }

    /// `module_decl = "module" module_name version? ";"`
    fn module_declaration(&mut self) -> PResult<ModuleDecl> {
        let keyword = self.consume(
            TokenKind::Module,
            "Program must start with module declaration.",
        )?;
        let mut path = vec![self.consume_module_name_part("Expect module name.")?];
        while self.eat(TokenKind::Slash) {
            path.push(self.consume_module_name_part("Expect module name part after \"/\".")?);
        }
        let version = if self.check(TokenKind::Semicolon) {
            None
        } else {
            Some(self.version()?)
        };
        let semicolon = self.consume(
            TokenKind::Semicolon,
            "Expect \";\" after module declaration.",
        )?;
        Ok(ModuleDecl {
            path,
            version,
            span: keyword.span.to(semicolon.span),
        })
    }

    /// `version = "v" INT "." INT "." INT`.
    ///
    /// There is no version token: `v1.2.3` lexes as the identifier `v1`, a
    /// dot and the float `2.3`, while `v1.2 .3` lexes as `v1 . 2 . 3`. Both
    /// spellings are accepted, exactly like the reference parser does.
    fn version(&mut self) -> PResult<Version> {
        let major_token = self.consume_identifier("Expect module version.")?;
        let major = match major_token.text.strip_prefix('v') {
            Some(digits) if is_digits(digits) => digits.to_string(),
            _ => {
                return Err(
                    self.error_at_span(major_token.span, "Expect module version like v1.0.0.")
                );
            }
        };
        self.consume(TokenKind::Dot, "Expect \".\" in module version.")?;
        let minor_token = self.consume_number("Expect minor version number.")?;
        let minor_text = self.lexeme(&minor_token).to_string();
        if let Some((minor, patch)) = minor_text.split_once('.') {
            if is_digits(minor) && is_digits(patch) {
                return Ok(Version {
                    major,
                    minor: minor.to_string(),
                    patch: patch.to_string(),
                    span: major_token.span.to(minor_token.span),
                });
            }
        }
        self.consume(TokenKind::Dot, "Expect \".\" in module version.")?;
        let patch_token = self.consume_number("Expect patch version number.")?;
        Ok(Version {
            major,
            minor: minor_text,
            patch: self.lexeme(&patch_token).to_string(),
            span: major_token.span.to(patch_token.span),
        })
    }

    /// `import_decl = "import" STRING ("as" IDENT)? ("take" name ("," name)*)? ";"`
    fn import_declaration(&mut self) -> PResult<ImportDecl> {
        let keyword = self.previous().span;
        let path_token = self.consume(TokenKind::String, "Expect import path after \"import\".")?;
        let path = self.string_lit(&path_token);
        let alias = if self.eat(TokenKind::As) {
            Some(self.consume_identifier("Expect import alias after \"as\".")?)
        } else {
            None
        };
        let mut take = Vec::new();
        if self.eat(TokenKind::Take) {
            take.push(self.consume_name("Expect imported name after \"take\".")?);
            while self.eat(TokenKind::Comma) {
                take.push(self.consume_name("Expect imported name after \",\".")?);
            }
        }
        let semicolon = self.consume(
            TokenKind::Semicolon,
            "Expect \";\" after import declaration.",
        )?;
        Ok(ImportDecl {
            path,
            alias,
            take,
            span: keyword.to(semicolon.span),
        })
    }

    // ===== token cursor ======================================================

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn peek_kind(&self) -> TokenKind {
        self.peek().kind
    }

    /// The token `offset` positions ahead, clamped to the final `EOF`.
    fn nth(&self, offset: usize) -> &Token {
        let index = (self.pos + offset).min(self.tokens.len() - 1);
        &self.tokens[index]
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.pos.saturating_sub(1)]
    }

    fn is_at_end(&self) -> bool {
        self.peek_kind() == TokenKind::Eof
    }

    fn check(&self, kind: TokenKind) -> bool {
        !self.is_at_end() && self.peek_kind() == kind
    }

    fn check_next(&self, kind: TokenKind) -> bool {
        self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1].kind == kind
    }

    fn advance(&mut self) -> Token {
        if !self.is_at_end() {
            self.pos += 1;
        }
        self.previous().clone()
    }

    fn eat(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn lexeme(&self, token: &Token) -> &'s str {
        self.text.get(token.span.range()).unwrap_or("")
    }

    /// Consumes a token of `kind` or fails with `message`.
    ///
    /// When a type parser asks for the generic closer `>`, a compound token
    /// that starts with `>` (`>>`, `>>>`, `>=`) is split in place: the first
    /// `>` closes one generic level and the rest stays in the stream. This is
    /// how `Box<Box<int>>` and `let x is Box<int>= ...` parse while
    /// expressions keep `>>` as one shift operator.
    fn consume(&mut self, kind: TokenKind, message: &str) -> PResult<Token> {
        if kind == TokenKind::Greater {
            self.split_greater();
        }
        if self.check(kind) {
            return Ok(self.advance());
        }
        let token = self.peek().clone();
        self.reject_reserved(&token)?;
        Err(self.error_at(&token, message))
    }

    fn split_greater(&mut self) {
        let token = self.peek().clone();
        let rest_kind = match token.kind {
            TokenKind::ShiftRight => TokenKind::Greater,
            TokenKind::ShiftUnsigned => TokenKind::ShiftRight,
            TokenKind::GreaterEqual => TokenKind::Equal,
            _ => return,
        };
        let middle = token.span.start + 1;
        self.tokens[self.pos] = Token::new(TokenKind::Greater, Span::new(token.span.start, middle));
        self.tokens.insert(
            self.pos + 1,
            Token::new(rest_kind, Span::new(middle, token.span.end)),
        );
    }

    fn consume_number(&mut self, message: &str) -> PResult<Token> {
        if self.check(TokenKind::Int) || self.check(TokenKind::Float) {
            return Ok(self.advance());
        }
        let token = self.peek().clone();
        self.reject_reserved(&token)?;
        Err(self.error_at(&token, message))
    }

    // ===== errors ==========================================================

    fn error_at(&self, token: &Token, message: impl Into<String>) -> Diagnostic {
        let found = if token.kind == TokenKind::Eof {
            Found::EndOfInput
        } else {
            Found::Token(self.lexeme(token).to_string())
        };
        Diagnostic::syntax(message, token.span, found)
    }

    fn error_here(&self, message: impl Into<String>) -> Diagnostic {
        self.error_at(self.peek(), message)
    }

    /// A syntax error located at an already consumed token or name.
    fn error_at_span(&self, span: Span, message: &str) -> Diagnostic {
        let lexeme = self.text.get(span.range()).unwrap_or("").to_string();
        Diagnostic::syntax(message, span, Found::Token(lexeme))
    }

    /// Removed and deferred syntax gets a dedicated explanation wherever the
    /// parser meets it, instead of a generic "unexpected token".
    fn reject_reserved(&self, token: &Token) -> PResult<()> {
        if token.kind == TokenKind::Eof || !token.kind.is_reserved() {
            return Ok(());
        }
        let message = match token.kind {
            TokenKind::Question => "Optional types are not part of Okkama v1.0 syntax.".to_string(),
            TokenKind::QuestionDot => {
                "Optional chaining is not part of Okkama v1.0 syntax.".to_string()
            }
            TokenKind::StarStar => {
                "Exponent operator is not part of Okkama v1.0 syntax.".to_string()
            }
            TokenKind::Isa | TokenKind::Super => {
                "Inheritance is not part of Okkama v1.0 syntax.".to_string()
            }
            TokenKind::None | TokenKind::Null | TokenKind::Nil => {
                "Null/none/nil values are not part of Okkama v1.0 syntax.".to_string()
            }
            _ => format!(
                "\"{}\" is reserved and not part of Okkama v1.0 syntax.",
                self.lexeme(token)
            ),
        };
        Err(self.error_at(token, message))
    }

    /// Forbids `with` and `with_*` in action-name positions (old copy-setter
    /// APIs); `withdrawal` stays legal.
    fn reject_wither(&self, token: &Token) -> PResult<()> {
        let lexeme = self.lexeme(token);
        if lexeme == "with" || lexeme.starts_with("with_") {
            return Err(self.error_at(
                token,
                "Action names \"with\" and \"with_*\" are forbidden; use domain composition or an explicitly named result.",
            ));
        }
        Ok(())
    }

    // ===== names =============================================================

    /// Whether a token can stand in a contextual name position: any
    /// word-shaped token that is not removed syntax. This deliberately
    /// includes active keywords, so member names such as `test.ok()`,
    /// `value.last()` or the action label `is(...)` work. Declarations that
    /// need a real identifier use the stricter `consume_*` helpers below.
    fn is_name_token(&self, token: &Token) -> bool {
        token.kind != TokenKind::Eof && !token.kind.is_reserved() && is_word(self.lexeme(token))
    }

    fn check_name(&self) -> bool {
        !self.is_at_end() && self.is_name_token(self.peek())
    }

    fn check_type_name(&self) -> bool {
        self.check_name()
            && self
                .lexeme(self.peek())
                .starts_with(|c: char| c.is_ascii_uppercase())
    }

    fn name_of(&self, token: &Token) -> Name {
        Name {
            text: self.lexeme(token).to_string(),
            span: token.span,
        }
    }

    /// Any name token (keyword-shaped words included).
    fn consume_name(&mut self, message: &str) -> PResult<Name> {
        if self.check_name() {
            let token = self.advance();
            return Ok(self.name_of(&token));
        }
        let token = self.peek().clone();
        self.reject_reserved(&token)?;
        Err(self.error_at(&token, message))
    }

    /// A real identifier of any case (`let`, `for`, `catch` and cap fields).
    fn consume_binding_name(&mut self, message: &str) -> PResult<Name> {
        if !self.check(TokenKind::Identifier) {
            let token = self.peek().clone();
            self.reject_reserved(&token)?;
            return Err(self.error_at(&token, message));
        }
        self.consume_name(message)
    }

    /// A real lower-case identifier (fields, parameters, aliases, ...).
    fn consume_identifier(&mut self, message: &str) -> PResult<Name> {
        let name = self.consume_binding_name(message)?;
        if !is_lower_word(&name.text) {
            return Err(self.error_at(&self.previous().clone(), message));
        }
        Ok(name)
    }

    /// A real identifier starting with an upper-case letter.
    fn consume_type_name(&mut self, message: &str) -> PResult<Name> {
        let name = self.consume_binding_name(message)?;
        if !is_type_word(&name.text) {
            return Err(self.error_at(&self.previous().clone(), message));
        }
        Ok(name)
    }

    /// Module path segments are namespace data: keyword-shaped lower-case
    /// words (`module test/more`) are allowed.
    fn consume_module_name_part(&mut self, message: &str) -> PResult<Name> {
        let name = self.consume_name(message)?;
        if !is_lower_word(&name.text) {
            return Err(self.error_at(&self.previous().clone(), message));
        }
        Ok(name)
    }

    /// Action labels share the contextual member namespace, so `is(...)` or
    /// `has(...)` can be declared, but they must be lower-case and must not
    /// be `with`/`with_*`.
    fn consume_action_name(&mut self, message: &str) -> PResult<Name> {
        let token = self.peek().clone();
        self.reject_wither(&token)?;
        let name = self.consume_name(message)?;
        if !is_lower_word(&name.text) {
            return Err(self.error_at(&self.previous().clone(), message));
        }
        Ok(name)
    }

    /// A member name after `.`.
    fn consume_member_name(&mut self, message: &str) -> PResult<Name> {
        let token = self.peek().clone();
        self.reject_wither(&token)?;
        self.consume_name(message)
    }

    /// `async` modifies a declaration only when it is followed by a name and
    /// `(`; elsewhere it is left for a precise error message.
    fn check_async_modifier(&self) -> bool {
        self.check(TokenKind::Async)
            && self.pos + 2 < self.tokens.len()
            && self.is_name_token(&self.tokens[self.pos + 1])
            && self.tokens[self.pos + 2].kind == TokenKind::LeftParen
    }

    fn string_lit(&self, token: &Token) -> StringLit {
        let literal = match &token.value {
            Some(TokenValue::Str(literal)) => literal.clone(),
            // Every STRING token carries a decoded value; stay total anyway.
            _ => crate::syntax::token::StrLiteral {
                value: self.lexeme(token).to_string(),
                form: crate::syntax::token::StringForm::Quoted,
            },
        };
        StringLit {
            literal,
            span: token.span,
        }
    }

    // ===== nesting guard =====================================================

    /// Runs `f` one nesting level deeper. Every recursive production
    /// (expressions, prefix operators, types, blocks, `else if` chains) goes
    /// through here, which bounds the native stack depth.
    fn nested<T>(&mut self, f: impl FnOnce(&mut Self) -> PResult<T>) -> PResult<T> {
        if self.depth >= self.max_depth {
            return Err(self.error_here(format!(
                "Nesting is too deep (the limit is {} levels).",
                self.max_depth
            )));
        }
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        result
    }
}

fn is_digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
}
