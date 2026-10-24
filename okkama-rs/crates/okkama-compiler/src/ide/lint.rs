//! Линтер (`okkama lint`, предупреждения в редакторе): корректный код, который,
//! вероятно, написан не так, как задумано.
//!
//! | код | правило |
//! | --- | --- |
//! | `unused-import` | имя из `take` (или псевдоним `as`) не встречается в файле |
//! | `unused-let` | значение `let` не читается (кроме имён с `_` в начале и объектов с `destroy`: их держат ради области жизни) |
//! | `unused-private` | приватное действие объекта нигде не вызывается |
//! | `unreachable` | оператор после `return`, `throw`, `next`, `last` или другого ухода управления |
//! | `snake-case` | локальная, параметр, поле или действие не в `snake_case` |
//!
//! Правила намеренно консервативны: ложное предупреждение хуже пропущенного.

use std::collections::HashSet;

use super::{Diagnostic, LineIndex, Severity};
use crate::analysis::registry::Registry;
use crate::analysis::thir::{self, Body, Callee, ExprKind, LocalId, StmtKind};
use crate::analysis::ty::{ObjectId, Ty};
use crate::source::{SourceFile, Span};
use crate::syntax::{ast, lexer, token::TokenKind};

pub fn lint(
    registry: &Registry<'_>,
    bodies: &[Body],
    source: &SourceFile,
    root: usize,
) -> Vec<Diagnostic> {
    let mut linter = Linter {
        registry,
        source,
        lines: LineIndex::new(source.text()),
        out: Vec::new(),
        called: HashSet::new(),
    };
    for body in bodies {
        linter.calls_in_block(&body.block);
    }
    for body in bodies.iter().filter(|body| body.module == root) {
        linter.body(body);
    }
    linter.imports(registry.modules[root].program);
    linter.private_actions(root);
    linter.declarations(registry.modules[root].program);
    linter
        .out
        .sort_by_key(|found| (found.range.start, found.message.clone()));
    linter
        .out
        .dedup_by(|a, b| a.range == b.range && a.message == b.message);
    linter.out
}

struct Linter<'r, 'a> {
    registry: &'r Registry<'a>,
    source: &'r SourceFile,
    lines: LineIndex<'r>,
    out: Vec<Diagnostic>,
    /// Вызванные действия: (объект, номер действия).
    called: HashSet<(usize, usize)>,
}

impl Linter<'_, '_> {
    fn warn(&mut self, span: Span, code: &str, message: String, unnecessary: bool) {
        self.out.push(Diagnostic {
            path: self.source.name().to_string(),
            range: self.lines.range(span),
            severity: Severity::Warning,
            message,
            code: Some(code.to_string()),
            unnecessary,
        });
    }

    // ---- вызовы (для unused-private) ----

    fn calls_in_block(&mut self, block: &thir::Block) {
        let mut found = Vec::new();
        walk_block(block, &mut |expr| match &expr.kind {
            ExprKind::Call {
                callee: Callee::Method { object, method, .. },
                ..
            }
            | ExprKind::BoundAction { object, method, .. }
            | ExprKind::Projection { object, method, .. } => found.push((object.0, *method)),
            _ => {}
        });
        self.called.extend(found);
        for inner in closures(block) {
            self.calls_in_block(&inner.block);
        }
    }

    fn private_actions(&mut self, root: usize) {
        for (id, object) in self.registry.objects.iter().enumerate() {
            if object.module != root {
                continue;
            }
            for (index, method) in object.methods.iter().enumerate() {
                let Some(decl) = method.decl else {
                    continue;
                };
                if method.private && !self.called.contains(&(id, index)) {
                    self.warn(
                        decl.name.span,
                        "unused-private",
                        format!(
                            "private action `{}` of `{}` is never called",
                            method.name, object.name
                        ),
                        true,
                    );
                }
            }
        }
    }

    // ---- тела ----

    fn body(&mut self, body: &Body) {
        let mut reads = HashSet::new();
        collect_reads(&body.block, &mut reads);
        let mut lets = Vec::new();
        collect_lets(&body.block, &mut lets);
        for local in lets {
            let decl = &body.locals[local.0];
            if reads.contains(&local) || decl.name.starts_with('_') || decl.span.is_empty() {
                continue;
            }
            if self.scoped(&decl.ty) {
                continue;
            }
            self.warn(
                decl.span,
                "unused-let",
                format!(
                    "`{}` is never read (name it `_{}` if that is intended)",
                    decl.name, decl.name
                ),
                true,
            );
        }
        self.unreachable(&body.block);
        for inner in closures(&body.block) {
            self.body(inner);
        }
    }

