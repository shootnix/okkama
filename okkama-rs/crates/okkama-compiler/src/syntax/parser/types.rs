//! Type expressions, result specifications and generic parameter lists.

use super::{PResult, Parser};
use crate::source::Span;
use crate::syntax::ast::{ActionResult, GenericParam, Name, ResultSpec, TypeExpr, TypeKind};
use crate::syntax::token::TokenKind;

impl Parser<'_> {
    fn check_void(&self) -> bool {
        self.check(TokenKind::Identifier) && self.lexeme(self.peek()) == "void"
    }

    /// `result_spec = type | "^" type | "void"` (after `is`).
    pub(super) fn result_spec(&mut self) -> PResult<ResultSpec> {
        if self.check_void() {
            let token = self.advance();
            return Ok(ResultSpec::Void(token.span));
        }
        if self.eat(TokenKind::Caret) {
            // `is ^Type` marks a successor action.
            let caret = self.previous().span;
            let ty = self.type_expr()?;
            let span = caret.to(ty.span);
            return Ok(ResultSpec::Successor { ty, span });
        }
        Ok(ResultSpec::Type(self.type_expr()?))
    }

    /// `type_ref = "@" type | "%" type ":" type | action_type | named_type`.
    ///
    /// Collection markers are prefix-recursive; then come action signatures,
    /// qualified names with optional generic arguments and an optional
    /// `::Cap` selection.
    pub(super) fn type_expr(&mut self) -> PResult<TypeExpr> {
        self.nested(|parser| parser.type_expr_inner())
    }

    fn type_expr_inner(&mut self) -> PResult<TypeExpr> {
        if self.check_void() {
            return Err(self.error_here("void is only allowed as an action result marker."));
        }
        let start = self.peek().span;
        let is_async = self.eat(TokenKind::Async);
        if is_async && !self.check(TokenKind::LeftParen) {
            return Err(self.error_here("Expect \"(\" after \"async\" in an async action type."));
        }
        if !is_async && self.eat(TokenKind::At) {
            let element = self.type_expr()?;
            let span = start.to(element.span);
            return Ok(TypeExpr {
                kind: TypeKind::Array(Box::new(element)),
                span,
            });
        }
        if !is_async && self.eat(TokenKind::Percent) {
            let key = self.type_expr()?;
            self.consume(
                TokenKind::Colon,
                "Expect \":\" between map key and value types.",
            )?;
            let value = self.type_expr()?;
            let span = start.to(value.span);
            return Ok(TypeExpr {
                kind: TypeKind::Map {
                    key: Box::new(key),
                    value: Box::new(value),
                },
                span,
            });
        }
        if self.eat(TokenKind::LeftParen) {
            return self.action_type(start, is_async);
        }
        self.named_type()
    }

    /// `(A, B) is R`, `() is void`, `async (A) is R` — the `(` is consumed.
    fn action_type(&mut self, start: Span, is_async: bool) -> PResult<TypeExpr> {
        let mut params = Vec::new();
        if !self.check(TokenKind::RightParen) {
            loop {
                params.push(self.type_expr()?);
                if !self.eat(TokenKind::Comma) {
                    break;
                }
                if self.check(TokenKind::RightParen) {
                    return Err(self.error_here("Expect action parameter type after \",\"."));
                }
            }
        }
        self.consume(
            TokenKind::RightParen,
            "Expect \")\" after action parameter types.",
        )?;
        self.consume(TokenKind::Is, "Expect \"is\" after action parameter types.")?;
        let result = if self.check_void() {
            ActionResult::Void(self.advance().span)
        } else {
            ActionResult::Type(Box::new(self.type_expr()?))
        };
        let end = match &result {
            ActionResult::Void(span) => *span,
            ActionResult::Type(ty) => ty.span,
        };
        Ok(TypeExpr {
            kind: TypeKind::Action {
                is_async,
                params,
                result,
            },
            span: start.to(end),
        })
    }

    /// `qualified_name generic_args? ("::" name)?`
    fn named_type(&mut self) -> PResult<TypeExpr> {
        let mut path = vec![self.consume_name("Expect name.")?];
        while self.eat(TokenKind::Dot) {
            path.push(self.consume_name("Expect name after \".\".")?);
        }
        let start = path[0].span;
        let mut end = path[path.len() - 1].span;
        let mut args = Vec::new();
        if self.eat(TokenKind::Less) {
            if self.check(TokenKind::Greater) {
                return Err(self.error_here("Expect type argument."));
            }
            loop {
                args.push(self.type_expr()?);
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            end = self
                .consume(TokenKind::Greater, "Expect \">\" after type arguments.")?
                .span;
        }
        let mut ty = TypeExpr {
            kind: TypeKind::Named { path, args },
            span: start.to(end),
        };
        if self.eat(TokenKind::DoubleColon) {
            let name = self.consume_name("Expect cap name after \"::\".")?;
            let span = ty.span.to(name.span);
            ty = TypeExpr {
                kind: TypeKind::Nested {
                    owner: Box::new(ty),
                    name,
                },
                span,
            };
        }
        Ok(ty)
    }

    /// `"<" TYPE_NAME (":" bound)? ("," TYPE_NAME (":" bound)?)* ","? ">"`
    /// after an object or role name. `owner_kind` is `object` or `role`.
    pub(super) fn generic_params(
        &mut self,
        owner_kind: &str,
        owner: &Name,
    ) -> PResult<Vec<GenericParam>> {
        let mut params: Vec<GenericParam> = Vec::new();
        if !self.eat(TokenKind::Less) {
            return Ok(params);
        }
        if self.check(TokenKind::Greater) {
            return Err(self.error_here(format!(
                "Generic {owner_kind} parameter list requires at least one type parameter."
            )));
        }
        loop {
            if !self.check_type_name() {
                return Err(self.error_here("Generic type parameter must be a type name."));
            }
            let token = self.advance();
            let name = self.name_of(&token);
            if name.text == owner.text {
                return Err(self.error_at(
                    &token,
                    format!(
                        "Generic type parameter \"{}\" cannot shadow {owner_kind} \"{}\".",
                        name.text, owner.text
                    ),
                ));
            }
            if params.iter().any(|param| param.name.text == name.text) {
                return Err(self.error_at(
                    &token,
                    format!(
                        "Generic type parameter \"{}\" is declared more than once.",
                        name.text
                    ),
                ));
            }
            let bound = if self.eat(TokenKind::Colon) {
                Some(self.type_expr()?)
            } else {
                None
            };
            let span = bound
                .as_ref()
                .map_or(name.span, |bound| name.span.to(bound.span));
            params.push(GenericParam { name, bound, span });
            if !self.eat(TokenKind::Comma) || self.check(TokenKind::Greater) {
                break;
            }
        }
        self.consume(
            TokenKind::Greater,
            &format!("Expect \">\" after generic {owner_kind} parameters."),
        )?;
        Ok(params)
    }
}
