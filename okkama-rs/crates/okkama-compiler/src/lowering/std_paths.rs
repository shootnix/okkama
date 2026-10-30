//! Пути `std.…`: объявление стандартной библиотеки без импорта, как `std::`
//! в Rust.
//!
//! ```okkama
//! let out = std.io.stdout.Stdout();
//! let path is std.fs.path.Path = std.fs.path.Path("home", "user");
//! ```
//!
//! Перед связыванием программа переписывается: каждый модуль std, названный
//! путём, получает скрытый импорт `import "@std/io/stdout" as std.io.stdout;`,
//! а путь становится обычным доступом через пространство имён: имя
//! `std.io.stdout` пользователь написать не может, поэтому оно ни с чем не
//! совпадёт. Дальше работает обычная загрузка импортов: модуль ищется только
//! в std (`@std/`), грузится один раз на программу — вместе с явными
//! импортами того же файла — и попадает в OIR, как любой импорт, только
//! достижимыми частями.
//!
//! Модуль пути — сегменты со строчной буквы до первого с заглавной
//! (`std.fs.path.Path`: модуль `fs/path`, объявление `Path`); дальше —
//! обычные обращения (`std.x.Light.red`). Путь только из строчных сегментов
//! называет native-функцию, если его вызывают: `std.runtime.debug.data_dump(v)`.
//! Слово `std` зарезервировано: им нельзя назвать переменную, параметр или
//! пространство имён.
use super::LoweringError;
use crate::source::Span;
use crate::syntax::ast::{self, Expr, ExprKind, TypeExpr, TypeKind};
use crate::syntax::token::{StrLiteral, StringForm};

/// Префикс пути импорта: модуль ищется только в стандартной библиотеке.
pub const STD_IMPORT: &str = "@std/";

/// Переписать пути `std.…` программы в скрытые импорты; первая ошибка — отказ.
pub(super) fn desugar(program: &mut ast::Program) -> Result<(), LoweringError> {
    let mut rewrite = Rewrite::default();
    for import in &program.imports {
        if let Some(alias) = &import.alias {
            rewrite.binding(alias);
        }
    }
    for item in &mut program.items {
        rewrite.item(item);
    }
    if let Some(error) = rewrite.error {
        return Err(error);
    }
    for (module, span) in rewrite.modules {
        program.imports.push(ast::ImportDecl {
            path: ast::StringLit {
                literal: StrLiteral {
                    value: format!("{STD_IMPORT}{module}"),
                    form: StringForm::Quoted,
                },
                span,
            },
            alias: Some(ast::Name {
                text: namespace(&module),
                span,
            }),
            take: Vec::new(),
            span,
        });
    }
    Ok(())
}

/// Пространство имён модуля `fs/path`: `std.fs.path`.
fn namespace(module: &str) -> String {
    format!("std.{}", module.replace('/', "."))
}

#[derive(Default)]
struct Rewrite {
    /// Модули std по порядку первого упоминания и место этого упоминания.
    modules: Vec<(String, Span)>,
    error: Option<LoweringError>,
}

impl Rewrite {
    fn fail(&mut self, span: Span, message: String) {
        if self.error.is_none() {
            self.error = Some(LoweringError::invalid(span, message));
        }
    }

    fn binding(&mut self, name: &ast::Name) {
        if name.text == "std" {
            self.fail(name.span, "\"std\" is reserved for standard library paths (std.io.stdout.Stdout)".into());
        }
    }

    /// Пространство имён модуля из сегментов пути.
    fn module(&mut self, segments: &[ast::Name]) -> String {
        let module: Vec<&str> = segments.iter().map(|segment| segment.text.as_str()).collect();
        let module = module.join("/");
        if !self.modules.iter().any(|(known, _)| *known == module) {
            let span = Span::new(segments[0].span.start, segments[segments.len() - 1].span.end);
            self.modules.push((module.clone(), span));
        }
        namespace(&module)
    }

    fn item(&mut self, item: &mut ast::Item) {
        match item {
            ast::Item::Object(decl) => {
                for generic in &mut decl.generics {
                    self.generic(generic);
                }
                for role in &mut decl.roles {
                    self.ty(role);
                }
                for member in &mut decl.members {
                    self.member(member);
                }
            }
            ast::Item::Role(decl) => {
                for generic in &mut decl.generics {
                    self.generic(generic);
                }
                for member in &mut decl.members {
                    match member {
                        ast::RoleMember::Signature(signature) => {
                            self.params(&mut signature.params);
                            self.result(&mut signature.result);
                        }
                        ast::RoleMember::Fake(fake) => {
                            if let ast::FakeBody::Parsed(members) = &mut fake.body {
                                for member in members {
                                    self.member(member);
                                }
                            }
                        }
                        ast::RoleMember::Test(test) => self.test(test),
                    }
                }
            }
            ast::Item::Enum(_) | ast::Item::NativeObject(_) => {}
            ast::Item::Test(test) => self.test(test),
            ast::Item::NativeAction(decl) => {
                self.params(&mut decl.params);
                self.result(&mut decl.result);
            }
            ast::Item::Cap(cap) => self.cap(cap),
            ast::Item::Action(action) => self.action(action),
        }
    }

