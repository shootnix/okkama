//! Module items and object/role members.

use super::{PResult, Parser};
use crate::source::Span;
use crate::syntax::ParseMode;
use crate::syntax::ast::{
    ActionDecl, ActionSignature, CapDecl, ComputedField, ConstructorDecl, ConstructorForm,
    DestroyDecl, EnumDecl, FakeBody, FakeDecl, HasDecl, Item, Name, NativeActionDecl,
    NativeObjectDecl, ObjectDecl, ObjectMember, Param, PrivateSection, ProjectionDirective,
    RoleDecl, RoleMember, TestBody, TestDecl, Visibility,
};
use crate::syntax::token::{TokenKind, is_lower_word, is_type_word};

/// How a parameter list is used; selects the context-specific checks.
#[derive(Copy, Clone, PartialEq, Eq)]
enum ParamContext {
    Action,
    Signature,
    Constructor,
    Native,
}

impl ParamContext {
    fn description(self) -> &'static str {
        match self {
            ParamContext::Action => "action parameters",
            ParamContext::Signature => "action signature parameters",
            ParamContext::Constructor => "constructor parameters",
            ParamContext::Native => "native declaration parameters",
        }
    }
}

impl Parser<'_> {
    // ===== module scope ======================================================

    /// One module-scope declaration.
    pub(super) fn module_item(&mut self) -> PResult<Item> {
        if self.check_cap_declaration() {
            return Ok(Item::Cap(self.cap_declaration(true)?));
        }
        if self.check_view_declaration() {
            return Err(self.error_here("Object views have been removed."));
        }
        if self.check(TokenKind::Async) && self.check_next(TokenKind::Native) {
            return Err(self.error_here("Native declarations cannot be async."));
        }
        if self.eat(TokenKind::Object) {
            return Ok(Item::Object(self.object_declaration()?));
        }
        if self.eat(TokenKind::Role) {
            return Ok(Item::Role(self.role_declaration()?));
        }
        if self.eat(TokenKind::Enum) {
            return Ok(Item::Enum(self.enum_declaration()?));
        }
        if self.eat(TokenKind::Test) {
            return Ok(Item::Test(self.test_block(false)?));
        }
        if self.check_async_test() {
            return Ok(Item::Test(self.async_test_block()?));
        }
        if self.eat(TokenKind::Native) {
            return self.native_declaration();
        }
        if self.eat(TokenKind::Import) {
            let token = self.previous().clone();
            return Err(self.error_at(&token, "Imports must appear before module declarations."));
        }
        if self.check_async_modifier() || self.check_name() {
            let decl = self.action_declaration(Visibility::Public, None)?;
            if decl.name.text != "main" {
                return Err(self.error_at_span(
                    decl.name.span,
                    "Only main() may be declared at module scope.",
                ));
            }
            if !decl.params.is_empty() {
                return Err(self.error_at_span(decl.name.span, "main() cannot declare parameters."));
            }
            return Ok(Item::Action(decl));
        }
        let token = self.peek().clone();
        self.reject_reserved(&token)?;
        Err(self.error_here("Expect module declaration item."))
    }

    /// `async test` (as opposed to an `async` action named `test(...)`).
    fn check_async_test(&self) -> bool {
        self.check(TokenKind::Async)
            && self.check_next(TokenKind::Test)
            && !self.check_async_modifier()
    }

    /// `cap` is contextual: it starts a cap declaration only when followed
    /// by another identifier (`cap Card {`, `cap Owner::Card {`).
    fn check_cap_declaration(&self) -> bool {
        self.check(TokenKind::Identifier)
            && self.lexeme(self.peek()) == "cap"
            && self.nth(1).kind == TokenKind::Identifier
    }

    /// The removed `view Name(...)` member syntax gets a targeted error.
    fn check_view_declaration(&self) -> bool {
        self.check(TokenKind::Identifier)
            && self.lexeme(self.peek()) == "view"
            && self.nth(1).kind == TokenKind::Identifier
            && is_type_word(self.lexeme(self.nth(1)))
    }

    pub(super) fn reject_native_outside_module(&self) -> PResult<()> {
        if self.check(TokenKind::Native)
            || (self.check(TokenKind::Async) && self.check_next(TokenKind::Native))
        {
            return Err(self.error_here("Native declarations are allowed only at module scope."));
        }
        Ok(())
    }

    // ===== natives ===========================================================

    /// After `native`: `object Name;` or `async? name(params) is Result;`.
    fn native_declaration(&mut self) -> PResult<Item> {
        let keyword = self.previous().span;
        if self.check(TokenKind::Object) {
            return Ok(Item::NativeObject(self.native_object(keyword)?));
        }
        let is_async = self.eat(TokenKind::Async);
        let name = self.consume_action_name("Expect native declaration name.")?;
        if name.text == "main" {
            return Err(
                self.error_at_span(name.span, "Native declaration cannot be named \"main\".")
            );
        }
        let params = self.parameter_list(ParamContext::Native)?;
        if let Some(variadic) = params.iter().find(|param| param.variadic) {
            return Err(self.error_at_span(
                variadic.name.span,
                "Variadic native declarations are not supported in Phase 01.",
            ));
        }
        self.consume(
            TokenKind::Is,
            &format!(
                "Native declaration \"{}\" requires a return type.",
                name.text
            ),
        )?;
        let result = self.result_spec()?;
        if self.check(TokenKind::LeftBrace) {
            return Err(
                self.error_here("Native declaration requires \";\" and cannot have a body.")
            );
        }
        let semicolon = self.consume(
            TokenKind::Semicolon,
            "Expect \";\" after native declaration.",
        )?;
        Ok(Item::NativeAction(NativeActionDecl {
            is_async,
            name,
            params,
            result,
            span: keyword.to(semicolon.span),
        }))
    }

    fn native_object(&mut self, keyword: Span) -> PResult<NativeObjectDecl> {
        self.consume(TokenKind::Object, "Expect \"object\" after \"native\".")?;
        let name = self.consume_type_name("Expect native object type name.")?;
        if self.check(TokenKind::Less) {
            return Err(self.error_here("Native object declarations cannot have type parameters."));
        }
        if self.check(TokenKind::LeftBrace) {
            return Err(
                self.error_here("Native object declaration requires \";\" and cannot have a body.")
            );
        }
        let semicolon = self.consume(
            TokenKind::Semicolon,
            "Expect \";\" after native object declaration.",
        )?;
        Ok(NativeObjectDecl {
            name,
            span: keyword.to(semicolon.span),
        })
    }

    // ===== objects ===========================================================

    /// After `object`: `Name<T> does Roles { members }`.
    fn object_declaration(&mut self) -> PResult<ObjectDecl> {
        let keyword = self.previous().span;
        let name = self.consume_type_name("Expect object name.")?;
        let generics = self.generic_params("object", &name)?;
        if self.check(TokenKind::LeftParen) {
            return Err(self.error_here("Object headers do not accept parameters in v1.0."));
        }
        let mut roles = Vec::new();
        if self.eat(TokenKind::Does) {
            roles.push(self.type_expr()?);
            while self.eat(TokenKind::Comma) {
                roles.push(self.type_expr()?);
            }
        }
        let (members, close) = self.object_body(Some(&name))?;
        Ok(ObjectDecl {
            name,
            generics,
            roles,
            members,
            span: keyword.to(close),
        })
    }

    /// `{ members }` of an object (`owner` is set) or a role fake (`None`).
    /// Returns the members and the span of the closing brace.
    fn object_body(&mut self, owner: Option<&Name>) -> PResult<(Vec<ObjectMember>, Span)> {
        self.consume(TokenKind::LeftBrace, "Expect \"{\" before object body.")?;
        let mut members = Vec::new();
        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            if self.eat(TokenKind::Private) {
                members.push(ObjectMember::Private(self.private_section()?));
                break;
            }
            members.push(self.object_member(owner)?);
        }
        let close = self.consume(TokenKind::RightBrace, "Expect \"}\" after object body.")?;
        Ok((members, close.span))
    }

    /// After `private`: `: action*` up to the end of the body. `private` is a
    /// section marker, not a per-action modifier.
    fn private_section(&mut self) -> PResult<PrivateSection> {
        let keyword = self.previous().span;
        let colon = self.consume(TokenKind::Colon, "Expect \":\" after \"private\".")?;
        let mut actions = Vec::new();
        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            self.reject_native_outside_module()?;
            if self.eat(TokenKind::Amp) {
                let token = self.previous().clone();
                return Err(self.error_at(
                    &token,
                    "Object projection declarations use \"#projected\", not \"&\".",
                ));
            }
            let directive = if self.eat(TokenKind::Hash) {
                Some(self.projection_directive()?)
            } else {
                None
            };
            actions.push(self.action_declaration(Visibility::Private, directive)?);
        }
        let end = actions.last().map_or(colon.span, |action| action.span);
        Ok(PrivateSection {
            actions,
            span: keyword.to(end),
        })
    }

    fn object_member(&mut self, owner: Option<&Name>) -> PResult<ObjectMember> {
        if self.check_cap_declaration() {
            if owner.is_none() {
                return Err(self.error_here("Caps are only allowed inside objects."));
            }
            return Ok(ObjectMember::Cap(self.cap_declaration(false)?));
        }
        if self.check_view_declaration() {
            return Err(self.error_here("Object views have been removed."));
        }
        self.reject_native_outside_module()?;
        let token = self.peek().clone();
        self.reject_wither(&token)?;
        // `has(` is an action labelled `has`; `has name ...` declares state.
        if self.check(TokenKind::Has) && self.check_next(TokenKind::LeftParen) {
            return Ok(ObjectMember::Action(
                self.action_declaration(Visibility::Public, None)?,
            ));
        }
        if self.eat(TokenKind::Has) {
            return Ok(ObjectMember::Has(self.has_declaration()?));
        }
        if self.eat(TokenKind::Destroy) {
            let keyword = self.previous().span;
            let body = self.block()?;
            return Ok(ObjectMember::Destroy(DestroyDecl {
                span: keyword.to(body.span),
                body,
            }));
        }
        if self.eat(TokenKind::Test) {
            return Ok(ObjectMember::Test(self.test_block(false)?));
        }
        if self.check_async_test() {
            return Ok(ObjectMember::Test(self.async_test_block()?));
        }
        // Constructors are type-name-shaped members followed by `(`; action
        // names are lower-case.
        if self.check_type_name() && self.check_next(TokenKind::LeftParen) {
            return Ok(ObjectMember::Constructor(
                self.constructor_declaration(owner)?,
            ));
        }
        if self.eat(TokenKind::Hash) {
            let directive = self.projection_directive()?;
            return Ok(ObjectMember::Action(
                self.action_declaration(Visibility::Public, Some(directive))?,
            ));
        }
        if self.eat(TokenKind::Amp) {
            let token = self.previous().clone();
            return Err(self.error_at(
                &token,
                "Object projection declarations use \"#projected\", not \"&\".",
            ));
        }
        if self.check_async_modifier() || self.check_name() {
            return Ok(ObjectMember::Action(
                self.action_declaration(Visibility::Public, None)?,
            ));
        }
        let token = self.peek().clone();
        self.reject_reserved(&token)?;
        Err(self.error_here("Expect object member declaration."))
    }

    /// After `has`: `a, b is Type = default;`
    fn has_declaration(&mut self) -> PResult<HasDecl> {
        let keyword = self.previous().span;
        let mut names = vec![self.consume_identifier("Expect attribute name after \"has\".")?];
        while self.eat(TokenKind::Comma) {
            names.push(self.consume_identifier("Expect attribute name after \",\".")?);
        }
        let ty = if self.eat(TokenKind::Is) {
            Some(self.type_expr()?)
        } else {
            None
        };
        let default = if self.eat(TokenKind::Equal) {
            Some(self.expression()?)
        } else {
            None
        };
        let semicolon =
            self.consume(TokenKind::Semicolon, "Expect \";\" after has declaration.")?;
        Ok(HasDecl {
            names,
            ty,
            default,
            span: keyword.to(semicolon.span),
        })
    }

    /// `Name(params) { }`, `Name(:a, :b);`, `Name(:a) { }` or `Name();`.
    fn constructor_declaration(&mut self, owner: Option<&Name>) -> PResult<ConstructorDecl> {
        let name = self.consume_type_name("Expect constructor name.")?;
        if let Some(owner) = owner {
            if name.text != owner.text {
                return Err(
                    self.error_at_span(name.span, "Constructor name must match enclosing object.")
                );
            }
        }
        if self.check(TokenKind::LeftParen) && self.check_next(TokenKind::Colon) {
            return self.constructor_shorthand(name);
        }
        let params = self.parameter_list(ParamContext::Constructor)?;
        if params.is_empty() && self.check(TokenKind::Semicolon) {
            // `Name();` — the zero-field shorthand, distinct from `Name() {}`.
            let semicolon = self.advance();
            return Ok(ConstructorDecl {
                span: name.span.to(semicolon.span),
                name,
                form: ConstructorForm::Shorthand {
                    fields: Vec::new(),
                    body: None,
                },
            });
        }
        let body = self.block()?;
        Ok(ConstructorDecl {
            span: name.span.to(body.span),
            name,
            form: ConstructorForm::Full { params, body },
        })
    }

    /// `Name(:a, :b,)` followed by `;` or a block.
    ///
    /// A trailing comma is accepted because the grammar lists
    /// `field_param_list = field_param ("," field_param)* trailing_comma?`
    /// (conformance case `L01.gram.constructor.shorthand-trailing-comma`);
    /// the reference parser rejects it, which is recorded there as defect
    /// DEF-001.
    fn constructor_shorthand(&mut self, name: Name) -> PResult<ConstructorDecl> {
        self.consume(
            TokenKind::LeftParen,
            "Expect \"(\" before constructor shorthand fields.",
        )?;
        let mut fields = Vec::new();
        loop {
            if self.check_name() && self.check_next(TokenKind::Is) {
                return Err(self.error_here(
                    "Constructor shorthand cannot mix field entries and typed parameters.",
                ));
            }
            self.consume(
                TokenKind::Colon,
                "Expect \":\" before constructor shorthand field.",
            )?;
            fields.push(self.consume_identifier("Expect field name after \":\".")?);
            if !self.eat(TokenKind::Comma) || self.check(TokenKind::RightParen) {
                break;
            }
        }
        self.consume(
            TokenKind::RightParen,
            "Expect \")\" after constructor shorthand fields.",
        )?;
        if self.check(TokenKind::LeftBrace) {
            let body = self.block()?;
            return Ok(ConstructorDecl {
                span: name.span.to(body.span),
                name,
                form: ConstructorForm::Shorthand {
                    fields,
                    body: Some(body),
                },
            });
        }
        let semicolon = self.consume(
            TokenKind::Semicolon,
            "Expect \";\" after constructor shorthand.",
        )?;
        Ok(ConstructorDecl {
            span: name.span.to(semicolon.span),
            name,
            form: ConstructorForm::Shorthand { fields, body: None },
        })
    }

    /// After `#`: `projected` or `projected(alias)`.
    fn projection_directive(&mut self) -> PResult<ProjectionDirective> {
        let marker = self.previous().span;
        let name = self.consume_name("Expect \"projected\" after \"#\".")?;
        if name.text != "projected" {
            return Err(self.error_at_span(
                name.span,
                &format!("Unknown object action directive \"{}\".", name.text),
            ));
        }
        let mut end = name.span;
        let mut alias = None;
        if self.eat(TokenKind::LeftParen) {
            if !self.check(TokenKind::Identifier) || !is_lower_word(self.lexeme(self.peek())) {
                return Err(self.error_here("Projection alias must be a lowercase identifier."));
            }
            let token = self.advance();
            alias = Some(self.name_of(&token));
            end = self
                .consume(
                    TokenKind::RightParen,
                    "Expect \")\" after projection alias.",
                )?
                .span;
        }
        Ok(ProjectionDirective {
            alias,
            span: marker.to(end),
        })
    }

    /// `async? name(params) is Result { body }`
    fn action_declaration(
        &mut self,
        visibility: Visibility,
        projection: Option<ProjectionDirective>,
    ) -> PResult<ActionDecl> {
        let start = projection
            .as_ref()
            .map_or(self.peek().span, |directive| directive.span);
        let is_async = if self.check_async_modifier() {
            self.advance();
            true
        } else {
            false
        };
        let name = self.consume_action_name("Expect action name.")?;
        let params = self.parameter_list(ParamContext::Action)?;
        self.consume(
            TokenKind::Is,
            "Action requires an explicit result: \"is <type>\" or \"is void\".",
        )?;
        let result = self.result_spec()?;
        let body = self.block()?;
        Ok(ActionDecl {
            is_async,
            visibility,
            projection,
            name,
            params,
            result,
            span: start.to(body.span),
            body,
        })
    }

    /// A role signature: `async? name(params) is Result;`
    fn action_signature(&mut self) -> PResult<ActionSignature> {
        let start = self.peek().span;
        let is_async = if self.check_async_modifier() {
            self.advance();
            true
        } else {
            false
        };
        let name = self.consume_action_name("Expect action name.")?;
        let params = self.parameter_list(ParamContext::Signature)?;
        self.consume(
            TokenKind::Is,
            "Action requires an explicit result: \"is <type>\" or \"is void\".",
        )?;
        let result = self.result_spec()?;
        let semicolon =
            self.consume(TokenKind::Semicolon, "Expect \";\" after action signature.")?;
        Ok(ActionSignature {
            is_async,
            name,
            params,
            result,
            span: start.to(semicolon.span),
        })
    }

    /// `(param ("," param)* ","?)`
    fn parameter_list(&mut self, context: ParamContext) -> PResult<Vec<Param>> {
        let description = context.description();
        self.consume(
            TokenKind::LeftParen,
            &format!("Expect \"(\" before {description}."),
        )?;
        let mut params = Vec::new();
        if !self.check(TokenKind::RightParen) {
            loop {
                if context == ParamContext::Constructor && self.check(TokenKind::Colon) {
                    return Err(self.error_here(
                        "Constructor shorthand cannot mix field entries and typed parameters.",
                    ));
                }
                params.push(self.parameter()?);
                if context == ParamContext::Native && self.check(TokenKind::Equal) {
                    return Err(self.error_here("Native parameters cannot have default values."));
                }
                if !self.eat(TokenKind::Comma) || self.check(TokenKind::RightParen) {
                    break;
                }
            }
        }
        // Variadic placement is checked before the closing parenthesis, in
        // the same order as the reference parser.
        if let Some(second) = params.iter().filter(|param| param.variadic).nth(1) {
            return Err(self.error_at_span(
                second.name.span,
                "Action may declare only one variadic parameter.",
            ));
        }
        if let Some(misplaced) = params
            .iter()
            .take(params.len().saturating_sub(1))
            .find(|param| param.variadic)
        {
            return Err(self.error_at_span(
                misplaced.name.span,
                &format!(
                    "Variadic parameter \"{}\" must be the last parameter.",
                    misplaced.name.text
                ),
            ));
        }
        self.consume(
            TokenKind::RightParen,
            &format!("Expect \")\" after {description}."),
        )?;
        Ok(params)
    }

    /// `name is ...? Type`
    pub(super) fn parameter(&mut self) -> PResult<Param> {
        let name = self.consume_identifier("Expect parameter name.")?;
        self.consume(TokenKind::Is, "Expect \"is\" after parameter name.")?;
        let variadic = self.eat(TokenKind::Ellipsis);
        if variadic
            && !self.check(TokenKind::At)
            && !self.check(TokenKind::Percent)
            && !self.check_name()
        {
            let ellipsis = self.previous().clone();
            return Err(self.error_at(&ellipsis, "Expect element type after \"...\"."));
        }
        let ty = self.type_expr()?;
        if variadic && self.check(TokenKind::Equal) {
            return Err(self.error_here(format!(
                "Variadic parameter \"{}\" cannot have a default value.",
                name.text
            )));
        }
        Ok(Param {
            span: name.span.to(ty.span),
            name,
            variadic,
            ty,
        })
    }

    // ===== caps ==============================================================

    /// `cap Name { fields }` (in an object) or `cap Owner::Name { fields }`
    /// (at module scope, `external`).
    fn cap_declaration(&mut self, external: bool) -> PResult<CapDecl> {
        let keyword = self.advance().span;
        let owner = if external {
            let owner = self.consume_name("Expect owner type before external cap name.")?;
            self.consume(
                TokenKind::DoubleColon,
                "A module-level cap requires Owner::Name.",
            )?;
            Some(owner)
        } else {
            None
        };
        let name = self.consume_name("Expect cap name.")?;
        self.consume(
            TokenKind::LeftBrace,
            "A cap contains only synchronous computed fields; expect \"{\".",
        )?;
        let previous_owner = std::mem::replace(
            &mut self.external_cap_owner,
            owner.as_ref().map(|owner| owner.text.clone()),
        );
        let fields = self.cap_fields();
        self.external_cap_owner = previous_owner;
        let (fields, close) = fields?;
        Ok(CapDecl {
            owner,
            name,
            fields,
            span: keyword.to(close),
        })
    }

    fn cap_fields(&mut self) -> PResult<(Vec<ComputedField>, Span)> {
        let mut fields = Vec::new();
        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            let name =
                self.consume_binding_name("A cap contains only synchronous computed fields.")?;
            self.consume(
                TokenKind::Is,
                "A cap field uses \"name is Type { body }\"; methods and declarations are not allowed.",
            )?;
            if self.check(TokenKind::Caret) {
                return Err(self.error_here("A computed field cannot declare a successor result."));
            }
            let ty = self.type_expr()?;
            let body = self.block()?;
            fields.push(ComputedField {
                span: name.span.to(body.span),
                name,
                ty,
                body,
            });
        }
        let close = self.consume(TokenKind::RightBrace, "Expect \"}\" after cap.")?;
        Ok((fields, close.span))
    }

    // ===== roles =============================================================

    /// After `role`: `Name<T> { members }`.
    fn role_declaration(&mut self) -> PResult<RoleDecl> {
        let keyword = self.previous().span;
        let name = self.consume_type_name("Expect role name.")?;
        let generics = self.generic_params("role", &name)?;
        if generics.iter().any(|param| param.bound.is_some()) {
            return Err(self.error_at_span(
                name.span,
                "Generic bounds are currently supported only on object declarations.",
            ));
        }
        self.consume(TokenKind::LeftBrace, "Expect \"{\" before role body.")?;
        let mut members = Vec::new();
        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            members.push(self.role_member()?);
        }
        let close = self.consume(TokenKind::RightBrace, "Expect \"}\" after role body.")?;
        Ok(RoleDecl {
            name,
            generics,
            members,
            span: keyword.to(close.span),
        })
    }

    fn role_member(&mut self) -> PResult<RoleMember> {
        if self.check_cap_declaration() {
            return Err(self.error_here("Caps are only allowed inside objects."));
        }
        if self.check_view_declaration() {
            return Err(self.error_here("Object views have been removed."));
        }
        self.reject_native_outside_module()?;
        let token = self.peek().clone();
        self.reject_wither(&token)?;
        if self.eat(TokenKind::Fake) {
            return Ok(RoleMember::Fake(self.fake_block()?));
        }
        if self.eat(TokenKind::Test) {
            return Ok(RoleMember::Test(self.test_block(false)?));
        }
        if self.check_async_test() {
            return Ok(RoleMember::Test(self.async_test_block()?));
        }
        if self.check_async_modifier() || self.check_name() {
            return Ok(RoleMember::Signature(self.action_signature()?));
        }
        let token = self.peek().clone();
        self.reject_reserved(&token)?;
        Err(self.error_here("Expect role member declaration."))
    }

    /// After `fake`: an object body without constructor-name checks.
    fn fake_block(&mut self) -> PResult<FakeDecl> {
        let keyword = self.previous().span;
        if self.mode == ParseMode::Production {
            let body = self.skip_braced_body("fake block")?;
            return Ok(FakeDecl {
                body: FakeBody::Skipped(body),
                span: keyword.to(body),
            });
        }
        let (members, close) = self.object_body(None)?;
        Ok(FakeDecl {
            body: FakeBody::Parsed(members),
            span: keyword.to(close),
        })
    }

    // ===== enums and tests ===================================================

    /// After `enum`: `Name = (a, b);`
    fn enum_declaration(&mut self) -> PResult<EnumDecl> {
        let keyword = self.previous().span;
        let name = self.consume_type_name("Expect enum name after \"enum\".")?;
        self.consume(TokenKind::Equal, "Expect \"=\" after enum name.")?;
        self.consume(TokenKind::LeftParen, "Expect \"(\" before enum cases.")?;
        let mut cases = Vec::new();
        if !self.check(TokenKind::RightParen) {
            loop {
                cases.push(self.consume_identifier("Expect enum case name.")?);
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenKind::RightParen, "Expect \")\" after enum cases.")?;
        let semicolon =
            self.consume(TokenKind::Semicolon, "Expect \";\" after enum declaration.")?;
        Ok(EnumDecl {
            name,
            cases,
            span: keyword.to(semicolon.span),
        })
    }

    fn async_test_block(&mut self) -> PResult<TestDecl> {
        let async_keyword = self.advance().span;
        self.consume(TokenKind::Test, "Expect \"test\" after \"async\".")?;
        let mut test = self.test_block(true)?;
        test.span = async_keyword.to(test.span);
        Ok(test)
    }

    /// After `test`: `"name" { body }`.
    fn test_block(&mut self, is_async: bool) -> PResult<TestDecl> {
        let keyword = self.previous().span;
        let name_token = self.consume(TokenKind::String, "Expect test name after \"test\".")?;
        let name = self.string_lit(&name_token);
        let body = if self.mode == ParseMode::Production {
            TestBody::Skipped(self.skip_braced_body("test block")?)
        } else {
            TestBody::Parsed(self.block()?)
        };
        let end = match &body {
            TestBody::Parsed(block) => block.span,
            TestBody::Skipped(span) => *span,
        };
        Ok(TestDecl {
            is_async,
            name,
            body,
            span: keyword.to(end),
        })
    }

    /// Production mode consumes test and fake bodies as balanced braces
    /// without parsing their inner grammar (spec §14). Lexical errors were
    /// already reported for the whole file, so they still apply.
    fn skip_braced_body(&mut self, context: &str) -> PResult<Span> {
        let open = self.consume(
            TokenKind::LeftBrace,
            &format!("Expect \"{{\" before {context}."),
        )?;
        let mut depth = 1usize;
        loop {
            if self.is_at_end() {
                return Err(self.error_here(format!("Expect \"}}\" after {context}.")));
            }
            let token = self.advance();
            match token.kind {
                TokenKind::LeftBrace => depth += 1,
                TokenKind::RightBrace => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(open.span.to(token.span));
                    }
                }
                _ => {}
            }
        }
    }
}