    /// Значение держат ради `destroy` в конце области.
    fn scoped(&self, ty: &Ty) -> bool {
        match ty {
            Ty::Object(ObjectId(id), _) => self.registry.objects[*id].destroy.is_some(),
            Ty::Successor(inner) => self.scoped(inner),
            _ => false,
        }
    }

    fn unreachable(&mut self, block: &thir::Block) {
        for (index, stmt) in block.stmts.iter().enumerate() {
            let leaves = match &stmt.kind {
                StmtKind::Return(_)
                | StmtKind::Throw(_)
                | StmtKind::Rethrow
                | StmtKind::Next
                | StmtKind::Last(_) => true,
                StmtKind::Expr(expr) => expr.diverges,
                _ => false,
            };
            if leaves {
                if let Some(next) = block.stmts.get(index + 1) {
                    self.warn(
                        next.span,
                        "unreachable",
                        "unreachable code".to_string(),
                        true,
                    );
                }
                break;
            }
        }
        for stmt in &block.stmts {
            for_nested_blocks(stmt, &mut |inner| self.unreachable(inner));
        }
    }

    // ---- исходник ----

    fn imports(&mut self, program: &ast::Program) {
        let text = self.source.text();
        let lexed = lexer::lex(text);
        let inside_imports = |span: Span| {
            program
                .imports
                .iter()
                .any(|import| import.span.start <= span.start && span.end <= import.span.end)
        };
        let used: HashSet<&str> = lexed
            .tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Identifier && !inside_imports(token.span))
            .map(|token| &text[token.span.range()])
            .collect();
        for import in &program.imports {
            if let Some(alias) = &import.alias {
                if import.take.is_empty() && !used.contains(alias.text.as_str()) {
                    self.warn(
                        alias.span,
                        "unused-import",
                        format!("import alias `{}` is never used", alias.text),
                        true,
                    );
                }
            }
            for name in &import.take {
                if !used.contains(name.text.as_str()) {
                    self.warn(
                        name.span,
                        "unused-import",
                        format!("`{}` is imported but never used", name.text),
                        true,
                    );
                }
            }
        }
    }

    fn declarations(&mut self, program: &ast::Program) {
        for item in &program.items {
            match item {
                ast::Item::Object(decl) => self.members(&decl.members),
                ast::Item::Role(decl) => {
                    for member in &decl.members {
                        match member {
                            ast::RoleMember::Signature(signature) => {
                                self.name(&signature.name, "action");
                                self.params(&signature.params);
                            }
                            ast::RoleMember::Fake(fake) => {
                                if let ast::FakeBody::Parsed(members) = &fake.body {
                                    self.members(members);
                                }
                            }
                            ast::RoleMember::Test(_) => {}
                        }
                    }
                }
                ast::Item::Action(decl) => self.params(&decl.params),
                ast::Item::NativeAction(decl) => self.params(&decl.params),
                _ => {}
            }
            walk_lets(item, &mut |name| self.name(name, "local"));
        }
    }

    fn members(&mut self, members: &[ast::ObjectMember]) {
        for member in members {
            match member {
                ast::ObjectMember::Has(has) => {
                    for name in &has.names {
                        self.name(name, "field");
                    }
                }
                ast::ObjectMember::Action(action) => {
                    self.name(&action.name, "action");
                    self.params(&action.params);
                }
                ast::ObjectMember::Private(section) => {
                    for action in &section.actions {
                        self.name(&action.name, "action");
                        self.params(&action.params);
                    }
                }
                ast::ObjectMember::Constructor(ctor) => {
                    if let ast::ConstructorForm::Full { params, .. } = &ctor.form {
                        self.params(params);
                    }
                }
                _ => {}
            }
        }
    }

    fn params(&mut self, params: &[ast::Param]) {
        for param in params {
            self.name(&param.name, "parameter");
        }
    }

    fn name(&mut self, name: &ast::Name, what: &str) {
        let text = &name.text;
        if text.chars().any(|ch| ch.is_ascii_uppercase())
            && text.starts_with(|ch: char| ch.is_ascii_lowercase() || ch == '_')
        {
            self.warn(
                name.span,
                "snake-case",
                format!("{what} `{text}` should be snake_case: `{}`", snake(text)),
                false,
            );
        }
    }
}

