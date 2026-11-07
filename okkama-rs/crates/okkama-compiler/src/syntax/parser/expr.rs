//! Expressions.
//!
//! Precedence, lowest first. All binary levels are left-associative except
//! ranges, which do not chain. The binary levels are parsed by precedence
//! climbing over the table [`LEVELS`]; the other levels are
//! one function each, mirroring `docs/okkama-grammar-1.0.md`.
//!
//! | level          | operators                       |
//! |----------------|---------------------------------|
//! | choice/fallback | `?? … !!`, `else`              |
//! | logic_or       | `\|\|`                          |
//! | logic_and      | `&&`                            |
//! | stream_write   | `<-`                            |
//! | equality       | `==` `!=`                       |
//! | range          | `..` `..=` (at most one)        |
//! | comparison     | `<` `<=` `>` `>=`               |
//! | bitwise_or     | `\|`                            |
//! | bitwise_xor    | `^`                             |
//! | bitwise_and    | `&`                             |
//! | shift          | `<<` `>>` `>>>`                 |
//! | term           | `+` `-`                         |
//! | factor         | `*` `/` `%`                     |
//! | unary          | `!` `-` `~` `await` `spawn` `panic` |
//! | postfix        | functional `->`, call, `.name`, `::name`, `[i] else f`, `...` |

use super::{PResult, Parser};
use crate::source::Span;
use crate::syntax::ast::{
    ActionResult, Argument, BinaryOp, Block, CatchClause, Expr, ExprKind, FunctionalOp, Literal, MapEntry,
    Name, ResultSpec, Stmt, StmtKind, TryBody, UnaryOp,
};
use crate::syntax::token::{StrLiteral, StringForm, TokenKind, TokenValue, is_lower_word, is_type_word};

