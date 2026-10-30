//! Индекс корневого файла: обход THIR (локальные, поля, вызовы,
//! конструкторы, natives, варианты enum) и токенов (имена типов в
//! сигнатурах, аннотациях и импортах). Кроме символов индекс собирает
//! данные дополнения: члены каждого встреченного типа, области видимости
//! тел и имена области модуля.

use std::collections::{HashMap, HashSet};

use super::{
    Completion, CompletionKind, LineIndex, Location, Scope, Symbol, builtin_members, doc_comment,
    hover_text,
};
use crate::analysis::registry::{CtorShape, Registry, TypeRef};
use crate::analysis::thir::{self, Body, BodyOwner, Callee, ExprKind, StmtKind};
use crate::analysis::ty::{EnumId, ObjectId, RoleId, Ty};
use crate::source::{SourceFile, Span};
use crate::syntax::{ast, lexer, token::TokenKind};

/// Индекс корня: символы, члены типов, области видимости, имена модуля.
#[derive(Debug, Default)]
pub struct Index {
    pub symbols: Vec<Symbol>,
    pub members: HashMap<String, Vec<Completion>>,
    pub scopes: Vec<Scope>,
    pub globals: Vec<Completion>,
}

/// Индекс модуля `root` (индекс в `registry.modules` и `files`).
pub fn build(registry: &Registry<'_>, bodies: &[Body], files: &[SourceFile], root: usize) -> Index {
    let mut builder = Builder {
        registry,
        files,
        root,
        lines: HashMap::new(),
        symbols: Vec::new(),
        members: HashMap::new(),
        scopes: Vec::new(),
    };
    for body in bodies.iter().filter(|body| body.module == root) {
        builder.body(body, owner_object(&body.owner));
    }
    builder.declarations();
    builder.tokens();
    let globals = builder.globals();
    Index {
        symbols: builder.symbols,
        members: builder.members,
        scopes: builder.scopes,
        globals,
    }
}

fn owner_object(owner: &BodyOwner) -> Option<ObjectId> {
    match owner {
        BodyOwner::Method { object, .. }
        | BodyOwner::Ctor { object, .. }
        | BodyOwner::Default { object, .. }
        | BodyOwner::Destroy { object }
        | BodyOwner::CapField { object, .. } => Some(*object),
        BodyOwner::Entry | BodyOwner::Closure | BodyOwner::Test(_) => None,
    }
}

/// Вариант-вызов: snippet с параметрами.
fn completion_call(
    name: &str,
    kind: CompletionKind,
    params: &[String],
    detail: String,
    doc: String,
) -> Completion {
    let args: Vec<String> = params
        .iter()
        .enumerate()
        .map(|(index, param)| format!("${{{}:{param}}}", index + 1))
        .collect();
    Completion {
        label: name.to_string(),
        kind,
        detail,
        documentation: doc,
        insert: format!("{name}({})", args.join(", ")),
        snippet: true,
    }
}

struct Builder<'r, 'a> {
    registry: &'r Registry<'a>,
    files: &'r [SourceFile],
    root: usize,
    lines: HashMap<usize, LineIndex<'r>>,
    symbols: Vec<Symbol>,
    members: HashMap<String, Vec<Completion>>,
    scopes: Vec<Scope>,
}

