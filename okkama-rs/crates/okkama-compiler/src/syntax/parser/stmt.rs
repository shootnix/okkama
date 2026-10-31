//! Blocks and statements.

use super::{PResult, Parser};
use crate::source::Span;
use crate::syntax::ast::{
    BatchSize, Block, CatchClause, ConfirmFallback, ElseBranch, Expr, ExprKind, ForBinding, IfStmt,
    MatchBody, MatchCase, Stmt, StmtKind, TryBody,
};
use crate::syntax::token::{TokenKind, TokenValue};

impl Parser<'_> {
    /// `block = "{" statement* "}"`
    pub(super) fn block(&mut self) -> PResult<Block> {
        self.nested(|parser| {
            let open = parser.consume(TokenKind::LeftBrace, "Expect \"{\" before block.")?;
            parser.finish_block(open.span)
        })
    }

    /// Statements up to and including the closing `}` of a block whose `{`
    /// (at `open`) was already consumed.
    fn finish_block(&mut self, open: Span) -> PResult<Block> {
        let mut stmts = Vec::new();
        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            stmts.push(self.statement()?);
        }
        let close = self.consume(TokenKind::RightBrace, "Expect \"}\" after block.")?;
        Ok(Block {
            stmts,
            span: open.to(close.span),
        })
    }

    /// One statement. Statement keywords are tried first; then the two
    /// assignment-shaped forms; everything else is an expression statement.
    ///
    /// Every form is parsed by its own function so this dispatcher keeps a
    /// small stack frame: blocks nest statements recursively.
    fn statement(&mut self) -> PResult<Stmt> {
        self.reject_native_outside_module()?;
        let start = self.peek().span;
        let kind = match self.peek_kind() {
            TokenKind::Let => self.let_statement()?,
            TokenKind::Defer => self.defer_statement()?,
            TokenKind::Try => {
                self.advance();
                StmtKind::Try(self.finish_try(start)?)
            }
            TokenKind::Throw => self.throw_statement()?,
            TokenKind::Return => self.return_statement()?,
            TokenKind::Catch => return Err(self.error_here("\"catch\" must follow a try block.")),
            TokenKind::Finally => return Err(self.error_here("finally is not supported.")),
            TokenKind::Confirm => self.confirm_statement()?,
            TokenKind::Caret => self.successor_assignment()?,
            TokenKind::LeftBrace => self.block_statement()?,
            TokenKind::If => {
                self.advance();
                StmtKind::If(self.if_statement(start)?)
            }
            TokenKind::Match => {
                self.advance();
                StmtKind::Match(self.finish_match(start)?)
            }
            TokenKind::For => self.for_statement()?,
            TokenKind::Loop => self.loop_statement(start)?,
            TokenKind::Next => self.next_statement()?,
            TokenKind::Last => self.last_statement()?,
            _ if self.is_member_field_bind() => self.self_field_bind()?,
            _ if self.check_name() && self.check_next(TokenKind::Equal) => self.field_bind()?,
            _ => self.expression_statement()?,
        };
        let end = self.previous().span;
        Ok(Stmt {
            kind,
            span: start.to(end),
        })
    }

    /// `defer expr ;`
    fn defer_statement(&mut self) -> PResult<StmtKind> {
        self.advance();
        let value = self.expression()?;
        self.consume(TokenKind::Semicolon, "Expect \";\" after defer statement.")?;
        Ok(StmtKind::Defer { value })
    }

    /// `throw expr? ;` — the bare form rethrows.
    fn throw_statement(&mut self) -> PResult<StmtKind> {
        self.advance();
        let value = if self.check(TokenKind::Semicolon) {
            None
        } else {
            Some(self.expression()?)
        };
        self.consume(TokenKind::Semicolon, "Expect \";\" after throw statement.")?;
        Ok(StmtKind::Throw { value })
    }

    /// `return expr? ;` — exactly one value.
    fn return_statement(&mut self) -> PResult<StmtKind> {
        self.advance();
        let value = if self.check(TokenKind::Semicolon) {
            None
        } else {
            Some(self.expression()?)
        };
        self.consume(
            TokenKind::Semicolon,
            "Expect \";\" after return; only one value is allowed.",
        )?;
        Ok(StmtKind::Return { value })
    }

    /// `^ name = expr ;`
    fn successor_assignment(&mut self) -> PResult<StmtKind> {
        self.advance();
        let name = self.consume_identifier("Expect successor field name after \"^\".")?;
        self.consume(TokenKind::Equal, "Expect \"=\" after successor field name.")?;
        let value = self.expression()?;
        self.consume(
            TokenKind::Semicolon,
            "Expect \";\" after successor assignment.",
        )?;
        Ok(StmtKind::SuccessorAssign { name, value })
    }

    /// A nested `{ ... }` block statement; the nesting guard bounds depth.
    fn block_statement(&mut self) -> PResult<StmtKind> {
        let block = self.nested(|parser| {
            let open = parser.advance().span;
            parser.finish_block(open)
        })?;
        Ok(StmtKind::Block(block))
    }

    /// `loop { }` in statement position is an expression statement whose
    /// terminating `;` is optional.
    fn loop_statement(&mut self, start: Span) -> PResult<StmtKind> {
        self.advance();
        let expr = self.finish_loop(start)?;
        self.eat(TokenKind::Semicolon);
        Ok(StmtKind::Expr {
            expr,
            condition: None,
        })
    }

    /// `self . name = expr ;`; any other receiver is rejected because
    /// object fields are not mutable.
    fn self_field_bind(&mut self) -> PResult<StmtKind> {
        if !self.check(TokenKind::SelfKw) {
            return Err(self.error_here(
                "Only \"self.field\" may be assigned; object fields are not mutable.",
            ));
        }
        let receiver = self.advance().span;
        self.consume(TokenKind::Dot, "Expect \".\" after \"self\".")?;
        let name = self.consume_identifier("Expect field name after \"self.\".")?;
        self.consume(TokenKind::Equal, "Expect \"=\" after receiver field.")?;
        let value = self.expression()?;
        self.consume(
            TokenKind::Semicolon,
            "Expect \";\" after receiver field bind.",
        )?;
        Ok(StmtKind::SelfFieldBind {
            receiver,
            name,
            value,
        })
    }

    /// `name = expr ;` — a constructor field bind or a local rebinding.
    fn field_bind(&mut self) -> PResult<StmtKind> {
        let name = self.consume_identifier("Expect field name.")?;
        self.consume(TokenKind::Equal, "Expect \"=\" after field name.")?;
        let value = self.expression()?;
        self.consume(TokenKind::Semicolon, "Expect \";\" after field bind.")?;
        Ok(StmtKind::FieldBind { name, value })
    }

    /// `expr ("if" expr)? ;`
    fn expression_statement(&mut self) -> PResult<StmtKind> {
        let expr = self.expression()?;
        let condition = if self.eat(TokenKind::If) {
            Some(self.expression()?)
        } else {
            None
        };
        self.consume(TokenKind::Semicolon, "Expect \";\" after expression.")?;
        Ok(StmtKind::Expr { expr, condition })
    }

    /// `receiver "." name "="` where the receiver is `self` or any name.
    /// Only `self.field = ...` is valid; other receivers get an explicit
    /// error instead of being parsed as an expression.
    fn is_member_field_bind(&self) -> bool {
        if self.pos + 3 >= self.tokens.len() {
            return false;
        }
        let receiver = &self.tokens[self.pos];
        (receiver.kind == TokenKind::SelfKw || self.is_name_token(receiver))
            && self.tokens[self.pos + 1].kind == TokenKind::Dot
            && self.is_name_token(&self.tokens[self.pos + 2])
            && self.tokens[self.pos + 3].kind == TokenKind::Equal
    }

    /// `let name ("is" type)? "=" expr ";"`
    fn let_statement(&mut self) -> PResult<StmtKind> {
        self.advance();
        let name = self.consume_binding_name("Expect variable name after \"let\".")?;
        let ty = if self.eat(TokenKind::Is) {
            Some(self.type_expr()?)
        } else {
            None
        };
        self.consume(TokenKind::Equal, "Expect \"=\" after variable name.")?;
        let value = self.expression()?;
        self.consume(TokenKind::Semicolon, "Expect \";\" after let declaration.")?;
        Ok(StmtKind::Let { name, ty, value })
    }

    /// After `try`: `block catch_clause+`. Shared by the statement and the
    /// expression form.
    pub(super) fn finish_try(&mut self, start: Span) -> PResult<TryBody> {
        let body = self.block()?;
        if !self.eat(TokenKind::Catch) {
            return Err(self.error_here("Expect \"catch\" after try block."));
        }
        let mut catches = vec![self.catch_clause()?];
        while self.eat(TokenKind::Catch) {
            catches.push(self.catch_clause()?);
        }
        if self.check(TokenKind::Finally) {
            return Err(self.error_here("finally is not supported."));
        }
        let end = catches.last().map_or(body.span, |clause| clause.span);
        Ok(TryBody {
            body,
            catches,
            span: start.to(end),
        })
    }

    /// After `catch`: `name "is" type block`.
    fn catch_clause(&mut self) -> PResult<CatchClause> {
        let keyword = self.previous().span;
        if self.check(TokenKind::Is) {
            return Err(self.error_here("Expect error name after \"catch\"."));
        }
        let name = self.consume_binding_name("Expect error name after \"catch\".")?;
        self.consume(TokenKind::Is, "Expect \"is\" after catch error name.")?;
        if !self.check(TokenKind::At) && !self.check(TokenKind::Percent) && !self.check_name() {
            return Err(self.error_here("Expect error type after \"is\"."));
        }
        let ty = self.type_expr()?;
        if !self.check(TokenKind::LeftBrace) {
            return Err(self.error_here("Expect \"{\" before catch body."));
        }
        let body = self.block()?;
        Ok(CatchClause {
            span: keyword.to(body.span),
            name,
            ty,
            body,
        })
    }

    /// `confirm expr ("else" ("panic" | "throw") expr)? ";"`
    fn confirm_statement(&mut self) -> PResult<StmtKind> {
        self.advance();
        let condition = self.expression()?;
        let mut fallback = None;
        if self.eat(TokenKind::Else) {
            if self.eat(TokenKind::Panic) {
                let keyword = self.previous().span;
                let value = self.panic_value()?;
                fallback = Some(ConfirmFallback::Panic { keyword, value });
            } else if self.eat(TokenKind::Throw) {
                let keyword = self.previous().span;
                let value = self.expression()?;
                fallback = Some(ConfirmFallback::Throw { keyword, value });
            } else {
                return Err(self.error_here("Expect \"throw\" or \"panic\" after confirm else."));
            }
        }
        self.consume(
            TokenKind::Semicolon,
            "Expect \";\" after confirm statement.",
        )?;
        Ok(StmtKind::Confirm {
            condition,
            fallback,
        })
    }

    /// After `if`: `expr block ("else" (if_stmt | block))?`. `else if`
    /// chains recurse, so each link counts against the nesting limit.
    fn if_statement(&mut self, start: Span) -> PResult<IfStmt> {
        self.nested(|parser| {
            let condition = parser.expression()?;
            let then_block = parser.block()?;
            let mut else_branch = None;
            if parser.eat(TokenKind::Else) {
                if parser.eat(TokenKind::If) {
                    let nested_start = parser.previous().span;
                    else_branch =
                        Some(ElseBranch::If(Box::new(parser.if_statement(nested_start)?)));
                } else {
                    else_branch = Some(ElseBranch::Block(parser.block()?));
                }
            }
            let end = match &else_branch {
                None => then_block.span,
                Some(ElseBranch::Block(block)) => block.span,
                Some(ElseBranch::If(nested)) => nested.span,
            };
            Ok(IfStmt {
                condition,
                then_block,
                else_branch,
                span: start.to(end),
            })
        })
    }

    /// After `match`: `expr "{" ("case" exprs block | "else" block)* "}"`.
    /// Shared by statements and expressions.
    pub(super) fn finish_match(&mut self, start: Span) -> PResult<MatchBody> {
        let subject = self.expression()?;
        self.consume(TokenKind::LeftBrace, "Expect \"{\" after match subject.")?;
        let mut cases = Vec::new();
        let mut else_block = None;
        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            if self.eat(TokenKind::Case) {
                let keyword = self.previous().span;
                let mut patterns = vec![self.expression()?];
                while self.eat(TokenKind::Comma) {
                    patterns.push(self.expression()?);
                }
                let body = self.block()?;
                cases.push(MatchCase {
                    patterns,
                    span: keyword.to(body.span),
                    body,
                });
                continue;
            }
            if self.eat(TokenKind::Else) {
                if else_block.is_some() {
                    let token = self.previous().clone();
                    return Err(self.error_at(&token, "Match cannot have multiple else branches."));
                }
                else_block = Some(self.block()?);
                continue;
            }
            return Err(self.error_here("Expect \"case\" or \"else\" in match."));
        }
        let close = self.consume(TokenKind::RightBrace, "Expect \"}\" after match.")?;
        Ok(MatchBody {
            subject,
            cases,
            else_block,
            span: start.to(close.span),
        })
    }

    /// `for source "->" binding ("," binding)* block`.
    ///
    /// A top-level arrow in the source always belongs to the `for` itself,
    /// so a functional call there is not completed and a functional source
    /// has to be grouped.
    fn for_statement(&mut self) -> PResult<StmtKind> {
        self.advance();
        if self.check(TokenKind::LeftBrace) {
            let keyword = self.previous().clone();
            return Err(self.error_at(
                &keyword,
                "Source-less \"for\" is not supported; use \"loop { ... }\".",
            ));
        }
        let enclosing = std::mem::replace(&mut self.for_source, true);
        let source = self.logic_or();
        self.for_source = enclosing;
        let source = source?;
        if !self.eat(TokenKind::Arrow) {
            return Err(self.error_here("Expect \"->\" and a binding after for source."));
        }
        let mut bindings = vec![self.for_binding()?];
        while self.eat(TokenKind::Comma) {
            bindings.push(self.for_binding()?);
        }
        let body = self.block()?;
        Ok(StmtKind::For {
            source,
            bindings,
            body,
        })
    }

    fn for_binding(&mut self) -> PResult<ForBinding> {
        let name = self.consume_binding_name("Expect loop binding name after \"->\".")?;
        let mut span = name.span;
        let mut batch = None;
        if self.eat(TokenKind::LeftBracket) {
            let token = self.consume(TokenKind::Int, "Expect batch size integer.")?;
            let literal = match &token.value {
                Some(TokenValue::Int(literal)) => literal.clone(),
                _ => return Err(self.error_at(&token, "Expect batch size integer.")),
            };
            batch = Some(BatchSize {
                literal,
                span: token.span,
            });
            let close = self.consume(TokenKind::RightBracket, "Expect \"]\" after batch size.")?;
            span = span.to(close.span);
        }
        Ok(ForBinding { name, batch, span })
    }

    /// `next ("if" expr)? ";"` — `next` never carries a value.
    fn next_statement(&mut self) -> PResult<StmtKind> {
        self.advance();
        let condition = if self.eat(TokenKind::If) {
            Some(self.expression()?)
        } else {
            None
        };
        if condition.is_none() && !self.check(TokenKind::Semicolon) {
            return Err(self.error_here("\"next\" does not accept a value."));
        }
        self.consume(
            TokenKind::Semicolon,
            "Expect \";\" after loop control statement.",
        )?;
        Ok(StmtKind::Next { condition })
    }

    /// `last ("if" expr | expr ("if" expr)?)? ";"`. In `last if c`
    /// the `if` introduces the condition; an `if` expression used as the
    /// payload has to be grouped.
    fn last_statement(&mut self) -> PResult<StmtKind> {
        self.advance();
        let mut value = None;
        let mut condition = None;
        if self.eat(TokenKind::If) {
            condition = Some(self.expression()?);
        } else if !self.check(TokenKind::Semicolon) {
            value = Some(self.expression()?);
            if self.eat(TokenKind::If) {
                condition = Some(self.expression()?);
            }
        }
        self.consume(
            TokenKind::Semicolon,
            "Expect \";\" after loop control statement.",
        )?;
        Ok(StmtKind::Last { value, condition })
    }

    /// After `loop`: a block. Shared by the statement and expression form.
    pub(super) fn finish_loop(&mut self, start: Span) -> PResult<Expr> {
        if !self.check(TokenKind::LeftBrace) {
            return Err(self.error_here("Expect \"{\" after \"loop\"."));
        }
        let body = self.block()?;
        Ok(Expr {
            span: start.to(body.span),
            kind: ExprKind::Loop { body },
        })
    }
}