impl Parser<'_> {
    /// `expr = logic_or optional_else? compact_choice?`.
    ///
    /// The functional suffix `-> bindings { }` is a postfix form of a member
    /// call (`receiver.map(args)`, see [`Self::finish_postfix`]), so it binds
    /// tighter than every operator: `total + xs.reduce(0) -> a, x { a + x; }`.
    /// A nested expression is never the top level of a `for` source.
    pub(super) fn expression(&mut self) -> PResult<Expr> {
        self.expression_with_fallback(true)
    }

    pub(super) fn expression_with_fallback(&mut self, optional: bool) -> PResult<Expr> {
        let enclosing = std::mem::replace(&mut self.for_source, false);
        let result = self.nested(|parser| {
            let mut expr = parser.logic_or()?;
            if optional
                && parser.check(TokenKind::Else)
                && parser.nth(1).kind != TokenKind::LeftBrace
            {
                parser.advance();
                let fallback = if parser.eat(TokenKind::Throw) {
                    let start = parser.previous().span;
                    let value = parser.expression()?;
                    Expr {
                        span: start.to(value.span),
                        kind: ExprKind::Throw(Box::new(value)),
                    }
                } else {
                    parser.expression()?
                };
                expr = Expr {
                    span: expr.span.to(fallback.span),
                    kind: ExprKind::Coalesce {
                        value: Box::new(expr),
                        fallback: Box::new(fallback),
                    },
                };
            }
            if parser.eat(TokenKind::Choice) {
                expr = parser.finish_choice(expr)?;
            }
            Ok(expr)
        });
        self.for_source = enclosing;
        result
    }

    /// The arrow after `receiver.name(args)` starts a functional block,
    /// except at the top level of a `for` source, where it is the loop's.
    fn functional_follows(&self, call: &Expr) -> PResult<bool> {
        if self.for_source || !self.check(TokenKind::Arrow) {
            return Ok(false);
        }
        let ExprKind::Call { callee, .. } = &call.kind else {
            return Ok(false);
        };
        let ExprKind::Get { name, .. } = &callee.kind else {
            return Ok(false);
        };
        if FunctionalOp::from_name(&name.text).is_none() {
            let message = format!("Unsupported functional operation \"{}\".", name.text);
            return Err(self.error_at_name(name, &message));
        }
        Ok(true)
    }

    fn choice_separator(&self) -> bool {
        self.check(TokenKind::Bang)
            && self.nth(1).kind == TokenKind::Bang
            && self.peek().span.end == self.nth(1).span.start
    }

    fn choice_value(&mut self) -> PResult<Expr> {
        if self.eat(TokenKind::Throw) {
            let start = self.previous().span;
            let value = self.expression()?;
            Ok(Expr {
                span: start.to(value.span),
                kind: ExprKind::Throw(Box::new(value)),
            })
        } else {
            self.expression()
        }
    }

    fn choice_block(value: Expr) -> crate::syntax::ast::Block {
        let span = value.span;
        crate::syntax::ast::Block {
            stmts: vec![crate::syntax::ast::Stmt {
                kind: crate::syntax::ast::StmtKind::Expr {
                    expr: value,
                    condition: None,
                },
                span,
            }],
            span,
        }
    }

    fn finish_choice(&mut self, subject: Expr) -> PResult<Expr> {
        use crate::syntax::ast::{MatchBody, MatchCase};
        let start = subject.span;
        let patterned = self.check(TokenKind::Dot)
            || (self.lexeme(self.peek()) == "_" && self.nth(1).kind == TokenKind::Colon);
        let kind = if patterned {
            let mut cases = Vec::new();
            let mut else_block = None;
            loop {
                let arm_start = self.peek().span;
                let pattern = if self.lexeme(self.peek()) == "_" {
                    self.advance();
                    None
                } else {
                    self.consume(TokenKind::Dot, "Expect .variant or _ in compact match.")?;
                    let name = self.consume_member_name("Expect enum variant after dot.")?;
                    let owner = Expr {
                        kind: ExprKind::Name(Name {
                            text: "$choice".into(),
                            span: arm_start,
                        }),
                        span: arm_start,
                    };
                    let mut pattern = Expr {
                        span: arm_start.to(name.span),
                        kind: ExprKind::Get {
                            object: Box::new(owner),
                            name,
                        },
                    };
                    if self.eat(TokenKind::LeftParen) {
                        pattern = self.finish_call(pattern)?;
                    }
                    Some(pattern)
                };
                self.consume(TokenKind::Colon, "Expect : after compact match pattern.")?;
                let body = Self::choice_block(self.choice_value()?);
                if let Some(pattern) = pattern {
                    cases.push(MatchCase {
                        span: arm_start.to(body.span),
                        patterns: vec![pattern],
                        body,
                    });
                } else {
                    else_block = Some(body);
                    if self.choice_separator() {
                        return Err(
                            self.error_here("Wildcard must be the last compact match branch.")
                        );
                    }
                    break;
                }
                if !self.choice_separator() {
                    break;
                }
                self.advance();
                self.advance();
            }
            ExprKind::Match(Box::new(MatchBody {
                subject,
                cases,
                else_block,
                span: start.to(self.previous().span),
            }))
        } else {
            let then_block = Self::choice_block(self.choice_value()?);
            if !self.choice_separator() {
                return Err(self.error_here("Boolean choice requires !! and a false branch."));
            }
            self.advance();
            self.advance();
            let else_block = Self::choice_block(self.choice_value()?);
            if self.choice_separator() {
                return Err(self.error_here("Boolean choice has exactly two branches."));
            }
            ExprKind::If {
                condition: Box::new(subject),
                then_block,
                else_block,
            }
        };
        let span = start.to(self.previous().span);
        // Include the subject and every branch, even choices hidden in parentheses,
        // calls or blocks. Sibling expressions outside this span remain independent.
        let first = self.tokens.partition_point(|t| t.span.start < span.start);
        if self.tokens[first..self.pos]
            .iter()
            .filter(|t| t.kind == TokenKind::Choice)
            .take(2)
            .count()
            > 1
        {
            return Err(self.error_at_span(
                span,
                "Nested ?? / !! choices are not allowed; use a local variable or match.",
            ));
        }
        Ok(Expr {
            kind: ExprKind::Choice(Box::new(Expr { kind, span })),
            span,
        })
    }

    fn error_at_name(&self, name: &Name, message: &str) -> crate::diagnostics::Diagnostic {
        crate::diagnostics::Diagnostic::syntax(
            message,
            name.span,
            crate::diagnostics::Found::Token(name.text.clone()),
        )
    }

    /// Turns `receiver.op(args)` followed by `-> a, b { body }` into a
    /// functional expression and validates the operation's arguments.
    fn finish_functional(&mut self, call: Expr) -> PResult<Expr> {
        self.consume(
            TokenKind::Arrow,
            "Expect \"->\" before functional bindings.",
        )?;
        let start = call.span;
        let ExprKind::Call { callee, args } = call.into_kind() else {
            unreachable!("finish_functional is only called for member calls");
        };
        let ExprKind::Get {
            object: receiver,
            name: operation_name,
        } = callee.into_kind()
        else {
            unreachable!("finish_functional is only called for member calls");
        };
        let operation = FunctionalOp::from_name(&operation_name.text)
            .expect("the caller checked the operation name");
        if let Some(named) = args.iter().find_map(|arg| arg.name.as_ref()) {
            let message = format!(
                "Functional operation \"{}\" does not accept named arguments.",
                operation_name.text
            );
            return Err(self.error_at_name(named, &message));
        }
        match operation {
            FunctionalOp::Map | FunctionalOp::Filter if !args.is_empty() => {
                let message = format!(
                    "Functional operation \"{}\" accepts no arguments.",
                    operation_name.text
                );
                return Err(self.error_at_name(&operation_name, &message));
            }
            FunctionalOp::Reduce if args.len() != 1 => {
                return Err(self.error_at_name(
                    &operation_name,
                    "Functional operation \"reduce\" requires exactly one initial value.",
                ));
            }
            _ => {}
        }
        let mut bindings: Vec<Name> = Vec::new();
        loop {
            let binding =
                self.consume_identifier("Expect functional binding name after \"->\".")?;
            if bindings.iter().any(|seen| seen.text == binding.text) {
                let message = format!(
                    "Functional binding \"{}\" is declared more than once.",
                    binding.text
                );
                return Err(self.error_at_name(&binding, &message));
            }
            bindings.push(binding);
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        let body = self.block()?;
        Ok(Expr {
            span: start.to(body.span),
            kind: ExprKind::Functional {
                operation,
                operation_name,
                receiver,
                args,
                bindings,
                body,
            },
        })
    }

    // ===== binary operators ==================================================

    /// `logic_or` in the grammar: the whole binary-operator part of an
    /// expression, i.e. everything below the functional suffix.
    ///
    /// The grammar spells the binary operators as a ladder of ten levels
    /// (`logic_or` ... `factor`). Parsing them with one function per level
    /// would push about thirty stack frames for every parenthesised
    /// sub-expression, so they are parsed here by precedence climbing over
    /// the same table ([`LEVELS`]): identical trees, a few frames per
    /// nesting level.
    pub(super) fn logic_or(&mut self) -> PResult<Expr> {
        self.binary(0)
    }

    /// Parses operators of level `min` (index into [`LEVELS`]) and tighter.
    fn binary(&mut self, min: usize) -> PResult<Expr> {
        let mut left = self.unary()?;
        while let Some((level, operator)) = binary_operator(self.peek_kind()) {
            if level < min {
                break;
            }
            self.advance();
            let op_span = self.previous().span;
            // Left associativity: the right operand only takes operators
            // that bind strictly tighter.
            let right = self.binary(level + 1)?;
            let span = left.span.to(right.span);
            left = Expr {
                span,
                kind: match operator {
                    Operator::Binary(op) => ExprKind::Binary {
                        op,
                        op_span,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    Operator::StreamWrite => ExprKind::StreamWrite {
                        op_span,
                        sink: Box::new(left),
                        source: Box::new(right),
                    },
                    Operator::Range { inclusive } => ExprKind::Range {
                        inclusive,
                        op_span,
                        start: Box::new(left),
                        end: Box::new(right),
                    },
                },
            };
            if LEVELS[level].chaining == Chaining::Single
                && binary_operator(self.peek_kind()).is_some_and(|(next, _)| next == level)
            {
                return Err(self.error_here("Range expressions cannot be chained."));
            }
        }
        Ok(left)
    }

    // ===== prefix forms ======================================================

    /// Prefix forms. Each prefix operator recurses through [`Self::operand`],
    /// which counts against the nesting limit (`- - - x` is three levels).
    fn unary(&mut self) -> PResult<Expr> {
        let start = self.peek().span;
        let op = match self.peek_kind() {
            TokenKind::Bang => Some(UnaryOp::Not),
            TokenKind::Minus => Some(UnaryOp::Negate),
            TokenKind::Tilde => Some(UnaryOp::BitNot),
            _ => None,
        };
        if let Some(op) = op {
            self.advance();
            let operand = self.operand()?;
            return Ok(Expr {
                span: start.to(operand.span),
                kind: ExprKind::Unary {
                    op,
                    op_span: start,
                    operand: Box::new(operand),
                },
            });
        }
        if self.check(TokenKind::Amp) {
            return Err(self.error_here(
                "Constructor projections have been removed; use Type(args).method().",
            ));
        }
        if self.eat(TokenKind::Await) {
            let value = self.operand()?;
            return Ok(Expr {
                span: start.to(value.span),
                kind: ExprKind::Await {
                    value: Box::new(value),
                },
            });
        }
        // `spawn(` is an ordinary call of an action labelled `spawn` (for
        // example the native `runtime/process` action); `spawn expr` is the
        // task operator and requires a call operand.
        if self.check(TokenKind::Spawn) && self.check_next(TokenKind::LeftParen) {
            return self.postfix();
        }
        if self.eat(TokenKind::Spawn) {
            let keyword = self.previous().clone();
            let call = self.operand()?;
            if !matches!(
                call.kind,
                ExprKind::Call { .. } | ExprKind::ArrayCast { .. }
            ) {
                return Err(self.error_at(&keyword, "spawn requires an action call"));
            }
            return Ok(Expr {
                span: start.to(call.span),
                kind: ExprKind::Spawn {
                    call: Box::new(call),
                },
            });
        }
        if self.eat(TokenKind::Panic) {
            let value = self.panic_value()?;
            return Ok(Expr {
                span: start.to(value.span),
                kind: ExprKind::Panic {
                    value: Box::new(value),
                },
            });
        }
        if self.starts_named_projection() {
            return Err(self.error_here(
                "Constructor projections have been removed; use Type(args).method().",
            ));
        }
        self.postfix()
    }

    /// The operand of a prefix operator: one more `unary`, one level deeper.
    fn operand(&mut self) -> PResult<Expr> {
        self.nested(Self::unary)
    }

    /// The operand of `panic`, which is a full expression. The bare
    /// `panic;` is a removed rethrow form.
    pub(super) fn panic_value(&mut self) -> PResult<Expr> {
        if self.check(TokenKind::Semicolon) {
            let keyword = self.previous().clone();
            return Err(self.error_at(
                &keyword,
                "\"panic;\" is not supported; use \"throw;\" to rethrow.",
            ));
        }
        self.expression()
    }

    /// `prefix Type(` or `prefix module.Type(` where `prefix` is a
    /// lower-case identifier: a named constructor projection.
    fn starts_named_projection(&self) -> bool {
        if !self.check(TokenKind::Identifier) || !is_lower_word(self.lexeme(self.peek())) {
            return false;
        }
        if self.pos + 2 >= self.tokens.len() {
            return false;
        }
        let target = &self.tokens[self.pos + 1];
        if self.is_name_token(target)
            && is_type_word(self.lexeme(target))
            && self.tokens[self.pos + 2].kind == TokenKind::LeftParen
        {
            return true;
        }
        if self.pos + 4 >= self.tokens.len() {
            return false;
        }
        let object = &self.tokens[self.pos + 3];
        self.is_name_token(target)
            && is_lower_word(self.lexeme(target))
            && self.tokens[self.pos + 2].kind == TokenKind::Dot
            && self.is_name_token(object)
            && is_type_word(self.lexeme(object))
            && self.tokens[self.pos + 4].kind == TokenKind::LeftParen
    }

    // ===== postfix ===========================================================

    fn postfix(&mut self) -> PResult<Expr> {
        let primary = self.primary()?;
        self.finish_postfix(primary)
    }

    /// Calls, member access, cap selection, indexing and unpack chain left to
    /// right from one primary expression.
    fn finish_postfix(&mut self, mut expr: Expr) -> PResult<Expr> {
        loop {
            if self.eat(TokenKind::DoubleColon) {
                let name = self.consume_name("Expect selection name after \"::\".")?;
                let span = expr.span.to(name.span);
                // Inside `cap Owner::Name { }`, `Owner::field` reads a private
                // field of the owner rather than selecting a cap.
                let owner_field = match (&expr.kind, &self.external_cap_owner) {
                    (ExprKind::Name(owner), Some(cap_owner)) if owner.text == *cap_owner => {
                        Some(owner.clone())
                    }
                    _ => None,
                };
                expr = Expr {
                    span,
                    kind: match owner_field {
                        Some(owner) => ExprKind::OwnerField { owner, name },
                        None => ExprKind::Select {
                            object: Box::new(expr),
                            name,
                        },
                    },
                };
                continue;
            }
            if self.eat(TokenKind::LeftParen) {
                expr = self.finish_call(expr)?;
                if self.functional_follows(&expr)? {
                    expr = self.finish_functional(expr)?;
                }
                continue;
            }
            if self.check(TokenKind::Colon)
                && self.check_next(TokenKind::LeftParen)
                && !self.starts_anonymous_action(self.pos + 1)
            {
                return Err(self.error_here(
                    "Constructor projections have been removed; use Type(args).method().",
                ));
            }
            if self.eat(TokenKind::Dot) {
                let name = self.consume_member_name("Expect property name after \".\".")?;
                if name.text == "throws"
                    && matches!(&expr.kind, ExprKind::Name(test) if test.text == "test")
                    && self.check(TokenKind::LeftParen)
                {
                    expr = self.test_throws(expr.span)?;
                    continue;
                }
                expr = Expr {
                    span: expr.span.to(name.span),
                    kind: ExprKind::Get {
                        object: Box::new(expr),
                        name,
                    },
                };
                continue;
            }
            if self.eat(TokenKind::LeftBracket) {
                let index = self.expression()?;
                let close = self.consume(
                    TokenKind::RightBracket,
                    "Expect \"]\" after index expression.",
                )?;
                let fallback = if self.eat(TokenKind::Else) {
                    Some(Box::new(self.expression()?))
                } else {
                    None
                };
                let end = fallback.as_ref().map_or(close.span, |value| value.span);
                expr = Expr {
                    span: expr.span.to(end),
                    kind: ExprKind::Index {
                        object: Box::new(expr),
                        index: Box::new(index),
                        fallback,
                    },
                };
                continue;
            }
            if self.eat(TokenKind::Ellipsis) {
                let ellipsis = self.previous().span;
                expr = Expr {
                    span: expr.span.to(ellipsis),
                    kind: ExprKind::Unpack {
                        operand: Box::new(expr),
                        ellipsis,
                    },
                };
                continue;
            }
            return Ok(expr);
        }
    }

    /// `test.throws(Type) { … }` (#216) after `test.throws`: the block must
    /// throw a `Type`, whose value is the expression's. It is written as the
    /// `try` it means — the block, then a panic that fails the test, under a
    /// `catch` of `Type` that yields the error — so checking and every engine
    /// treat it as one. The binding's name cannot be written in source.
    fn test_throws(&mut self, start: Span) -> PResult<Expr> {
        self.consume(TokenKind::LeftParen, "Expect \"(\" after test.throws.")?;
        let ty = self.type_expr()?;
        self.consume(TokenKind::RightParen, "Expect \")\" after the error type of test.throws.")?;
        if !self.check(TokenKind::LeftBrace) {
            return Err(self.error_here("Expect a block after test.throws(Type): test.throws(Type) { … }"));
        }
        let mut body = self.block()?;
        let span = start.to(body.span);
        let expected = format!("test.throws: the block did not throw {}", self.text_of(ty.span));
        let literal = |value: String| Expr {
            kind: ExprKind::Literal(Literal::String(StrLiteral { value, form: StringForm::Quoted })),
            span,
        };
        let panic = Expr { kind: ExprKind::Panic { value: Box::new(literal(expected)) }, span };
        body.stmts.push(Stmt { kind: StmtKind::Expr { expr: panic, condition: None }, span: body.span });
        let caught = Name { text: "test.throws#error".into(), span: ty.span };
        let clause = CatchClause {
            name: Some(caught.clone()),
            types: vec![ty],
            patterns: Vec::new(),
            body: Block {
                stmts: vec![Stmt {
                    kind: StmtKind::Expr { expr: Expr { kind: ExprKind::Name(caught), span }, condition: None },
                    span,
                }],
                span,
            },
            span,
        };
        Ok(Expr { kind: ExprKind::Try(Box::new(TryBody { body, catches: vec![clause], span })), span })
    }

    /// Arguments after `(` up to and including `)`.
    fn finish_call(&mut self, callee: Expr) -> PResult<Expr> {
        let args = self.arguments()?;
        let close = self.previous().span;
        Ok(Expr {
            span: callee.span.to(close),
            kind: ExprKind::Call {
                callee: Box::new(callee),
                args,
            },
        })
    }

    /// `(arg ("," arg)* ","?)` after the `(`; consumes the `)`.
    fn arguments(&mut self) -> PResult<Vec<Argument>> {
        let mut args = Vec::new();
        if !self.check(TokenKind::RightParen) {
            loop {
                args.push(self.argument()?);
                if !self.eat(TokenKind::Comma) || self.check(TokenKind::RightParen) {
                    break;
                }
            }
        }
        self.consume(TokenKind::RightParen, "Expect \")\" after arguments.")?;
        Ok(args)
    }

    /// `name: expr` or `expr`.
    fn argument(&mut self) -> PResult<Argument> {
        // The removed `Type:(...)` / `Type:{...}` projection syntax.
        if self.check_type_name()
            && self.check_next(TokenKind::Colon)
            && matches!(
                self.nth(2).kind,
                TokenKind::LeftParen | TokenKind::LeftBrace
            )
            && self.pos + 2 < self.tokens.len()
        {
            let colon = self.nth(1).clone();
            return Err(self.error_at(
                &colon,
                "Constructor projections have been removed; use Type(args).method().",
            ));
        }
        if self.check_name() && self.check_next(TokenKind::Colon) {
            let token = self.advance();
            let name = self.name_of(&token);
            self.consume(TokenKind::Colon, "Expect \":\" after argument name.")?;
            let value = self.expression()?;
            return Ok(Argument {
                span: name.span.to(value.span),
                name: Some(name),
                value,
            });
        }
        let value = self.expression()?;
        Ok(Argument {
            span: value.span,
            name: None,
            value,
        })
    }

    /// `(` at `index` followed by `name is` (or `)`) starts an anonymous
    /// action rather than a grouping.
    fn starts_anonymous_action(&self, index: usize) -> bool {
        index + 2 < self.tokens.len()
            && self.tokens[index].kind == TokenKind::LeftParen
            && (self.tokens[index + 2].kind == TokenKind::Is || Self::group_ends_in_is(&self.tokens, index + 1))
    }

    // ===== primaries =========================================================

    /// Primary expressions. Like `statement`, this is a small dispatcher:
    /// grouping recurses into `expression`, so the bulky forms live in
    /// their own functions to keep the recursive frame small.
    fn primary(&mut self) -> PResult<Expr> {
        let start = self.peek().span;
        match self.peek_kind() {
            TokenKind::At => {
                self.advance();
                self.array_cast(start)
            }
            TokenKind::Try => self.try_expression(start),
            TokenKind::Int
            | TokenKind::Float
            | TokenKind::String
            | TokenKind::Char
            | TokenKind::True
            | TokenKind::False => self.literal(),
            TokenKind::SelfKw => {
                self.advance();
                Ok(Expr {
                    span: start,
                    kind: ExprKind::SelfValue,
                })
            }
            TokenKind::LeftParen => self.parenthesized(start),
            TokenKind::LeftBracket => {
                self.advance();
                self.bracket_literal(start)
            }
            TokenKind::Loop => {
                self.advance();
                self.finish_loop(start)
            }
            TokenKind::If => {
                self.advance();
                self.if_expression(start)
            }
            TokenKind::Match => self.match_expression(start),
            _ if (self.check_type_name() && self.check_next(TokenKind::Less))
                || (self.check_name()
                    && self.check_next(TokenKind::Dot)
                    && is_type_word(self.lexeme(self.nth(2)))
                    && self.nth(3).kind == TokenKind::Less) =>
            {
                let ty = self.type_expr()?;
                Ok(Expr {
                    span: ty.span,
                    kind: ExprKind::TypeValue(ty),
                })
            }
            _ if self.check_name() => {
                let token = self.advance();
                Ok(Expr {
                    span: token.span,
                    kind: ExprKind::Name(self.name_of(&token)),
                })
            }
            _ => {
                let token = self.peek().clone();
                self.reject_reserved(&token)?;
                Err(self.error_here("Expect expression."))
            }
        }
    }

    fn literal(&mut self) -> PResult<Expr> {
        let token = self.advance();
        let literal = match token.kind {
            TokenKind::True => Literal::Bool(true),
            TokenKind::False => Literal::Bool(false),
            _ => match token.value {
                Some(TokenValue::Int(value)) => Literal::Int(value),
                Some(TokenValue::Float(value)) => Literal::Float(value),
                Some(TokenValue::Str(value)) => Literal::String(value),
                Some(TokenValue::Char(value)) => Literal::Char(value),
                // Literal tokens always carry a value; stay total anyway.
                None => return Err(self.error_at(&token, "Expect expression.")),
            },
        };
        Ok(Expr {
            span: token.span,
            kind: ExprKind::Literal(literal),
        })
    }

    /// `try?` / `try!`: the mark is written without a space after `try`.
    pub(super) fn try_mark_follows(&self) -> bool {
        self.check(TokenKind::Try)
            && matches!(self.nth(1).kind, TokenKind::Question | TokenKind::Bang)
            && self.peek().span.end == self.nth(1).span.start
    }

    fn try_expression(&mut self, start: Span) -> PResult<Expr> {
        if self.try_mark_follows() {
            self.advance();
            let force = self.advance().kind == TokenKind::Bang;
            // The mark covers the whole operand to its right, up to `else`/`??`.
            let value = self.logic_or()?;
            return Ok(Expr {
                span: start.to(value.span),
                kind: ExprKind::TryMark {
                    force,
                    value: Box::new(value),
                },
            });
        }
        self.advance();
        let body = self.finish_try(start)?;
        Ok(Expr {
            span: body.span,
            kind: ExprKind::Try(Box::new(body)),
        })
    }

    fn match_expression(&mut self, start: Span) -> PResult<Expr> {
        self.advance();
        let body = self.finish_match(start)?;
        Ok(Expr {
            span: body.span,
            kind: ExprKind::Match(Box::new(body)),
        })
    }

    /// `(` starts a grouping or, when followed by `)` or `name is`, an
    /// anonymous action.
    fn parenthesized(&mut self, start: Span) -> PResult<Expr> {
        self.advance();
        if self.check(TokenKind::RightParen)
            || self.nth(1).kind == TokenKind::Is
            || Self::group_ends_in_is(&self.tokens, self.pos)
        {
            return self.anonymous_action(start);
        }
        let inner = self.expression()?;
        let close = self.consume(TokenKind::RightParen, "Expect \")\" after expression.")?;
        Ok(Expr {
            span: start.to(close.span),
            kind: ExprKind::Grouping(Box::new(inner)),
        })
    }

    /// After `@`: `int|float|byte|string (args)`.
    fn array_cast(&mut self, marker: Span) -> PResult<Expr> {
        let target = self.consume_name("Expect primitive cast type after \"@\".")?;
        if !matches!(target.text.as_str(), "int" | "float" | "byte" | "string") {
            return Err(self.error_at_name(
                &target,
                "Array casts support int, float, byte and string only.",
            ));
        }
        self.consume(TokenKind::LeftParen, "Expect \"(\" after array cast type.")?;
        let args = self.arguments()?;
        let close = self.previous().span;
        Ok(Expr {
            span: marker.to(close),
            kind: ExprKind::ArrayCast {
                marker,
                target,
                args,
            },
        })
    }

    /// After `(`: `params? ")" "is" result block`.
    fn anonymous_action(&mut self, start: Span) -> PResult<Expr> {
        let mut params = Vec::new();
        if !self.check(TokenKind::RightParen) {
            'params: loop {
                for param in self.parameter_group()? {
                if params
                    .iter()
                    .any(|seen: &crate::syntax::ast::Param| seen.name.text == param.name.text)
                {
                    let message = format!(
                        "Anonymous action parameter \"{}\" is declared more than once.",
                        param.name.text
                    );
                    return Err(self.error_at_name(&param.name, &message));
                }
                if param.variadic {
                    return Err(self.error_at_name(
                        &param.name,
                        "Variadic anonymous action parameters are not supported.",
                    ));
                }
                params.push(param);
                }
                if !self.eat(TokenKind::Comma) {
                    break 'params;
                }
                if self.check(TokenKind::RightParen) {
                    return Err(self.error_here("Expect anonymous action parameter after \",\"."));
                }
            }
        }
        self.consume(
            TokenKind::RightParen,
            "Expect \")\" after anonymous action parameters.",
        )?;
        let result_marker = self.consume(
            TokenKind::Is,
            "Anonymous action requires an explicit result: \"is <type>\" or \"is void\".",
        )?;
        let result = match self.result_spec()? {
            ResultSpec::Void(span) => ActionResult::Void(span),
            ResultSpec::Type(ty) => ActionResult::Type(Box::new(ty)),
            ResultSpec::Successor { .. } => {
                return Err(self.error_at(
                    &result_marker,
                    "Anonymous actions cannot declare successor results.",
                ));
            }
        };
        let body = self.block()?;
        Ok(Expr {
            span: start.to(body.span),
            kind: ExprKind::AnonymousAction {
                params,
                result,
                body,
            },
        })
    }

    /// After `[`: `[]`, `[:]`, `[a, b,]` or `[k: v, ...,]`. The first
    /// separator decides between an array and a map.
    fn bracket_literal(&mut self, start: Span) -> PResult<Expr> {
        if self.eat(TokenKind::Colon) {
            let close = self.consume(
                TokenKind::RightBracket,
                "Expect \"]\" after empty map literal.",
            )?;
            return Ok(Expr {
                span: start.to(close.span),
                kind: ExprKind::Map {
                    entries: Vec::new(),
                },
            });
        }
        if self.check(TokenKind::RightBracket) {
            let close = self.advance();
            return Ok(Expr {
                span: start.to(close.span),
                kind: ExprKind::Array {
                    elements: Vec::new(),
                },
            });
        }
        let first = self.expression()?;
        if self.eat(TokenKind::Colon) {
            let value = self.expression()?;
            let mut entries = vec![MapEntry {
                span: first.span.to(value.span),
                key: first,
                value,
            }];
            while self.eat(TokenKind::Comma) && !self.check(TokenKind::RightBracket) {
                let key = self.expression()?;
                self.consume(TokenKind::Colon, "Expect \":\" after map key.")?;
                let value = self.expression()?;
                entries.push(MapEntry {
                    span: key.span.to(value.span),
                    key,
                    value,
                });
            }
            let close = self.consume(TokenKind::RightBracket, "Expect \"]\" after map literal.")?;
            return Ok(Expr {
                span: start.to(close.span),
                kind: ExprKind::Map { entries },
            });
        }
        let mut elements = vec![first];
        while self.eat(TokenKind::Comma) && !self.check(TokenKind::RightBracket) {
            elements.push(self.expression()?);
        }
        let close = self.consume(TokenKind::RightBracket, "Expect \"]\" after array literal.")?;
        Ok(Expr {
            span: start.to(close.span),
            kind: ExprKind::Array { elements },
        })
    }

    /// After `if` in expression position: `expr block "else" block`.
    /// `else if` chains exist only in statement position.
    fn if_expression(&mut self, start: Span) -> PResult<Expr> {
        let condition = self.expression()?;
        let then_block = self.block()?;
        self.consume(TokenKind::Else, "If expression requires else branch.")?;
        let else_block = self.block()?;
        Ok(Expr {
            span: start.to(else_block.span),
            kind: ExprKind::If {
                condition: Box::new(condition),
                then_block,
                else_block,
            },
        })
    }
}