fn snake(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if ch.is_ascii_uppercase() {
            if !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Имена `let` во всех телах объявления (по синтаксическому дереву).
fn walk_lets(item: &ast::Item, visit: &mut dyn FnMut(&ast::Name)) {
    fn block(found: &ast::Block, visit: &mut dyn FnMut(&ast::Name)) {
        for stmt in &found.stmts {
            match &stmt.kind {
                ast::StmtKind::Let { name, .. } => visit(name),
                ast::StmtKind::Block(inner) => block(inner, visit),
                ast::StmtKind::If(found) => if_stmt(found, visit),
                ast::StmtKind::For { body, bindings, .. } => {
                    for binding in bindings {
                        visit(&binding.name);
                    }
                    block(body, visit);
                }
                ast::StmtKind::Try(body) => {
                    block(&body.body, visit);
                    for catch in &body.catches {
                        block(&catch.body, visit);
                    }
                }
                ast::StmtKind::Match(body) => {
                    for case in &body.cases {
                        block(&case.body, visit);
                    }
                    if let Some(other) = &body.else_block {
                        block(other, visit);
                    }
                }
                _ => {}
            }
        }
    }
    fn if_stmt(found: &ast::IfStmt, visit: &mut dyn FnMut(&ast::Name)) {
        block(&found.then_block, visit);
        match &found.else_branch {
            Some(ast::ElseBranch::Block(other)) => block(other, visit),
            Some(ast::ElseBranch::If(next)) => if_stmt(next, visit),
            None => {}
        }
    }
    fn members(members: &[ast::ObjectMember], visit: &mut dyn FnMut(&ast::Name)) {
        for member in members {
            match member {
                ast::ObjectMember::Action(action) => block(&action.body, visit),
                ast::ObjectMember::Private(section) => {
                    for action in &section.actions {
                        block(&action.body, visit);
                    }
                }
                ast::ObjectMember::Constructor(ctor) => match &ctor.form {
                    ast::ConstructorForm::Full { body, .. } => block(body, visit),
                    ast::ConstructorForm::Shorthand {
                        body: Some(body), ..
                    } => block(body, visit),
                    ast::ConstructorForm::Shorthand { body: None, .. } => {}
                },
                ast::ObjectMember::Destroy(destroy) => block(&destroy.body, visit),
                _ => {}
            }
        }
    }
    match item {
        ast::Item::Object(decl) => members(&decl.members, visit),
        ast::Item::Action(decl) => block(&decl.body, visit),
        _ => {}
    }
}

// ---- обход THIR ----

/// Каждое выражение блока (включая вложенные блоки, но не тела closures).
fn walk_block(block: &thir::Block, visit: &mut dyn FnMut(&thir::Expr)) {
    for stmt in &block.stmts {
        walk_stmt(stmt, visit);
    }
}

fn walk_stmt(stmt: &thir::Stmt, visit: &mut dyn FnMut(&thir::Expr)) {
    match &stmt.kind {
        StmtKind::Let { value, .. }
        | StmtKind::Expr(value)
        | StmtKind::Throw(value)
        | StmtKind::Defer(value)
        | StmtKind::BindField { value, .. }
        | StmtKind::StageField { value, .. }
        | StmtKind::Rebind { value, .. } => walk_expr(value, visit),
        StmtKind::Guarded { condition, stmt } => {
            walk_expr(condition, visit);
            walk_stmt(stmt, visit);
        }
        StmtKind::Return(value) | StmtKind::Last(value) => {
            if let Some(value) = value {
                walk_expr(value, visit);
            }
        }
        StmtKind::Confirm {
            condition,
            fallback,
        } => {
            walk_expr(condition, visit);
            if let Some(thir::Fallback::Throw(value) | thir::Fallback::Panic(value)) = fallback {
                walk_expr(value, visit);
            }
        }
        StmtKind::Rethrow | StmtKind::Next => {}
    }
}

fn walk_source(source: &thir::Source, visit: &mut dyn FnMut(&thir::Expr)) {
    match source {
        thir::Source::Range { start, end, .. } => {
            walk_expr(start, visit);
            walk_expr(end, visit);
        }
        thir::Source::Array(value) | thir::Source::Map(value) | thir::Source::Iterating(value) => {
            walk_expr(value, visit)
        }
        thir::Source::Varargs(_) => {}
    }
}

fn walk_expr(expr: &thir::Expr, visit: &mut dyn FnMut(&thir::Expr)) {
    visit(expr);
    match &expr.kind {
        ExprKind::Constant(_)
        | ExprKind::Local(_)
        | ExprKind::SelfValue
        | ExprKind::Field(_)
        | ExprKind::PendingField(_)
        | ExprKind::OwnerField(_)
        | ExprKind::EnumCase { .. }
        | ExprKind::TestCapability
        | ExprKind::Unreachable
        | ExprKind::Closure { .. } => {}
        ExprKind::Unary { operand: value, .. }
        | ExprKind::Coerce { value, .. }
        | ExprKind::Cast { value, .. }
        | ExprKind::Panic(value)
        | ExprKind::Unpack(value)
        | ExprKind::Await(value)
        | ExprKind::Spawn(value)
        | ExprKind::Projection {
            construction: value,
            ..
        }
        | ExprKind::CapSelect { owner: value, .. }
        | ExprKind::CapRead { cap: value, .. }
        | ExprKind::BoundRoleAction {
            receiver: value, ..
        }
        | ExprKind::BoundAction {
            receiver: value, ..
        } => walk_expr(value, visit),
        ExprKind::Binary { left, right, .. }
        | ExprKind::Logical { left, right, .. }
        | ExprKind::RatNew {
            numerator: left,
            denominator: right,
        }
        | ExprKind::Range {
            start: left,
            end: right,
            ..
        }
        | ExprKind::StreamWrite {
            sink: left,
            source: right,
            ..
        } => {
            walk_expr(left, visit);
            walk_expr(right, visit);
        }
        ExprKind::If {
            condition,
            then_block,
            else_block,
        } => {
            walk_expr(condition, visit);
            walk_block(then_block, visit);
            if let Some(block) = else_block {
                walk_block(block, visit);
            }
        }
        ExprKind::Match(found) => {
            walk_expr(&found.subject, visit);
            for (patterns, block) in &found.cases {
                for pattern in patterns {
                    walk_expr(pattern, visit);
                }
                walk_block(block, visit);
            }
            if let Some(block) = &found.else_block {
                walk_block(block, visit);
            }
        }
        ExprKind::Loop(block) | ExprKind::Block(block) => walk_block(block, visit),
        ExprKind::For { source, body, .. } => {
            walk_source(source, visit);
            walk_block(body, visit);
        }
        ExprKind::Try(found) => {
            walk_block(&found.body, visit);
            for catch in &found.catches {
                walk_block(&catch.body, visit);
            }
        }
        ExprKind::Call { callee, args } => {
            match callee {
                Callee::Method { receiver, .. }
                | Callee::Role { receiver, .. }
                | Callee::Builtin { receiver, .. } => walk_expr(receiver, visit),
                Callee::Ctor { .. }
                | Callee::Native(_)
                | Callee::Test(_)
                | Callee::FakeStorage(_) => {}
            }
            for arg in args {
                walk_expr(arg, visit);
            }
        }
        ExprKind::Array(parts) => {
            for part in parts {
                match part {
                    thir::ArrayPart::Value(value) => walk_expr(value, visit),
                    thir::ArrayPart::Range { start, end, .. } => {
                        walk_expr(start, visit);
                        walk_expr(end, visit);
                    }
                }
            }
        }
        ExprKind::Map(pairs) => {
            for (key, value) in pairs {
                walk_expr(key, visit);
                walk_expr(value, visit);
            }
        }
        ExprKind::Index {
            object,
            index,
            fallback,
        } => {
            walk_expr(object, visit);
            walk_expr(index, visit);
            if let Some(fallback) = fallback {
                walk_expr(fallback, visit);
            }
        }
        ExprKind::Functional(functional) => {
            walk_source(&functional.source, visit);
            if let Some(initial) = &functional.initial {
                walk_expr(initial, visit);
            }
            walk_block(&functional.body, visit);
        }
        ExprKind::ArrayCast { parts, .. } => {
            for part in parts {
                match part {
                    thir::CastPart::One(value) | thir::CastPart::Spread(value) => {
                        walk_expr(value, visit)
                    }
                }
            }
        }
        ExprKind::Pack { items, spread } => {
            for item in items {
                walk_expr(item, visit);
            }
            if let Some(spread) = spread {
                walk_expr(spread, visit);
            }
        }
        ExprKind::CallValue { callee, args } => {
            walk_expr(callee, visit);
            for arg in args {
                walk_expr(arg, visit);
            }
        }
    }
}

/// Тела closures блока (без вложенных в них: их обходит `Linter::body`).
fn closures(block: &thir::Block) -> Vec<&Body> {
    let mut out = Vec::new();
    closures_in_block(block, &mut out);
    out
}

fn closures_in_block<'b>(block: &'b thir::Block, out: &mut Vec<&'b Body>) {
    for stmt in &block.stmts {
        let mut exprs = Vec::new();
        stmt_exprs(stmt, &mut exprs);
        for expr in exprs {
            closures_in_expr(expr, out);
        }
    }
}

fn closures_in_expr<'b>(expr: &'b thir::Expr, out: &mut Vec<&'b Body>) {
    if let ExprKind::Closure { body, .. } = &expr.kind {
        out.push(body);
        return;
    }
    let mut children = Vec::new();
    let mut blocks = Vec::new();
    children_of(expr, &mut children, &mut blocks);
    for child in children {
        closures_in_expr(child, out);
    }
    for block in blocks {
        closures_in_block(block, out);
    }
}