impl<'r, 'a> Builder<'r, 'a> {
    fn text(&self) -> &'r str {
        self.files[self.root].text()
    }

    fn location(&mut self, module: usize, span: Span) -> Option<Location> {
        let file = self.files.get(module)?;
        let lines = self
            .lines
            .entry(module)
            .or_insert_with(|| LineIndex::new(file.text()));
        Some(Location {
            path: file.name().to_string(),
            range: lines.range(span),
        })
    }

    fn doc(&self, module: usize, span: Span) -> String {
        self.files
            .get(module)
            .map(|file| doc_comment(file, span.start))
            .unwrap_or_default()
    }

    fn push(&mut self, span: Span, hover: String, definition: Option<Location>) {
        self.push_typed(span, hover, definition, None);
    }

    /// Символ со значением типа `ty`: после `.` дополняются члены этого типа.
    fn push_typed(
        &mut self,
        span: Span,
        hover: String,
        definition: Option<Location>,
        ty: Option<&Ty>,
    ) {
        if span.is_empty() || span.end as usize > self.text().len() {
            return;
        }
        let ty = ty.and_then(|ty| self.members_of(ty));
        self.symbols.push(Symbol {
            span,
            hover,
            definition,
            ty,
            local: false,
        });
    }

    /// Символ локальной: переименование меняет все её вхождения в файле.
    fn push_local(&mut self, span: Span, hover: String, definition: Option<Location>, ty: &Ty) {
        self.push_typed(span, hover, definition, Some(ty));
        if let Some(symbol) = self.symbols.last_mut() {
            if symbol.span == span {
                symbol.local = true;
            }
        }
    }

    fn ty(&self, ty: &Ty) -> String {
        self.registry.type_name(ty)
    }

    /// Параметры подписи `name is T`; variadic-хвост — как в исходнике, `...T`.
    fn params(&self, params: &[(String, Ty)], variadic: bool) -> Vec<String> {
        params
            .iter()
            .enumerate()
            .map(|(index, (name, ty))| match ty {
                Ty::Array(element) if variadic && index + 1 == params.len() => {
                    format!("{name} is ...{}", self.ty(element))
                }
                _ => format!("{name} is {}", self.ty(ty)),
            })
            .collect()
    }

    // ---- члены типов (дополнение) ----

    /// Ключ членов типа; члены вычисляются один раз.
    fn members_of(&mut self, ty: &Ty) -> Option<String> {
        let ty = match ty {
            Ty::Successor(inner) => inner,
            other => other,
        };
        if matches!(ty, Ty::Error | Ty::Never | Ty::Test) {
            return None;
        }
        let key = self.ty(ty);
        if !self.members.contains_key(&key) {
            self.members.insert(key.clone(), Vec::new());
            let found = self.compute_members(ty, &key);
            self.members.insert(key.clone(), found);
        }
        Some(key)
    }

    fn compute_members(&mut self, ty: &Ty, key: &str) -> Vec<Completion> {
        match ty {
            Ty::Object(id, _) => self.object_members(*id, false),
            Ty::Role(id, _) => self.role_members(*id),
            Ty::Param(param) => match self.registry.params[param.0].bound.clone() {
                Some(bound) => self
                    .members_of(&bound)
                    .and_then(|bound| self.members.get(&bound).cloned())
                    .unwrap_or_default(),
                None => Vec::new(),
            },
            _ => builtin_members(key),
        }
    }

    fn method_completion(&self, object: ObjectId, method: usize) -> Completion {
        let info = &self.registry.objects[object.0];
        let found = &info.methods[method];
        let params = self.params(&found.params, found.variadic);
        let result = if found.successor {
            format!("^{}", self.registry.result_name(&found.result))
        } else {
            self.registry.result_name(&found.result)
        };
        let doc = found
            .decl
            .map(|decl| self.doc(info.module, decl.span))
            .unwrap_or_default();
        let names: Vec<String> = found.params.iter().map(|(name, _)| name.clone()).collect();
        completion_call(
            &found.name,
            CompletionKind::Method,
            &names,
            format!(
                "{}({}) is {result}",
                if found.is_async { "async " } else { "" },
                params.join(", ")
            ),
            doc,
        )
    }

    /// Действия объекта; `inside` — для `self`: с приватными действиями и полями.
    fn object_members(&self, object: ObjectId, inside: bool) -> Vec<Completion> {
        let info = &self.registry.objects[object.0];
        let mut out: Vec<Completion> = (0..info.methods.len())
            .filter(|index| {
                let method = &info.methods[*index];
                method.decl.is_some() && (inside || !method.private)
            })
            .map(|index| self.method_completion(object, index))
            .collect();
        if inside {
            for field in &info.fields {
                out.push(Completion {
                    label: field.name.clone(),
                    kind: CompletionKind::Field,
                    detail: self.ty(&field.ty),
                    documentation: format!("field of `{}`", info.name),
                    insert: field.name.clone(),
                    snippet: false,
                });
            }
        }
        out
    }

    fn role_members(&self, role: RoleId) -> Vec<Completion> {
        let info = &self.registry.roles[role.0];
        info.methods
            .iter()
            .map(|method| {
                let params = self.params(&method.params, method.variadic);
                let doc = info
                    .decl
                    .and_then(|decl| {
                        decl.members.iter().find_map(|member| match member {
                            ast::RoleMember::Signature(signature)
                                if signature.name.text == method.name =>
                            {
                                Some(self.doc(info.module, signature.span))
                            }
                            _ => None,
                        })
                    })
                    .unwrap_or_default();
                let names: Vec<String> =
                    method.params.iter().map(|(name, _)| name.clone()).collect();
                completion_call(
                    &method.name,
                    CompletionKind::Method,
                    &names,
                    format!(
                        "{}({}) is {}",
                        if method.is_async { "async " } else { "" },
                        params.join(", "),
                        self.registry.result_name(&method.result)
                    ),
                    doc,
                )
            })
            .collect()
    }

    /// Имя типа модуля как вариант дополнения.
    fn type_completion(&self, name: &str, found: TypeRef) -> Completion {
        let (kind, module, span, detail) = match found {
            TypeRef::Object(id) => {
                let info = &self.registry.objects[id.0];
                (
                    CompletionKind::Object,
                    info.module,
                    info.decl.map(|decl| decl.span),
                    "object",
                )
            }
            TypeRef::Role(id) => {
                let info = &self.registry.roles[id.0];
                (
                    CompletionKind::Role,
                    info.module,
                    info.decl.map(|decl| decl.span),
                    "role",
                )
            }
            TypeRef::Enum(id) => {
                let info = &self.registry.enums[id.0];
                (CompletionKind::Enum, info.module, Some(info.span), "enum")
            }
            TypeRef::Handle(id) => {
                let info = &self.registry.handles[id.0];
                (
                    CompletionKind::Object,
                    info.module,
                    Some(info.span),
                    "native object",
                )
            }
        };
        Completion {
            label: name.to_string(),
            kind,
            detail: detail.to_string(),
            documentation: span.map(|span| self.doc(module, span)).unwrap_or_default(),
            insert: name.to_string(),
            snippet: false,
        }
    }

    fn native_completion(&self, native: usize) -> Completion {
        let info = &self.registry.natives[native];
        let detail = super::outline::signature_text(&info.decl.params, &info.decl.result);
        let names: Vec<String> = info.params.iter().map(|(name, _)| name.clone()).collect();
        completion_call(
            &info.name,
            CompletionKind::Function,
            &names,
            detail,
            self.doc(info.module, info.decl.span),
        )
    }

    /// Собственные имена модуля `module` (для `take` и `alias.`); `only` — взятые.
    fn module_names(&self, module: usize, only: Option<&Vec<String>>) -> Vec<Completion> {
        let names = &self.registry.modules[module].own;
        let visible = |name: &String| only.is_none_or(|only| only.contains(name));
        let mut out: Vec<Completion> = names
            .types
            .iter()
            .filter(|(name, _)| visible(name))
            .map(|(name, found)| self.type_completion(name, *found))
            .collect();
        out.extend(
            names
                .natives
                .iter()
                .filter(|(name, _)| visible(name))
                .map(|(_, native)| self.native_completion(*native)),
        );
        out.sort_by(|a, b| a.label.cmp(&b.label));
        out
    }

    /// Имена области модуля; заодно `static:Enum`, `namespace:alias` и `module:path`.
    fn globals(&mut self) -> Vec<Completion> {
        let module = &self.registry.modules[self.root];
        let mut out: Vec<Completion> = module
            .scope
            .types
            .iter()
            .map(|(name, found)| self.type_completion(name, *found))
            .collect();
        out.extend(
            module
                .scope
                .natives
                .values()
                .map(|native| self.native_completion(*native)),
        );
        for (name, found) in &module.scope.types {
            if let TypeRef::Object(id) = found {
                let info = &self.registry.objects[id.0];
                let ctors = (0..info.ctors.len())
                    .map(|ctor| {
                        let hover = self.ctor_hover(*id, ctor);
                        let signature = hover
                            .strip_prefix("```okkama\n")
                            .and_then(|rest| rest.lines().next())
                            .unwrap_or(name)
                            .to_string();
                        Completion {
                            label: name.clone(),
                            kind: CompletionKind::Object,
                            detail: signature,
                            documentation: String::new(),
                            insert: name.clone(),
                            snippet: false,
                        }
                    })
                    .collect();
                self.members.insert(format!("ctor:{name}"), ctors);
            }
            if let TypeRef::Enum(id) = found {
                let info = &self.registry.enums[id.0];
                let cases = info
                    .cases
                    .iter()
                    .map(|case| Completion {
                        label: case.clone(),
                        kind: CompletionKind::EnumMember,
                        detail: format!("{}.{case}", info.name),
                        documentation: String::new(),
                        insert: case.clone(),
                        snippet: false,
                    })
                    .collect();
                self.members.insert(format!("static:{name}"), cases);
            }
        }
        for (alias, namespace) in &module.namespaces {
            let names = self.module_names(namespace.module, namespace.only.as_ref());
            self.members.insert(format!("namespace:{alias}"), names);
            out.push(Completion {
                label: alias.clone(),
                kind: CompletionKind::Module,
                detail: format!(
                    "import \"{}\"",
                    self.registry.modules[namespace.module].name
                ),
                documentation: String::new(),
                insert: alias.clone(),
                snippet: false,
            });
        }
        for (dependency, import) in &module.imports {
            let names = self.module_names(*dependency, None);
            self.members
                .insert(format!("module:{}", import.path.literal.value), names);
        }
        out.sort_by(|a, b| a.label.cmp(&b.label));
        out
    }

    // ---- тела ----

    fn body(&mut self, body: &Body, object: Option<ObjectId>) {
        let mut scope = Scope {
            span: body.span,
            locals: Vec::new(),
            object: None,
        };
        if let Some(object) = object {
            let key = format!("self:{}", self.registry.objects[object.0].name);
            if !self.members.contains_key(&key) {
                let found = self.object_members(object, true);
                self.members.insert(key.clone(), found);
            }
            scope.object = Some(key);
        }
        for (index, local) in body.locals.iter().enumerate() {
            if local.span.is_empty() || !self.names_at(local.span, &local.name) {
                continue;
            }
            scope
                .locals
                .push((local.name.clone(), self.ty(&local.ty), local.span.start));
            let hover = self.local_hover(body, index);
            let definition = self.location(self.root, local.span);
            self.push_local(local.span, hover, definition, &local.ty);
        }
        self.scopes.push(scope);
        self.block(&body.block, body, object);
    }

    /// Текст корня в `span` — это имя `name`.
    fn names_at(&self, span: Span, name: &str) -> bool {
        self.text().get(span.range()) == Some(name)
    }

    fn local_hover(&self, body: &Body, index: usize) -> String {
        let local = &body.locals[index];
        let parameter = body.params.contains(&thir::LocalId(index));
        let signature = if parameter {
            format!("{} is {}", local.name, self.ty(&local.ty))
        } else {
            format!("let {} is {}", local.name, self.ty(&local.ty))
        };
        hover_text(&signature, if parameter { "parameter" } else { "" })
    }

    fn block(&mut self, block: &thir::Block, body: &Body, object: Option<ObjectId>) {
        for stmt in &block.stmts {
            self.stmt(stmt, body, object);
        }
    }

    fn stmt(&mut self, stmt: &thir::Stmt, body: &Body, object: Option<ObjectId>) {
        match &stmt.kind {
            StmtKind::Let { value, .. } => self.expr(value, body, object),
            StmtKind::Expr(value) | StmtKind::Throw(value) | StmtKind::Defer(value) => {
                self.expr(value, body, object)
            }
            StmtKind::Guarded { condition, stmt } => {
                self.expr(condition, body, object);
                self.stmt(stmt, body, object);
            }
            StmtKind::Return(value) | StmtKind::Last(value) => {
                if let Some(value) = value {
                    self.expr(value, body, object);
                }
            }
            StmtKind::Confirm {
                condition,
                fallback,
            } => {
                self.expr(condition, body, object);
                match fallback {
                    Some(thir::Fallback::Throw(value)) | Some(thir::Fallback::Panic(value)) => {
                        self.expr(value, body, object)
                    }
                    None => {}
                }
            }
            StmtKind::BindField { field, value } | StmtKind::StageField { field, value } => {
                if let Some(object) = object {
                    let name = self.registry.objects[object.0].fields[*field].name.clone();
                    if let Some(span) = self.field_target(stmt.span, &name) {
                        self.field(span, object, *field);
                    }
                }
                self.expr(value, body, object);
            }
            StmtKind::Rebind { local, value } => {
                let name = &body.locals[local.0].name;
                let span = Span::new(stmt.span.start, stmt.span.start + name.len() as u32);
                if self.names_at(span, name) {
                    let hover = self.local_hover(body, local.0);
                    let definition = self.location(self.root, body.locals[local.0].span);
                    let ty = body.locals[local.0].ty.clone();
                    self.push_local(span, hover, definition, &ty);
                }
                self.expr(value, body, object);
            }
            StmtKind::Rethrow | StmtKind::Next => {}
        }
    }

    /// Имя поля в `field = v`, `self.field = v` или `^field = v`.
    fn field_target(&self, stmt: Span, name: &str) -> Option<Span> {
        let text = self.text().get(stmt.range())?;
        let skipped = if text.starts_with("self.") {
            5
        } else if text.starts_with('^') {
            1
        } else {
            0
        };
        text[skipped..].starts_with(name).then(|| {
            Span::new(
                stmt.start + skipped as u32,
                stmt.start + (skipped + name.len()) as u32,
            )
        })
    }

    fn exprs(&mut self, values: &[thir::Expr], body: &Body, object: Option<ObjectId>) {
        for value in values {
            self.expr(value, body, object);
        }
    }

    fn source(&mut self, source: &thir::Source, body: &Body, object: Option<ObjectId>) {
        match source {
            thir::Source::Range { start, end, .. } => {
                self.expr(start, body, object);
                self.expr(end, body, object);
            }
            thir::Source::Array(value)
            | thir::Source::Map(value)
            | thir::Source::Iterating(value) => self.expr(value, body, object),
            thir::Source::Varargs(_) => {}
        }
    }

    fn expr(&mut self, expr: &thir::Expr, body: &Body, object: Option<ObjectId>) {
        match &expr.kind {
            ExprKind::Constant(_) | ExprKind::TestCapability | ExprKind::Unreachable => {}
            ExprKind::SelfValue => {
                if let Some(object) = object {
                    let name = self.registry.objects[object.0].name.clone();
                    self.push(expr.span, hover_text(&format!("self is {name}"), ""), None);
                }
            }
            ExprKind::Local(local) => {
                let decl = &body.locals[local.0];
                if self.names_at(expr.span, &decl.name) {
                    let hover = self.local_hover(body, local.0);
                    let definition = if decl.span.is_empty() {
                        None
                    } else {
                        self.location(self.root, decl.span)
                    };
                    let ty = decl.ty.clone();
                    self.push_local(expr.span, hover, definition, &ty);
                }
            }
            ExprKind::Field(field)
            | ExprKind::PendingField(field)
            | ExprKind::OwnerField(field) => {
                if let Some(object) = object {
                    self.field(expr.span, object, *field);
                }
            }
            ExprKind::EnumCase { ty, case } => self.enum_case(expr.span, ty.0, *case),
            ExprKind::Unary { operand, .. } => self.expr(operand, body, object),
            ExprKind::Binary { left, right, .. } | ExprKind::Logical { left, right, .. } => {
                self.expr(left, body, object);
                self.expr(right, body, object);
            }
            ExprKind::Coerce { value, .. }
            | ExprKind::Cast { value, .. }
            | ExprKind::Panic(value)
            | ExprKind::Unpack(value)
            | ExprKind::Await(value)
            | ExprKind::Spawn(value) => self.expr(value, body, object),
            ExprKind::RatNew {
                numerator,
                denominator,
            } => {
                self.expr(numerator, body, object);
                self.expr(denominator, body, object);
            }
            ExprKind::If {
                condition,
                then_block,
                else_block,
            } => {
                self.expr(condition, body, object);
                self.block(then_block, body, object);
                if let Some(block) = else_block {
                    self.block(block, body, object);
                }
            }
            ExprKind::Match(found) => {
                self.expr(&found.subject, body, object);
                for (patterns, block) in &found.cases {
                    self.exprs(patterns, body, object);
                    self.block(block, body, object);
                }
                if let Some(block) = &found.else_block {
                    self.block(block, body, object);
                }
            }
            ExprKind::Loop(block) | ExprKind::Block(block) => self.block(block, body, object),
            ExprKind::For {
                source,
                body: block,
                ..
            } => {
                self.source(source, body, object);
                self.block(block, body, object);
            }
            ExprKind::Try(found) => {
                self.block(&found.body, body, object);
                for catch in &found.catches {
                    self.block(&catch.body, body, object);
                }
            }
            ExprKind::Call { callee, args } => {
                self.callee(callee, expr.span, body, object);
                self.exprs(args, body, object);
            }
            ExprKind::Array(parts) => {
                for part in parts {
                    match part {
                        thir::ArrayPart::Value(value) => self.expr(value, body, object),
                        thir::ArrayPart::Range { start, end, .. } => {
                            self.expr(start, body, object);
                            self.expr(end, body, object);
                        }
                    }
                }
            }
            ExprKind::Map(pairs) => {
                for (key, value) in pairs {
                    self.expr(key, body, object);
                    self.expr(value, body, object);
                }
            }
            ExprKind::Range { start, end, .. } => {
                self.expr(start, body, object);
                self.expr(end, body, object);
            }
            ExprKind::Index {
                object: indexed,
                index,
                fallback,
            } => {
                self.expr(indexed, body, object);
                self.expr(index, body, object);
                if let Some(fallback) = fallback {
                    self.expr(fallback, body, object);
                }
            }
            ExprKind::Functional(functional) => {
                self.source(&functional.source, body, object);
                if let Some(initial) = &functional.initial {
                    self.expr(initial, body, object);
                }
                self.block(&functional.body, body, object);
            }
            ExprKind::ArrayCast { parts, .. } => {
                for part in parts {
                    match part {
                        thir::CastPart::One(value) | thir::CastPart::Spread(value) => {
                            self.expr(value, body, object)
                        }
                    }
                }
            }
            ExprKind::Projection { construction, .. } => self.expr(construction, body, object),
            ExprKind::Pack { items, spread } => {
                self.exprs(items, body, object);
                if let Some(spread) = spread {
                    self.expr(spread, body, object);
                }
            }
            ExprKind::Closure { body: inner, .. } => self.body(inner, object),
            ExprKind::BoundRoleAction {
                receiver,
                role,
                slot,
            } => {
                self.expr(receiver, body, object);
                self.role_member(receiver.span.end, expr.span.end, *role, *slot);
            }
            ExprKind::BoundAction {
                receiver,
                object: target,
                method,
            } => {
                self.expr(receiver, body, object);
                self.method_member(receiver.span.end, expr.span.end, *target, *method);
            }
            ExprKind::CallValue { callee, args } => {
                self.expr(callee, body, object);
                self.exprs(args, body, object);
            }
            ExprKind::StreamWrite { sink, source, .. } => {
                self.expr(sink, body, object);
                self.expr(source, body, object);
            }
            ExprKind::CapSelect { owner, .. } => self.expr(owner, body, object),
            ExprKind::CapRead { cap, .. } => self.expr(cap, body, object),
        }
    }

    fn callee(&mut self, callee: &Callee, span: Span, body: &Body, object: Option<ObjectId>) {
        match callee {
            Callee::Method {
                receiver,
                object: target,
                method,
            } => {
                self.expr(receiver, body, object);
                self.method_member(receiver.span.end, span.end, *target, *method);
            }
            Callee::Role {
                receiver,
                role,
                slot,
            } => {
                self.expr(receiver, body, object);
                self.role_member(receiver.span.end, span.end, *role, *slot);
            }
            Callee::Builtin { receiver, .. } => self.expr(receiver, body, object),
            Callee::Ctor {
                object: target,
                ctor,
                type_args,
                ..
            } => {
                let name = self.registry.objects[target.0].name.clone();
                if let Some(at) = self.name_before_paren(span, &name) {
                    let hover = self.ctor_hover(*target, *ctor);
                    let info = &self.registry.objects[target.0];
                    let module = info.module;
                    let target_span = if info.ctors[*ctor].span.is_empty() {
                        info.decl.map(|decl| decl.name.span)
                    } else {
                        Some(info.ctors[*ctor].span)
                    };
                    let definition = target_span.and_then(|span| self.location(module, span));
                    let ty = Ty::Object(*target, type_args.clone());
                    self.push_typed(at, hover, definition, Some(&ty));
                }
            }
            Callee::Native(native) => {
                let info = &self.registry.natives[*native];
                let (name, module, decl, result) = (
                    info.name.clone(),
                    info.module,
                    info.decl,
                    info.result.clone(),
                );
                if let Some(at) = self.name_before_paren(span, &name) {
                    let signature = format!(
                        "native {}{}{}",
                        if decl.is_async { "async " } else { "" },
                        name,
                        super::outline::signature_text(&decl.params, &decl.result)
                    );
                    let hover = hover_text(&signature, &self.doc(module, decl.span));
                    let definition = self.location(module, decl.name.span);
                    self.push_typed(at, hover, definition, result.as_ref());
                }
            }
            Callee::Test(_) | Callee::FakeStorage(_) => {}
        }
    }

    /// Имя `name` в вызове до первой `(`: `Name(`, `ns.Name(`.
    fn name_before_paren(&self, call: Span, name: &str) -> Option<Span> {
        let text = self.text().get(call.range())?;
        let head = &text[..text.find('(').unwrap_or(text.len())];
        let at = head.rfind(name)?;
        let start = call.start + at as u32;
        Some(Span::new(start, start + name.len() as u32))
    }

    /// Имя действия после receiver: `receiver . name`.
    fn member_after(&self, from: u32, to: u32, name: &str) -> Option<Span> {
        let text = self.text().get(from as usize..to as usize)?;
        let trimmed = text.trim_start();
        let rest = trimmed.strip_prefix('.')?;
        let gap = text.len() - rest.len();
        let rest_trimmed = rest.trim_start();
        let start = from as usize + gap + (rest.len() - rest_trimmed.len());
        rest_trimmed
            .starts_with(name)
            .then(|| Span::new(start as u32, (start + name.len()) as u32))
    }

    fn method_member(&mut self, from: u32, to: u32, object: ObjectId, method: usize) {
        let info = &self.registry.objects[object.0];
        let name = info.methods[method].name.clone();
        let Some(at) = self.member_after(from, to, &name) else {
            return;
        };
        let (hover, definition) = self.method_info(object, method);
        let result = self.registry.objects[object.0].methods[method]
            .result
            .clone();
        self.push_typed(at, hover, definition, result.as_ref());
    }

    fn method_info(&mut self, object: ObjectId, method: usize) -> (String, Option<Location>) {
        let info = &self.registry.objects[object.0];
        let found = &info.methods[method];
        let params = self.params(&found.params, found.variadic);
        let result = if found.successor {
            format!("^{}", self.registry.result_name(&found.result))
        } else {
            self.registry.result_name(&found.result)
        };
        let signature = format!(
            "{}{}{}.{}({}) is {result}",
            if found.private { "private " } else { "" },
            if found.is_async { "async " } else { "" },
            self.ty(&Ty::Object(
                object,
                info.params.iter().map(|param| Ty::Param(*param)).collect()
            )),
            found.name,
            params.join(", ")
        );
        let module = info.module;
        let decl = found.decl;
        let doc = decl
            .map(|decl| self.doc(module, decl.span))
            .unwrap_or_default();
        let definition = decl.and_then(|decl| self.location(module, decl.name.span));
        (hover_text(&signature, &doc), definition)
    }

    fn role_member(&mut self, from: u32, to: u32, role: RoleId, slot: usize) {
        let info = &self.registry.roles[role.0];
        let name = info.methods[slot].name.clone();
        let Some(at) = self.member_after(from, to, &name) else {
            return;
        };
        let (hover, definition) = self.role_method_info(role, slot);
        let result = self.registry.roles[role.0].methods[slot].result.clone();
        self.push_typed(at, hover, definition, result.as_ref());
    }

    fn role_method_info(&mut self, role: RoleId, slot: usize) -> (String, Option<Location>) {
        let info = &self.registry.roles[role.0];
        let found = &info.methods[slot];
        let params = self.params(&found.params, found.variadic);
        let signature = format!(
            "{}{}.{}({}) is {}",
            if found.is_async { "async " } else { "" },
            self.ty(&Ty::Role(
                role,
                info.params.iter().map(|param| Ty::Param(*param)).collect()
            )),
            found.name,
            params.join(", "),
            self.registry.result_name(&found.result)
        );
        let module = info.module;
        let decl = info.decl.and_then(|decl| {
            decl.members.iter().find_map(|member| match member {
                ast::RoleMember::Signature(signature) if signature.name.text == found.name => {
                    Some(signature)
                }
                _ => None,
            })
        });
        let doc = decl
            .map(|decl| self.doc(module, decl.span))
            .unwrap_or_default();
        let definition = decl.and_then(|decl| self.location(module, decl.name.span));
        (hover_text(&signature, &doc), definition)
    }

    fn ctor_hover(&self, object: ObjectId, ctor: usize) -> String {
        let info = &self.registry.objects[object.0];
        let params: Vec<String> = match &info.ctors[ctor].shape {
            CtorShape::Full {
                params, variadic, ..
            } => self.params(params, *variadic),
            CtorShape::Shorthand { fields, .. } => fields
                .iter()
                .map(|field| {
                    let field = &info.fields[*field];
                    format!(":{} is {}", field.name, self.ty(&field.ty))
                })
                .collect(),
        };
        let doc = info
            .decl
            .map(|decl| self.doc(info.module, decl.span))
            .unwrap_or_default();
        hover_text(&format!("{}({})", info.name, params.join(", ")), &doc)
    }

    fn field(&mut self, span: Span, object: ObjectId, field: usize) {
        let info = &self.registry.objects[object.0];
        let Some(found) = info.fields.get(field) else {
            return;
        };
        let signature = format!("has {} is {}", found.name, self.ty(&found.ty));
        let hover = hover_text(&signature, &format!("field of `{}`", info.name));
        let (module, decl, ty) = (info.module, found.span, found.ty.clone());
        let definition = if decl.is_empty() {
            None
        } else {
            self.location(module, decl)
        };
        self.push_typed(span, hover, definition, Some(&ty));
    }

    fn enum_decl(&self, id: usize) -> Option<&'a ast::EnumDecl> {
        let info = &self.registry.enums[id];
        let module = self.registry.modules.get(info.module)?;
        module.program.items.iter().find_map(|item| match item {
            ast::Item::Enum(decl) if decl.name.text == info.name => Some(decl),
            _ => None,
        })
    }

    fn enum_case(&mut self, span: Span, id: usize, case: usize) {
        let info = &self.registry.enums[id];
        let (name, module) = (info.name.clone(), info.module);
        let case_name = info.cases[case].clone();
        let decl = self.enum_decl(id);
        let doc = decl
            .map(|decl| self.doc(module, decl.span))
            .unwrap_or_default();
        let definition = decl
            .and_then(|decl| decl.cases.get(case))
            .and_then(|case| self.location(module, case.span));
        // `Enum.case`: символ — имя варианта в конце выражения.
        let text = self.text().get(span.range()).unwrap_or("");
        let at = if text.ends_with(&case_name) {
            Span::new(span.end - case_name.len() as u32, span.end)
        } else {
            span
        };
        self.push_typed(
            at,
            hover_text(&format!("{name}.{case_name}"), &doc),
            definition,
            Some(&Ty::Enum(EnumId(id))),
        );
    }

    // ---- объявления корня ----

    /// Имена действий, полей и сигнатур ролей корня: наведение показывает
    /// сигнатуру и документацию там, где они объявлены.
    fn declarations(&mut self) {
        let objects: Vec<usize> = (0..self.registry.objects.len())
            .filter(|id| self.registry.objects[*id].module == self.root)
            .collect();
        for id in objects {
            let object = ObjectId(id);
            let info = &self.registry.objects[id];
            let methods: Vec<(usize, Span)> = info
                .methods
                .iter()
                .enumerate()
                .filter_map(|(index, method)| method.decl.map(|decl| (index, decl.name.span)))
                .collect();
            let fields: Vec<(usize, Span)> = info
                .fields
                .iter()
                .enumerate()
                .map(|(index, field)| (index, field.span))
                .collect();
            for (method, span) in methods {
                let (hover, definition) = self.method_info(object, method);
                self.push(span, hover, definition);
            }
            for (field, span) in fields {
                self.field(span, object, field);
            }
        }
        let roles: Vec<usize> = (0..self.registry.roles.len())
            .filter(|id| self.registry.roles[*id].module == self.root)
            .collect();
        for id in roles {
            let info = &self.registry.roles[id];
            let Some(decl) = info.decl else {
                continue;
            };
            for (slot, method) in info.methods.iter().enumerate() {
                let span = decl.members.iter().find_map(|member| match member {
                    ast::RoleMember::Signature(signature) if signature.name.text == method.name => {
                        Some(signature.name.span)
                    }
                    _ => None,
                });
                if let Some(span) = span {
                    let (hover, definition) = self.role_method_info(RoleId(id), slot);
                    self.push(span, hover, definition);
                }
            }
        }
    }

    // ---- имена типов по токенам ----

    fn tokens(&mut self) {
        let covered: HashSet<(u32, u32)> = self
            .symbols
            .iter()
            .map(|symbol| (symbol.span.start, symbol.span.end))
            .collect();
        let text = self.text();
        let lexed = lexer::lex(text);
        let imports: Vec<Span> = self.registry.modules[self.root]
            .program
            .imports
            .iter()
            .map(|import| import.span)
            .collect();
        let tokens = &lexed.tokens;
        for (index, token) in tokens.iter().enumerate() {
            if token.kind != TokenKind::Identifier {
                continue;
            }
            let span = token.span;
            if covered.contains(&(span.start, span.end)) {
                continue;
            }
            let name = &text[span.range()];
            let qualifier = (index >= 2
                && tokens[index - 1].kind == TokenKind::Dot
                && tokens[index - 2].kind == TokenKind::Identifier)
                .then(|| &text[tokens[index - 2].span.range()]);
            if let Some(found) = self.resolve_type(name, qualifier) {
                if let Some((hover, definition)) = self.type_info(found) {
                    self.push(span, hover, definition);
                }
            } else if imports
                .iter()
                .any(|import| import.start <= span.start && span.end <= import.end)
            {
                let native = self.registry.modules[self.root]
                    .scope
                    .natives
                    .get(name)
                    .copied();
                if let Some(native) = native {
                    let info = &self.registry.natives[native];
                    let (module, decl) = (info.module, info.decl);
                    let signature = format!(
                        "native {}{}",
                        info.name,
                        super::outline::signature_text(&decl.params, &decl.result)
                    );
                    let hover = hover_text(&signature, &self.doc(module, decl.span));
                    let definition = self.location(module, decl.name.span);
                    self.push(span, hover, definition);
                }
            }
        }
    }

    fn resolve_type(&self, name: &str, qualifier: Option<&str>) -> Option<TypeRef> {
        let module = &self.registry.modules[self.root];
        if let Some(alias) = qualifier {
            if let Some(namespace) = module.namespaces.get(alias) {
                if let Some(only) = &namespace.only {
                    if !only.iter().any(|taken| taken == name) {
                        return None;
                    }
                }
                return self.registry.modules[namespace.module]
                    .own
                    .types
                    .get(name)
                    .copied();
            }
        }
        if !name.starts_with(|ch: char| ch.is_ascii_uppercase()) {
            return None;
        }
        module.scope.types.get(name).copied()
    }

    fn type_info(&mut self, found: TypeRef) -> Option<(String, Option<Location>)> {
        match found {
            TypeRef::Object(id) => {
                let info = &self.registry.objects[id.0];
                let decl = info.decl?;
                let module = info.module;
                let mut lines = vec![self.object_header(id)];
                for (index, ctor) in info.ctors.iter().enumerate() {
                    if !ctor.span.is_empty() {
                        let text = self.ctor_hover(id, index);
                        let signature = text
                            .strip_prefix("```okkama\n")
                            .and_then(|rest| rest.split("\n```").next())
                            .unwrap_or("")
                            .to_string();
                        lines.push(format!("    {signature}"));
                    }
                }
                for method in info
                    .methods
                    .iter()
                    .filter(|method| !method.private && method.decl.is_some())
                {
                    let params = self.params(&method.params, method.variadic);
                    let result = if method.successor {
                        format!("^{}", self.registry.result_name(&method.result))
                    } else {
                        self.registry.result_name(&method.result)
                    };
                    lines.push(format!(
                        "    {}{}({}) is {result}",
                        if method.is_async { "async " } else { "" },
                        method.name,
                        params.join(", ")
                    ));
                }
                truncate(&mut lines);
                let doc = self.doc(module, decl.span);
                let definition = self.location(module, decl.name.span);
                Some((hover_text(&lines.join("\n"), &doc), definition))
            }
            TypeRef::Role(id) => {
                let info = &self.registry.roles[id.0];
                let decl = info.decl?;
                let module = info.module;
                let mut lines = vec![format!(
                    "role {}",
                    self.ty(&Ty::Role(
                        id,
                        info.params.iter().map(|param| Ty::Param(*param)).collect()
                    ))
                )];
                for method in &info.methods {
                    let params = self.params(&method.params, method.variadic);
                    lines.push(format!(
                        "    {}{}({}) is {}",
                        if method.is_async { "async " } else { "" },
                        method.name,
                        params.join(", "),
                        self.registry.result_name(&method.result)
                    ));
                }
                truncate(&mut lines);
                let doc = self.doc(module, decl.span);
                let definition = self.location(module, decl.name.span);
                Some((hover_text(&lines.join("\n"), &doc), definition))
            }
            TypeRef::Enum(id) => {
                let info = &self.registry.enums[id.0];
                let module = info.module;
                let signature = format!("enum {} = ({})", info.name, info.cases.join(", "));
                let decl = self.enum_decl(id.0);
                let doc = decl
                    .map(|decl| self.doc(module, decl.span))
                    .unwrap_or_default();
                let definition = decl.and_then(|decl| self.location(module, decl.name.span));
                Some((hover_text(&signature, &doc), definition))
            }
            TypeRef::Handle(id) => {
                let info = &self.registry.handles[id.0];
                let (module, span) = (info.module, info.span);
                let signature = format!("native object {}", info.name);
                let doc = self.doc(module, span);
                let definition = self.location(module, span);
                Some((hover_text(&signature, &doc), definition))
            }
        }
    }

    fn object_header(&self, id: ObjectId) -> String {
        let info = &self.registry.objects[id.0];
        let name = self.ty(&Ty::Object(
            id,
            info.params.iter().map(|param| Ty::Param(*param)).collect(),
        ));
        if info.does.is_empty() {
            format!("object {name}")
        } else {
            let roles: Vec<String> = info
                .does
                .iter()
                .map(|(role, args)| self.ty(&Ty::Role(*role, args.clone())))
                .collect();
            format!("object {name} does {}", roles.join(", "))
        }
    }
}

/// Длинные списки действий в наведении обрезаются.
fn truncate(lines: &mut Vec<String>) {
    const LIMIT: usize = 40;
    if lines.len() > LIMIT {
        let more = lines.len() - LIMIT;
        lines.truncate(LIMIT);
        lines.push(format!("    // … ещё {more}"));
    }
}