/// What a binary operator builds.
#[derive(Copy, Clone, Debug)]
enum Operator {
    Binary(BinaryOp),
    StreamWrite,
    Range { inclusive: bool },
}

/// How operators of one level combine with each other.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Chaining {
    /// `a - b - c` is `(a - b) - c`.
    Left,
    /// At most one operator: `1..2..3` is an error.
    Single,
}

/// One precedence level of the grammar's ladder.
struct Level {
    chaining: Chaining,
    operators: &'static [(TokenKind, Operator)],
}

/// Every binary operator, by precedence level from the loosest to the
/// tightest: the one place that knows token, AST operation, precedence and
/// chaining (#46). A new operator is a new row; a new level a new entry.
const LEVELS: &[Level] = {
    use BinaryOp as B;
    use Chaining::*;
    use Operator::Binary;
    use TokenKind as T;
    &[
        Level { chaining: Left, operators: &[(T::PipePipe, Binary(B::Or))] },
        Level { chaining: Left, operators: &[(T::AmpAmp, Binary(B::And))] },
        Level { chaining: Left, operators: &[(T::StreamWrite, Operator::StreamWrite)] },
        Level { chaining: Left, operators: &[(T::EqualEqual, Binary(B::Equal)), (T::BangEqual, Binary(B::NotEqual))] },
        Level {
            chaining: Single,
            operators: &[
                (T::DotDot, Operator::Range { inclusive: false }),
                (T::DotDotEqual, Operator::Range { inclusive: true }),
            ],
        },
        Level {
            chaining: Left,
            operators: &[
                (T::Less, Binary(B::Less)),
                (T::LessEqual, Binary(B::LessEqual)),
                (T::Greater, Binary(B::Greater)),
                (T::GreaterEqual, Binary(B::GreaterEqual)),
            ],
        },
        Level { chaining: Left, operators: &[(T::Pipe, Binary(B::BitOr))] },
        Level { chaining: Left, operators: &[(T::Caret, Binary(B::BitXor))] },
        Level { chaining: Left, operators: &[(T::Amp, Binary(B::BitAnd))] },
        Level {
            chaining: Left,
            operators: &[
                (T::ShiftLeft, Binary(B::ShiftLeft)),
                (T::ShiftRight, Binary(B::ShiftRight)),
                (T::ShiftUnsigned, Binary(B::ShiftRightUnsigned)),
            ],
        },
        Level { chaining: Left, operators: &[(T::Plus, Binary(B::Add)), (T::Minus, Binary(B::Subtract))] },
        Level {
            chaining: Left,
            operators: &[(T::Star, Binary(B::Multiply)), (T::Slash, Binary(B::Divide)), (T::Percent, Binary(B::Remainder))],
        },
    ]
};

/// The level (index into [`LEVELS`]) and operation of a binary operator token.
fn binary_operator(kind: TokenKind) -> Option<(usize, Operator)> {
    LEVELS.iter().enumerate().find_map(|(level, entry)| {
        entry.operators.iter().find(|(token, _)| *token == kind).map(|&(_, operator)| (level, operator))
    })
}