    fn generic(&mut self, generic: &mut ast::GenericParam) {
        if let Some(bound) = &mut generic.bound {
            self.ty(bound);
        }
    }

    fn member(&mut self, member: &mut ast::ObjectMember) {
        match member {
            ast::ObjectMember::Has(has) => {
                for name in &has.names {
                    self.binding(name);
                }
                if let Some(ty) = &mut has.ty {
                    self.ty(ty);
                }
                if let Some(default) = &mut has.default {
                    self.expr(default, false);
                }
            }
            ast::ObjectMember::Cap(cap) => self.cap(cap),
            ast::ObjectMember::Constructor(ctor) => match &mut ctor.form {
                ast::ConstructorForm::Full { params, body } => {
                    self.params(params);
                    self.block(body);
                }
                ast::ConstructorForm::Shorthand { body, .. } => {
                    if let Some(body) = body {
                        self.block(body);
                    }
                }
            },
            ast::ObjectMember::Destroy(destroy) => self.block(&mut destroy.body),
            ast::ObjectMember::Test(test) => self.test(test),
            ast::ObjectMember::Action(action) => self.action(action),
            ast::ObjectMember::Private(section) => {
                for action in &mut section.actions {
                    self.action(action);
                }
            }
        }
    }

    fn cap(&mut self, cap: &mut ast::CapDecl) {
        for field in &mut cap.fields {
            self.ty(&mut field.ty);
            self.block(&mut field.body);
        }
    }

    fn test(&mut self, test: &mut ast::TestDecl) {
        if let ast::TestBody::Parsed(body) = &mut test.body {
            self.block(body);
        }
    }

    fn action(&mut self, action: &mut ast::ActionDecl) {
        self.params(&mut action.params);
        self.result(&mut action.result);
        self.block(&mut action.body);
    }

    fn params(&mut self, params: &mut [ast::Param]) {
        for param in params {
            self.binding(&param.name);
            self.ty(&mut param.ty);
        }
    }

    fn result(&mut self, result: &mut ast::ResultSpec) {
        match result {
            ast::ResultSpec::Void(_) => {}
            ast::ResultSpec::Successor { ty, .. } | ast::ResultSpec::Type(ty) => self.ty(ty),
        }
    }

    fn action_result(&mut self, result: &mut ast::ActionResult) {
        if let ast::ActionResult::Type(ty) = result {
            self.ty(ty);
        }
    }

    fn ty(&mut self, ty: &mut TypeExpr) {
        match &mut ty.kind {
            TypeKind::Named { path, args } => {
                for arg in args {
                    self.ty(arg);
                }
                if path.first().is_some_and(|first| first.text == "std") {
                    if path.len() < 3 {
                        self.fail(
                            ty.span,
                            "A std type path names a module and a type: std.io.stdout.Stdout".into(),
                        );
                        return;
                    }
                    let item = path.pop().expect("checked length");
                    let space = self.module(&path[1..]);
                    let span = Span::new(path[0].span.start, path[path.len() - 1].span.end);
                    *path = vec![ast::Name { text: space, span }, item];
                }
            }
            TypeKind::Nested { owner, .. } => self.ty(owner),
            TypeKind::Array(element) => self.ty(element),
            TypeKind::Map { key, value } => {
                self.ty(key);
                self.ty(value);
            }
            TypeKind::Action { params, result, .. } => {
                for param in params {
                    self.ty(param);
                }
                self.action_result(result);
            }
        }
    }

    fn block(&mut self, block: &mut ast::Block) {
        for stmt in &mut block.stmts {
            self.stmt(stmt);
        }
    }