/// Выражения оператора верхнего уровня.
fn stmt_exprs<'b>(stmt: &'b thir::Stmt, out: &mut Vec<&'b thir::Expr>) {
    match &stmt.kind {
        StmtKind::Let { value, .. }
        | StmtKind::Expr(value)
        | StmtKind::Throw(value)
        | StmtKind::Defer(value)
        | StmtKind::BindField { value, .. }
        | StmtKind::StageField { value, .. }
        | StmtKind::Rebind { value, .. } => out.push(value),
        StmtKind::Guarded { condition, stmt } => {
            out.push(condition);
            stmt_exprs(stmt, out);
        }
        StmtKind::Return(Some(value)) | StmtKind::Last(Some(value)) => out.push(value),
        StmtKind::Confirm {
            condition,
            fallback,
        } => {
            out.push(condition);
            if let Some(thir::Fallback::Throw(value) | thir::Fallback::Panic(value)) = fallback {
                out.push(value);
            }
        }
        _ => {}
    }
}

/// Непосредственные подвыражения и блоки выражения.
fn children_of<'b>(
    expr: &'b thir::Expr,
    out: &mut Vec<&'b thir::Expr>,
    blocks: &mut Vec<&'b thir::Block>,
) {
    match &expr.kind {
        ExprKind::Unary { operand: value, .. }
        | ExprKind::Coerce { value, .. }
        | ExprKind::Cast { value, .. }
        | ExprKind::Panic(value)
        | ExprKind::Unpack(value)
        | ExprKind::Await(value)
        | ExprKind::Spawn(value)
        | ExprKind::Projection {
            construction: value,
            ..
        }
        | ExprKind::CapSelect { owner: value, .. }
        | ExprKind::CapRead { cap: value, .. }
        | ExprKind::BoundRoleAction {
            receiver: value, ..
        }
        | ExprKind::BoundAction {
            receiver: value, ..
        } => out.push(value),
        ExprKind::Binary { left, right, .. }
        | ExprKind::Logical { left, right, .. }
        | ExprKind::RatNew {
            numerator: left,
            denominator: right,
        }
        | ExprKind::Range {
            start: left,
            end: right,
            ..
        }
        | ExprKind::StreamWrite {
            sink: left,
            source: right,
            ..
        } => {
            out.push(left);
            out.push(right);
        }
        ExprKind::If {
            condition,
            then_block,
            else_block,
        } => {
            out.push(condition);
            blocks.push(then_block);
            if let Some(block) = else_block {
                blocks.push(block);
            }
        }
        ExprKind::Match(found) => {
            out.push(&found.subject);
            for (patterns, block) in &found.cases {
                out.extend(patterns.iter());
                blocks.push(block);
            }
            if let Some(block) = &found.else_block {
                blocks.push(block);
            }
        }
        ExprKind::Loop(block) | ExprKind::Block(block) => blocks.push(block),
        ExprKind::For { source, body, .. } => {
            source_exprs(source, out);
            blocks.push(body);
        }
        ExprKind::Try(found) => {
            blocks.push(&found.body);
            for catch in &found.catches {
                blocks.push(&catch.body);
            }
        }
        ExprKind::Call { callee, args } => {
            if let Callee::Method { receiver, .. }
            | Callee::Role { receiver, .. }
            | Callee::Builtin { receiver, .. } = callee
            {
                out.push(receiver);
            }
            out.extend(args.iter());
        }
        ExprKind::Array(parts) => {
            for part in parts {
                match part {
                    thir::ArrayPart::Value(value) => out.push(value),
                    thir::ArrayPart::Range { start, end, .. } => {
                        out.push(start);
                        out.push(end);
                    }
                }
            }
        }
        ExprKind::Map(pairs) => {
            for (key, value) in pairs {
                out.push(key);
                out.push(value);
            }
        }
        ExprKind::Index {
            object,
            index,
            fallback,
        } => {
            out.push(object);
            out.push(index);
            if let Some(fallback) = fallback {
                out.push(fallback);
            }
        }
        ExprKind::Functional(functional) => {
            source_exprs(&functional.source, out);
            if let Some(initial) = &functional.initial {
                out.push(initial);
            }
            blocks.push(&functional.body);
        }
        ExprKind::ArrayCast { parts, .. } => {
            for part in parts {
                match part {
                    thir::CastPart::One(value) | thir::CastPart::Spread(value) => out.push(value),
                }
            }
        }
        ExprKind::Pack { items, spread } => {
            out.extend(items.iter());
            if let Some(spread) = spread {
                out.push(spread);
            }
        }
        ExprKind::CallValue { callee, args } => {
            out.push(callee);
            out.extend(args.iter());
        }
        _ => {}
    }
}

