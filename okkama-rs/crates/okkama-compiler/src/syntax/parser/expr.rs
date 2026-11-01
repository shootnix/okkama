//! Expressions.
//!
//! Precedence, lowest first. All binary levels are left-associative except
//! ranges, which do not chain. The binary levels are parsed by precedence
//! climbing over the table in [`binary_operator`]; the other levels are
//! one function each, mirroring `docs/okkama-grammar-1.0.md`.
//!
//! | level          | operators                       |
//! |----------------|---------------------------------|
//! | functional     | `call -> bindings { }` suffix   |
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
//! | unary          | `!` `-` `~` `await` `spawn` `panic`, projections |
//! | postfix        | call, `.name`, `::name`, `[i] else f`, `...` |

use super::{PResult, Parser};
use crate::source::Span;
use crate::syntax::ast::{
    ActionResult, Argument, BinaryOp, Expr, ExprKind, FunctionalOp, Literal, MapEntry, Name,
    ResultSpec, UnaryOp,
};
use crate::syntax::token::{TokenKind, TokenValue, is_lower_word, is_type_word};

impl Parser<'_> {
    /// `expr = logic_or`.
    ///
    /// The functional suffix `-> bindings { }` is a postfix form of a member
    /// call (`receiver.map(args)`, see [`Self::finish_postfix`]), so it binds
    /// tighter than every operator: `total + xs.reduce(0) -> a, x { a + x; }`.
    /// A nested expression is never the top level of a `for` source.
    pub(super) fn expression(&mut self) -> PResult<Expr> {
        let enclosing = std::mem::replace(&mut self.for_source, false);
        let expr = self.nested(|parser| parser.logic_or());
        self.for_source = enclosing;
        expr
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
    /// the same table ([`binary_operator`]): identical trees, a few frames
    /// per nesting level. All levels are left-associative; the range level
    /// accepts at most one operator.
    pub(super) fn logic_or(&mut self) -> PResult<Expr> {
        self.binary(Precedence::Or)
    }

    /// Parses operators whose precedence is at least `min`.
    fn binary(&mut self, min: Precedence) -> PResult<Expr> {
        let mut left = self.unary()?;
        while let Some((precedence, operator)) = binary_operator(self.peek_kind()) {
            if precedence < min {
                break;
            }
            self.advance();
            let op_span = self.previous().span;
            // Left associativity: the right operand only takes operators
            // that bind strictly tighter.
            let right = self.binary(precedence.next())?;
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
            if matches!(operator, Operator::Range { .. })
                && matches!(self.peek_kind(), TokenKind::DotDot | TokenKind::DotDotEqual)
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
            return Err(
                self.error_here("Constructor projections use a declared prefix, not \"&\".")
            );
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
            return self.named_projection();
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

    fn named_projection(&mut self) -> PResult<Expr> {
        let prefix_token = self.advance();
        let prefix = self.name_of(&prefix_token);
        let mut target = self.primary()?;
        if self.eat(TokenKind::Dot) {
            let name = self.consume_name("Expect object name after \".\".")?;
            target = Expr {
                span: target.span.to(name.span),
                kind: ExprKind::Get {
                    object: Box::new(target),
                    name,
                },
            };
        }
        if !is_projection_target(&target) {
            return Err(self.error_at(
                &prefix_token,
                "Named constructor projection target must be an object name.",
            ));
        }
        if !self.eat(TokenKind::LeftParen) {
            return Err(self.error_here("Expect \"(\" after named constructor projection target."));
        }
        let construction = self.finish_call(target)?;
        let projection = Expr {
            span: prefix.span.to(construction.span),
            kind: ExprKind::Projection {
                prefix,
                construction: Box::new(construction),
            },
        };
        self.finish_postfix(projection)
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
                return Err(
                    self.error_here("Constructor projections use a declared prefix, not \":\".")
                );
            }
            if self.eat(TokenKind::Dot) {
                let name = self.consume_member_name("Expect property name after \".\".")?;
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
                "Constructor projections use a declared prefix, not \":\".",
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
            && self.tokens[index + 2].kind == TokenKind::Is
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

    fn try_expression(&mut self, start: Span) -> PResult<Expr> {
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
        if self.check(TokenKind::RightParen) || self.nth(1).kind == TokenKind::Is {
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
            loop {
                let param = self.parameter()?;
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
                if !self.eat(TokenKind::Comma) {
                    break;
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

/// `Type` or `module.Type` — the only valid projection targets. The
/// qualifier only has to be lower-case-shaped (the reference parser treats
/// `self` like any other variable here); whether it names a module is a
/// semantic question.
fn is_projection_target(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Name(name) => is_type_word(&name.text),
        ExprKind::Get { object, name } => {
            let lower_qualifier = match &object.kind {
                ExprKind::Name(module) => is_lower_word(&module.text),
                ExprKind::SelfValue => true,
                _ => false,
            };
            lower_qualifier && is_type_word(&name.text)
        }
        _ => false,
    }
}

/// Binary precedence levels, lowest first (the grammar's ladder).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Precedence {
    Or,
    And,
    StreamWrite,
    Equality,
    Range,
    Comparison,
    BitOr,
    BitXor,
    BitAnd,
    Shift,
    Term,
    Factor,
    /// Above every binary operator: only prefix/postfix forms.
    Unary,
}

impl Precedence {
    fn next(self) -> Precedence {
        use Precedence::*;
        match self {
            Or => And,
            And => StreamWrite,
            StreamWrite => Equality,
            Equality => Range,
            Range => Comparison,
            Comparison => BitOr,
            BitOr => BitXor,
            BitXor => BitAnd,
            BitAnd => Shift,
            Shift => Term,
            Term => Factor,
            Factor | Unary => Unary,
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum Operator {
    Binary(BinaryOp),
    StreamWrite,
    Range { inclusive: bool },
}

/// The precedence table:
///
/// | level        | operators                |
/// |--------------|--------------------------|
/// | Or           | `\|\|`                     |
/// | And          | `&&`                     |
/// | StreamWrite  | `<-`                     |
/// | Equality     | `==` `!=`                |
/// | Range        | `..` `..=`               |
/// | Comparison   | `<` `<=` `>` `>=`        |
/// | BitOr        | `\|`                      |
/// | BitXor       | `^`                      |
/// | BitAnd       | `&`                      |
/// | Shift        | `<<` `>>` `>>>`          |
/// | Term         | `+` `-`                  |
/// | Factor       | `*` `/` `%`              |
fn binary_operator(kind: TokenKind) -> Option<(Precedence, Operator)> {
    use BinaryOp as B;
    use Precedence as P;
    let entry = match kind {
        TokenKind::PipePipe => (P::Or, Operator::Binary(B::Or)),
        TokenKind::AmpAmp => (P::And, Operator::Binary(B::And)),
        TokenKind::StreamWrite => (P::StreamWrite, Operator::StreamWrite),
        TokenKind::EqualEqual => (P::Equality, Operator::Binary(B::Equal)),
        TokenKind::BangEqual => (P::Equality, Operator::Binary(B::NotEqual)),
        TokenKind::DotDot => (P::Range, Operator::Range { inclusive: false }),
        TokenKind::DotDotEqual => (P::Range, Operator::Range { inclusive: true }),
        TokenKind::Less => (P::Comparison, Operator::Binary(B::Less)),
        TokenKind::LessEqual => (P::Comparison, Operator::Binary(B::LessEqual)),
        TokenKind::Greater => (P::Comparison, Operator::Binary(B::Greater)),
        TokenKind::GreaterEqual => (P::Comparison, Operator::Binary(B::GreaterEqual)),
        TokenKind::Pipe => (P::BitOr, Operator::Binary(B::BitOr)),
        TokenKind::Caret => (P::BitXor, Operator::Binary(B::BitXor)),
        TokenKind::Amp => (P::BitAnd, Operator::Binary(B::BitAnd)),
        TokenKind::ShiftLeft => (P::Shift, Operator::Binary(B::ShiftLeft)),
        TokenKind::ShiftRight => (P::Shift, Operator::Binary(B::ShiftRight)),
        TokenKind::ShiftUnsigned => (P::Shift, Operator::Binary(B::ShiftRightUnsigned)),
        TokenKind::Plus => (P::Term, Operator::Binary(B::Add)),
        TokenKind::Minus => (P::Term, Operator::Binary(B::Subtract)),
        TokenKind::Star => (P::Factor, Operator::Binary(B::Multiply)),
        TokenKind::Slash => (P::Factor, Operator::Binary(B::Divide)),
        TokenKind::Percent => (P::Factor, Operator::Binary(B::Remainder)),
        _ => return None,
    };
    Some(entry)
}