    fn stmt(&mut self, stmt: &mut ast::Stmt) {
        use ast::StmtKind as S;
        match &mut stmt.kind {
            S::Let { name, ty, value } => {
                self.binding(name);
                if let Some(ty) = ty {
                    self.ty(ty);
                }
                self.expr(value, false);
            }
            S::Defer { value } | S::SuccessorAssign { value, .. } | S::FieldBind { value, .. } => {
                self.expr(value, false)
            }
            S::SelfFieldBind { value, .. } => self.expr(value, false),
            S::Try(body) => self.try_body(body),
            S::Throw { value } | S::Return { value } => {
                if let Some(value) = value {
                    self.expr(value, false);
                }
            }
            S::Confirm { condition, fallback } => {
                self.expr(condition, false);
                match fallback {
                    Some(ast::ConfirmFallback::Panic { value, .. } | ast::ConfirmFallback::Throw { value, .. }) => {
                        self.expr(value, false)
                    }
                    None => {}
                }
            }
            S::Block(block) => self.block(block),
            S::If(stmt) => self.if_stmt(stmt),
            S::Match(body) => self.match_body(body),
            S::For { source, bindings, body } => {
                self.expr(source, false);
                for binding in bindings.iter() {
                    self.binding(&binding.name);
                }
                self.block(body);
            }
            S::Next { condition } => {
                if let Some(condition) = condition {
                    self.expr(condition, false);
                }
            }
            S::Last { value, condition } => {
                for expr in [value, condition].into_iter().flatten() {
                    self.expr(expr, false);
                }
            }
            S::Expr { expr, condition } => {
                self.expr(expr, false);
                if let Some(condition) = condition {
                    self.expr(condition, false);
                }
            }
        }
    }

    fn if_stmt(&mut self, stmt: &mut ast::IfStmt) {
        self.expr(&mut stmt.condition, false);
        self.block(&mut stmt.then_block);
        match &mut stmt.else_branch {
            Some(ast::ElseBranch::Block(block)) => self.block(block),
            Some(ast::ElseBranch::If(nested)) => self.if_stmt(nested),
            None => {}
        }
    }

    fn match_body(&mut self, body: &mut ast::MatchBody) {
        self.expr(&mut body.subject, false);
        for case in &mut body.cases {
            for pattern in &mut case.patterns {
                self.expr(pattern, false);
            }
            self.block(&mut case.body);
        }
        if let Some(block) = &mut body.else_block {
            self.block(block);
        }
    }

    fn try_body(&mut self, body: &mut ast::TryBody) {
        self.block(&mut body.body);
        for catch in &mut body.catches {
            self.binding(&catch.name);
            self.ty(&mut catch.ty);
            self.block(&mut catch.body);
        }
    }

    fn args(&mut self, args: &mut [ast::Argument]) {
        for arg in args {
            self.expr(&mut arg.value, false);
        }
    }

    /// `called` — выражение стоит на месте вызываемого: путь из строчных
    /// сегментов называет native-функцию.
    fn expr(&mut self, expr: &mut Expr, called: bool) {
        if self.std_path(expr, called) {
            return;
        }
        match &mut expr.kind {
            ExprKind::Literal(_) | ExprKind::SelfValue | ExprKind::OwnerField { .. } => {}
            ExprKind::Name(name) => {
                if name.text == "std" {
                    self.fail(
                        name.span,
                        "\"std\" is not a value: name a declaration, e.g. std.io.stdout.Stdout()".into(),
                    );
                }
            }
            ExprKind::Grouping(inner)
            | ExprKind::Unary { operand: inner, .. }
            | ExprKind::Unpack { operand: inner, .. }
            | ExprKind::Panic { value: inner }
            | ExprKind::Await { value: inner } => self.expr(inner, false),
            ExprKind::Spawn { call } => self.expr(call, false),
            ExprKind::Binary { left, right, .. }
            | ExprKind::Range { start: left, end: right, .. }
            | ExprKind::StreamWrite { sink: left, source: right, .. } => {
                self.expr(left, false);
                self.expr(right, false);
            }
            ExprKind::Call { callee, args } => {
                self.expr(callee, true);
                self.args(args);
            }
            ExprKind::ArrayCast { args, .. } => self.args(args),
            ExprKind::Get { object, .. } | ExprKind::Select { object, .. } => self.expr(object, false),
            ExprKind::Index { object, index, fallback } => {
                self.expr(object, false);
                self.expr(index, false);
                if let Some(fallback) = fallback {
                    self.expr(fallback, false);
                }
            }
            ExprKind::Projection { construction, .. } => self.expr(construction, false),
            ExprKind::Functional { receiver, args, bindings, body, .. } => {
                self.expr(receiver, false);
                self.args(args);
                for binding in bindings.iter() {
                    self.binding(binding);
                }
                self.block(body);
            }
            ExprKind::AnonymousAction { params, result, body } => {
                self.params(params);
                self.action_result(result);
                self.block(body);
            }
            ExprKind::Array { elements } => {
                for element in elements {
                    self.expr(element, false);
                }
            }
            ExprKind::Map { entries } => {
                for entry in entries {
                    self.expr(&mut entry.key, false);
                    self.expr(&mut entry.value, false);
                }
            }
            ExprKind::If { condition, then_block, else_block } => {
                self.expr(condition, false);
                self.block(then_block);
                self.block(else_block);
            }
            ExprKind::Match(body) => self.match_body(body),
            ExprKind::Loop { body } => self.block(body),
            ExprKind::Try(body) => self.try_body(body),
        }
    }