fn source_exprs<'b>(source: &'b thir::Source, out: &mut Vec<&'b thir::Expr>) {
    match source {
        thir::Source::Range { start, end, .. } => {
            out.push(start);
            out.push(end);
        }
        thir::Source::Array(value) | thir::Source::Map(value) | thir::Source::Iterating(value) => {
            out.push(value)
        }
        thir::Source::Varargs(_) => {}
    }
}

/// Читаемые локальные тела (closures читают свои захваты).
fn collect_reads(block: &thir::Block, reads: &mut HashSet<LocalId>) {
    walk_block(block, &mut |expr| match &expr.kind {
        ExprKind::Local(local) => {
            reads.insert(*local);
        }
        ExprKind::Closure { captures, .. } => {
            for (outer, _) in captures {
                reads.insert(*outer);
            }
        }
        ExprKind::For {
            source: thir::Source::Varargs(local),
            ..
        } => {
            reads.insert(*local);
        }
        ExprKind::Functional(functional) => {
            if let thir::Source::Varargs(local) = &functional.source {
                reads.insert(*local);
            }
        }
        _ => {}
    });
}

/// Локальные, объявленные `let` в этом теле (не в closures).
fn collect_lets(block: &thir::Block, out: &mut Vec<LocalId>) {
    for stmt in &block.stmts {
        if let StmtKind::Let { local, .. } = &stmt.kind {
            out.push(*local);
        }
        for_nested_blocks(stmt, &mut |inner| collect_lets(inner, out));
    }
}

/// Блоки внутри оператора (`if`, `match`, `for`, `try`, `loop`, вложенные
/// блоки) без тел closures.
fn for_nested_blocks(stmt: &thir::Stmt, visit: &mut dyn FnMut(&thir::Block)) {
    let mut exprs = Vec::new();
    stmt_exprs(stmt, &mut exprs);
    for expr in exprs {
        nested_in_expr(expr, visit);
    }
}

fn nested_in_expr(expr: &thir::Expr, visit: &mut dyn FnMut(&thir::Block)) {
    if matches!(expr.kind, ExprKind::Closure { .. }) {
        return;
    }
    let mut children = Vec::new();
    let mut blocks = Vec::new();
    children_of(expr, &mut children, &mut blocks);
    for block in blocks {
        visit(block);
    }
    for child in children {
        nested_in_expr(child, visit);
    }
}