    /// Переписать `expr`, если это путь `std.…`; `true` — путь найден
    /// (переписан или отвергнут).
    fn std_path(&mut self, expr: &mut Expr, called: bool) -> bool {
        // Сегменты цепочки `a.b.c` от корня, если её корень — имя `std`.
        let mut segments = Vec::new();
        let mut cursor = &*expr;
        loop {
            match &cursor.kind {
                ExprKind::Get { object, name } => {
                    segments.push(name.clone());
                    cursor = object;
                }
                ExprKind::Name(name) if name.text == "std" && !segments.is_empty() => {
                    segments.push(name.clone());
                    break;
                }
                _ => return false,
            }
        }
        segments.reverse();
        let path = &segments[1..];
        // Объявление: первый сегмент с заглавной буквы, иначе — вызываемая
        // native-функция (последний сегмент).
        let item = match path.iter().position(|segment| segment.text.starts_with(char::is_uppercase)) {
            Some(index) => index,
            None if called && path.len() >= 2 => path.len() - 1,
            None => {
                let shown: Vec<&str> = segments.iter().map(|segment| segment.text.as_str()).collect();
                self.fail(
                    expr.span,
                    format!(
                        "\"{}\" names a module, not a declaration: write e.g. std.io.stdout.Stdout",
                        shown.join(".")
                    ),
                );
                return true;
            }
        };
        if item == 0 {
            self.fail(expr.span, "A std path names a module before the declaration: std.io.stdout.Stdout".into());
            return true;
        }
        let space = self.module(&path[..item]);
        let start = segments[0].span.start;
        let mut rebuilt = Expr {
            kind: ExprKind::Name(ast::Name {
                text: space,
                span: Span::new(start, path[item - 1].span.end),
            }),
            span: Span::new(start, path[item - 1].span.end),
        };
        for name in &path[item..] {
            let span = Span::new(start, name.span.end);
            rebuilt = Expr {
                kind: ExprKind::Get {
                    object: Box::new(rebuilt),
                    name: name.clone(),
                },
                span,
            };
        }
        *expr = rebuilt;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::desugar;
    use crate::source::SourceFile;
    use crate::syntax::{self, ParseOptions, ast};

    fn program(body: &str) -> ast::Program {
        let source = SourceFile::new("t.ok", format!("module app v1.0.0;\n{body}\n")).unwrap();
        syntax::parse(&source, &ParseOptions::default()).unwrap()
    }

    fn imports(program: &ast::Program) -> Vec<(String, String)> {
        program
            .imports
            .iter()
            .map(|import| {
                let alias = import.alias.as_ref().map_or_else(String::new, |alias| alias.text.clone());
                (import.path.literal.value.clone(), alias)
            })
            .collect()
    }

    #[test]
    fn paths_become_one_hidden_import_per_module() {
        let mut found = program(
            "object Keep { has out is std.io.stdout.Stdout; Keep(:out); }\n\
             main() is int { let a = std.io.stdout.Stdout(); let o = std.compare.Order.less; \
             std.runtime.debug.data_dump(o); 0; }",
        );
        desugar(&mut found).unwrap();
        assert_eq!(
            imports(&found),
            [
                ("@std/io/stdout".to_string(), "std.io.stdout".to_string()),
                ("@std/compare".to_string(), "std.compare".to_string()),
                ("@std/runtime/debug".to_string(), "std.runtime.debug".to_string()),
            ]
        );
    }

    #[test]
    fn reserved_word_and_module_only_paths_are_errors() {
        for (body, message) in [
            ("main() is int { let std = 1; 0; }", "\"std\" is reserved"),
            ("main() is int { let m = std.io.stdout; 0; }", "names a module, not a declaration"),
            ("main() is int { let m = std; 0; }", "\"std\" is not a value"),
            ("main() is int { std.Stdout(); 0; }", "names a module before the declaration"),
        ] {
            let mut found = program(body);
            let error = desugar(&mut found).unwrap_err();
            assert!(error.message().contains(message), "{body}: {}", error.message());
        }
    }
}
