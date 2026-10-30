//! Форматтер исходников Okkama (`okkama fmt`, форматирование в редакторе).
//!
//! Стиль — perlstyle: отступ 4 пробела, открывающая фигурная скобка на строке
//! конструкции, каждый оператор и член объекта на своей строке, `else` и
//! `catch` на новой строке после `}` («uncuddled elses»), пробелы вокруг
//! бинарных операторов и после запятых, пустая строка между объявлениями и
//! между членами объекта (подряд идущие `has` остаются вместе). Пустые строки
//! автора сохраняются (не больше одной подряд).
//!
//! Длинные списки (аргументы, параметры, литералы) переносятся по одному
//! элементу на строку, если не помещаются в [`WIDTH`] столбцов, если автор
//! уже разбил их на строки или поставил висячую запятую. Цепочки `&&`/`||`
//! переносятся перед оператором, вызовы через точку — там, где их перенёс
//! автор.
//!
//! Форматтер меняет только пробелы и переводы строк: токены остаются теми же
//! и в том же порядке, комментарии сохраняются (комментарий внутри выражения
//! может переехать на строку перед следующим оператором). Это проверяется
//! после форматирования: при расхождении — [`FormatError::Internal`], а не
//! испорченный файл.

use crate::diagnostics::Diagnostic;
use crate::source::{SourceFile, Span};
use crate::syntax::ast::*;
use crate::syntax::{ParseMode, ParseOptions, lexer, parse, token::TokenKind};

/// Ширина строки, после которой списки переносятся.
pub const WIDTH: usize = 100;
const INDENT: &str = "    ";

#[derive(Debug)]
pub enum FormatError {
    /// Исходник не разбирается: форматируется только корректный код.
    Syntax(Vec<Diagnostic>),
    /// Ошибка форматтера (результат изменил бы токены или потерял комментарий).
    Internal(String),
}

/// Отформатированный текст файла.
pub fn format(source: &SourceFile) -> Result<String, FormatError> {
    let options = ParseOptions {
        mode: ParseMode::Full,
        ..ParseOptions::default()
    };
    let program = parse(source, &options).map_err(FormatError::Syntax)?;
    let text = source.text();
    let mut printer = Printer::new(text, comments(text), WIDTH);
    printer.program(&program);
    let output = printer.finish();
    verify(text, &output)?;
    Ok(output)
}

/// Комментарий исходника.
#[derive(Clone, Debug)]
struct Comment {
    span: Span,
}

/// Комментарии лежат только в промежутках между токенами.
fn comments(text: &str) -> Vec<Comment> {
    let lexed = lexer::lex(text);
    let mut found = Vec::new();
    let mut previous = 0usize;
    for token in &lexed.tokens {
        let gap = &text[previous..token.span.start as usize];
        let mut index = 0;
        while index < gap.len() {
            let rest = &gap[index..];
            if rest.starts_with("//") {
                let end = rest.find('\n').unwrap_or(rest.len());
                found.push(Comment {
                    span: Span::new((previous + index) as u32, (previous + index + end) as u32),
                });
                index += end;
            } else if rest.starts_with("/*") {
                let end = rest.find("*/").map_or(rest.len(), |at| at + 2);
                found.push(Comment {
                    span: Span::new((previous + index) as u32, (previous + index + end) as u32),
                });
                index += end;
            } else {
                index += rest.chars().next().map_or(1, char::len_utf8);
            }
        }
        previous = token.span.end as usize;
    }
    found
}

/// Токены и комментарии результата совпадают с исходными.
fn verify(input: &str, output: &str) -> Result<(), FormatError> {
    let before = lexer::lex(input);
    let after = lexer::lex(output);
    if !after.diagnostics.is_empty() {
        return Err(FormatError::Internal(
            "formatted text does not lex".to_string(),
        ));
    }
    // `Atomic<int>=x` лексер склеивает в `>=`, а разбор внутри типа делит
    // обратно: составные токены из `<`, `>`, `=` сравниваются посимвольно.
    let tokens =
        |text: &str, tokens: &[crate::syntax::token::Token]| -> Vec<(Option<TokenKind>, String)> {
            let mut out = Vec::new();
            for token in tokens {
                let spelling = &text[token.span.range()];
                if spelling.chars().all(|ch| matches!(ch, '<' | '>' | '=')) {
                    out.extend(spelling.chars().map(|ch| (None, ch.to_string())));
                } else {
                    out.push((Some(token.kind), spelling.to_string()));
                }
            }
            out
        };
    let (left, right) = (tokens(input, &before.tokens), tokens(output, &after.tokens));
    if left != right {
        let at = left
            .iter()
            .zip(&right)
            .position(|(a, b)| a != b)
            .unwrap_or(left.len().min(right.len()));
        return Err(FormatError::Internal(format!(
            "formatting would change tokens at token {at}: {:?} became {:?}",
            left.get(at),
            right.get(at)
        )));
    }
    let texts = |text: &str| -> Vec<String> {
        let mut all: Vec<String> = comments(text)
            .iter()
            .map(|comment| text[comment.span.range()].trim_end().to_string())
            .collect();
        all.sort();
        all
    };
    if texts(input) != texts(output) {
        return Err(FormatError::Internal(
            "formatting would lose a comment".to_string(),
        ));
    }
    Ok(())
}

struct Printer<'s> {
    text: &'s str,
    out: String,
    indent: usize,
    /// Отступ текущего оператора: продолжения строк — на один глубже.
    stmt_indent: usize,
    /// Следующее слово начинает строку (отступ ещё не выведен).
    line_start: bool,
    comments: Vec<Comment>,
    next_comment: usize,
    width: usize,
}

impl<'s> Printer<'s> {
    fn new(text: &'s str, comments: Vec<Comment>, width: usize) -> Self {
        Self {
            text,
            out: String::new(),
            indent: 0,
            stmt_indent: 0,
            line_start: true,
            comments,
            next_comment: 0,
            width,
        }
    }

    fn finish(mut self) -> String {
        let _ = self.leading_comments(self.text.len() as u32, false);
        let mut out = self.out.trim_end().to_string();
        out.push('\n');
        out
    }

    // ---- вывод ----

    fn word(&mut self, text: &str) {
        if self.line_start {
            for _ in 0..self.indent {
                self.out.push_str(INDENT);
            }
            self.line_start = false;
        }
        self.out.push_str(text);
    }

    fn newline(&mut self) {
        if self.line_start {
            return;
        }
        while self.out.ends_with(' ') {
            self.out.pop();
        }
        self.out.push('\n');
        self.line_start = true;
    }

    fn blank_line(&mut self) {
        self.newline();
        if !self.out.is_empty() && !self.out.ends_with("\n\n") && !self.out.ends_with("{\n") {
            self.out.push('\n');
        }
    }

    /// Перенос внутри оператора: продолжение на один уровень глубже оператора.
    fn continuation(&mut self) {
        self.newline();
        self.indent = self.indent.max(self.stmt_indent + 1);
    }

    fn column(&self) -> usize {
        if self.line_start {
            return self.indent * INDENT.len();
        }
        let line = self.out.rsplit('\n').next().unwrap_or("");
        line.chars().count()
    }

    fn slice(&self, span: Span) -> &'s str {
        &self.text[span.range()]
    }

    // ---- исходник ----

    /// Между `from` и `to` в исходнике есть перевод строки (вне комментариев
    /// не различаем: комментарий тоже кончается переводом строки).
    fn broken(&self, from: u32, to: u32) -> bool {
        from < to && self.text[from as usize..to as usize].contains('\n')
    }

    /// Перед `offset` (через пробелы) — пустая строка.
    fn blank_before(&self, offset: u32) -> bool {
        let before = &self.text[..offset as usize];
        let gap = &before[before.trim_end().len()..];
        gap.matches('\n').count() >= 2
    }

    /// В исходнике между `from` и `to` есть запятая (висячая запятая списка).
    fn comma_between(&self, from: u32, to: u32) -> bool {
        from < to
            && self.text[from as usize..to as usize]
                .trim_start()
                .starts_with(',')
    }

    fn has_comment_in(&self, span: Span) -> bool {
        self.comments[self.next_comment..]
            .iter()
            .any(|comment| span.start <= comment.span.start && comment.span.end <= span.end)
    }

    // ---- комментарии ----

    /// Комментарии до `offset` — каждый на своей строке. `first` — первый
    /// элемент блока: без пустой строки перед ним.
    fn leading_comments(&mut self, offset: u32, first: bool) -> bool {
        let mut first = first;
        let mut printed = false;
        while let Some(comment) = self.comments.get(self.next_comment).cloned() {
            if comment.span.start >= offset {
                break;
            }
            self.next_comment += 1;
            self.newline();
            if !first && self.blank_before(comment.span.start) {
                self.blank_line();
            }
            first = false;
            let text = self.slice(comment.span).trim_end();
            self.word(text);
            self.newline();
            printed = true;
        }
        printed
    }

    /// Комментарий на той же строке после `end`.
    fn trailing_comment(&mut self, end: u32) {
        let Some(comment) = self.comments.get(self.next_comment).cloned() else {
            return;
        };
        if comment.span.start >= end && !self.broken(end, comment.span.start) {
            self.next_comment += 1;
            self.word(" ");
            let text = self.slice(comment.span).trim_end();
            self.word(text);
        }
    }

    /// Элемент на своей строке (оператор, член, объявление).
    fn line(&mut self, span: Span, first: bool, blank: bool, print: impl FnOnce(&mut Self)) {
        // Обязательная пустая строка — перед комментариями элемента: они
        // документируют его и остаются над ним.
        if !first && blank {
            self.newline();
            self.blank_line();
        }
        let commented = self.leading_comments(span.start, first);
        self.newline();
        let first = first && !commented;
        if !first && self.blank_before(span.start) {
            self.blank_line();
        }
        let saved = (self.indent, self.stmt_indent);
        self.stmt_indent = self.indent;
        print(self);
        self.indent = saved.0;
        self.stmt_indent = saved.1;
        self.trailing_comment(span.end);
        self.newline();
    }

    /// Ширина первой строки `print`.
    fn first_line(&self, print: impl FnOnce(&mut Printer<'s>)) -> usize {
        let mut scratch = Printer::new(self.text, Vec::new(), usize::MAX);
        scratch.line_start = false;
        print(&mut scratch);
        scratch
            .out
            .split('\n')
            .next()
            .map_or(0, |line| line.chars().count())
    }

    /// Ширина `print` в одну строку (`None` — многострочное).
    fn flat(&self, print: impl FnOnce(&mut Printer<'s>)) -> Option<usize> {
        let mut scratch = Printer::new(self.text, Vec::new(), usize::MAX);
        scratch.line_start = false;
        print(&mut scratch);
        (!scratch.out.contains('\n')).then(|| scratch.out.chars().count())
    }

    // ---- файл ----

    fn program(&mut self, program: &Program) {
        self.line(program.module.span, true, false, |p| {
            p.word("module ");
            let path: Vec<&str> = program
                .module
                .path
                .iter()
                .map(|part| part.text.as_str())
                .collect();
            p.word(&path.join("/"));
            if let Some(version) = &program.module.version {
                p.word(" ");
                p.word(&version.text());
            }
            p.word(";");
        });
        for (index, import) in program.imports.iter().enumerate() {
            self.line(import.span, false, index == 0, |p| p.import(import));
        }
        for item in &program.items {
            self.line(item.span(), false, true, |p| p.item(item));
        }
    }

    fn import(&mut self, import: &ImportDecl) {
        self.word("import ");
        self.word(self.slice(import.path.span));
        if let Some(alias) = &import.alias {
            self.word(" as ");
            self.word(&alias.text);
        }
        if !import.take.is_empty() {
            self.word(" take ");
            for (index, name) in import.take.iter().enumerate() {
                if index > 0 {
                    self.word(",");
                    if self.column() + 1 + name.text.len() + 1 > self.width {
                        self.continuation();
                    } else {
                        self.word(" ");
                    }
                }
                self.word(&name.text);
            }
        }
        self.word(";");
    }

    fn item(&mut self, item: &Item) {
        match item {
            Item::Object(decl) => self.object(decl),
            Item::Role(decl) => self.role(decl),
            Item::Enum(decl) => {
                self.word("enum ");
                self.word(&decl.name.text);
                self.word(" = ");
                let close = decl.span.end;
                self.list(
                    "(",
                    ")",
                    &decl.cases,
                    close,
                    |p, case| p.word(&case.text),
                    |case| case.span,
                );
                self.word(";");
            }
            Item::Test(test) => self.test(test),
            Item::NativeAction(decl) => {
                self.word("native ");
                if decl.is_async {
                    self.word("async ");
                }
                self.word(&decl.name.text);
                self.params(&decl.params, decl.span.end);
                self.result(&decl.result);
                self.word(";");
            }
            Item::NativeObject(decl) => {
                self.word("native object ");
                self.word(&decl.name.text);
                self.word(";");
            }
            Item::Cap(decl) => self.cap(decl),
            Item::Action(decl) => self.action(decl),
        }
    }

    fn generics(&mut self, generics: &[GenericParam]) {
        if generics.is_empty() {
            return;
        }
        self.word("<");
        for (index, param) in generics.iter().enumerate() {
            if index > 0 {
                self.word(", ");
            }
            self.word(&param.name.text);
            if let Some(bound) = &param.bound {
                self.word(": ");
                self.ty(bound);
            }
        }
        let last = generics[generics.len() - 1].span.end;
        if self.text[last as usize..].trim_start().starts_with(',') {
            self.word(",");
        }
        self.word(">");
    }

    fn object(&mut self, decl: &ObjectDecl) {
        self.word("object ");
        self.word(&decl.name.text);
        self.generics(&decl.generics);
        if !decl.roles.is_empty() {
            self.word(" does ");
            for (index, role) in decl.roles.iter().enumerate() {
                if index > 0 {
                    self.word(", ");
                }
                self.ty(role);
            }
        }
        self.word(" ");
        self.members(&decl.members, decl.span.end);
    }

    /// Тело объекта или fake: члены через пустую строку, кроме подряд идущих `has`.
    fn members(&mut self, members: &[ObjectMember], end: u32) {
        self.word("{");
        if members.is_empty() && !self.has_comment_in(Span::new(self.offset_before(end), end)) {
            self.word("}");
            return;
        }
        self.indent += 1;
        let mut previous_has = false;
        for (index, member) in members.iter().enumerate() {
            let is_has = matches!(member, ObjectMember::Has(_));
            let blank = index > 0 && !(is_has && previous_has);
            self.line(member.span(), index == 0, blank, |p| p.member(member));
            previous_has = is_has;
        }
        self.leading_comments(end.saturating_sub(1), members.is_empty());
        self.newline();
        self.indent -= 1;
        self.word("}");
    }

    /// Начало содержимого блока `{ ... }`, кончающегося в `end`.
    fn offset_before(&self, end: u32) -> u32 {
        self.text[..end as usize]
            .rfind('{')
            .map_or(0, |at| at as u32)
    }

    fn member(&mut self, member: &ObjectMember) {
        match member {
            ObjectMember::Has(has) => {
                self.word("has ");
                let names: Vec<&str> = has.names.iter().map(|name| name.text.as_str()).collect();
                self.word(&names.join(", "));
                if let Some(ty) = &has.ty {
                    self.word(" is ");
                    self.ty(ty);
                }
                if let Some(default) = &has.default {
                    self.word(" = ");
                    self.expr(default);
                }
                self.word(";");
            }
            ObjectMember::Cap(cap) => self.cap(cap),
            ObjectMember::Constructor(ctor) => {
                self.word(&ctor.name.text);
                match &ctor.form {
                    ConstructorForm::Full { params, body } => {
                        self.params(params, body.span.start);
                        self.word(" ");
                        self.block(body);
                    }
                    ConstructorForm::Shorthand { fields, body } => {
                        let close = body.as_ref().map_or(ctor.span.end, |body| body.span.start);
                        self.list(
                            "(",
                            ")",
                            fields,
                            close,
                            |p, field| {
                                p.word(":");
                                p.word(&field.text);
                            },
                            |field| Span::new(field.span.start.saturating_sub(1), field.span.end),
                        );
                        match body {
                            Some(body) => {
                                self.word(" ");
                                self.block(body);
                            }
                            None => self.word(";"),
                        }
                    }
                }
            }
            ObjectMember::Destroy(destroy) => {
                self.word("destroy ");
                self.block(&destroy.body);
            }
            ObjectMember::Test(test) => self.test(test),
            ObjectMember::Action(action) => self.action(action),
            ObjectMember::Private(section) => {
                self.word("private:");
                for action in &section.actions {
                    self.newline();
                    self.blank_line();
                    let span = action
                        .projection
                        .as_ref()
                        .map_or(action.span, |projection| projection.span.to(action.span));
                    self.leading_comments(span.start, true);
                    self.action(action);
                    self.trailing_comment(action.span.end);
                }
            }
        }
    }

    fn cap(&mut self, cap: &CapDecl) {
        self.word("cap ");
        if let Some(owner) = &cap.owner {
            self.word(&owner.text);
            self.word("::");
        }
        self.word(&cap.name.text);
        self.word(" {");
        if cap.fields.is_empty() {
            self.word("}");
            return;
        }
        self.indent += 1;
        for (index, field) in cap.fields.iter().enumerate() {
            self.line(field.span, index == 0, index > 0, |p| {
                p.word(&field.name.text);
                p.word(" is ");
                p.ty(&field.ty);
                p.word(" ");
                p.block(&field.body);
            });
        }
        self.leading_comments(cap.span.end.saturating_sub(1), false);
        self.newline();
        self.indent -= 1;
        self.word("}");
    }

    fn test(&mut self, test: &TestDecl) {
        if test.is_async {
            self.word("async ");
        }
        self.word("test ");
        self.word(self.slice(test.name.span));
        self.word(" ");
        match &test.body {
            TestBody::Parsed(block) => self.block(block),
            TestBody::Skipped(span) => self.word(self.slice(*span)),
        }
    }

    fn action(&mut self, action: &ActionDecl) {
        if let Some(projection) = &action.projection {
            self.word("#projected");
            if let Some(alias) = &projection.alias {
                self.word("(");
                self.word(&alias.text);
                self.word(")");
            }
            self.newline();
        }
        if action.is_async {
            self.word("async ");
        }
        self.word(&action.name.text);
        self.params(&action.params, action.body.span.start);
        self.result(&action.result);
        self.word(" ");
        self.block(&action.body);
    }

    fn role(&mut self, decl: &RoleDecl) {
        self.word("role ");
        self.word(&decl.name.text);
        self.generics(&decl.generics);
        self.word(" {");
        if decl.members.is_empty() {
            self.word("}");
            return;
        }
        self.indent += 1;
        let mut previous_signature = false;
        for (index, member) in decl.members.iter().enumerate() {
            let signature = matches!(member, RoleMember::Signature(_));
            let blank = index > 0 && !(signature && previous_signature);
            self.line(member.span(), index == 0, blank, |p| match member {
                RoleMember::Signature(signature) => {
                    if signature.is_async {
                        p.word("async ");
                    }
                    p.word(&signature.name.text);
                    p.params(&signature.params, signature.span.end);
                    p.result(&signature.result);
                    p.word(";");
                }
                RoleMember::Fake(fake) => {
                    p.word("fake ");
                    match &fake.body {
                        FakeBody::Parsed(members) => p.members(members, fake.span.end),
                        FakeBody::Skipped(span) => p.word(p.slice(*span)),
                    }
                }
                RoleMember::Test(test) => p.test(test),
            });
            previous_signature = signature;
        }
        self.leading_comments(decl.span.end.saturating_sub(1), false);
        self.newline();
        self.indent -= 1;
        self.word("}");
    }

    fn params(&mut self, params: &[Param], close: u32) {
        self.list(
            "(",
            ")",
            params,
            close,
            |p, param| {
                p.word(&param.name.text);
                p.word(" is ");
                if param.variadic {
                    p.word("...");
                }
                p.ty(&param.ty);
            },
            |param| param.span,
        );
    }

    fn result(&mut self, result: &ResultSpec) {
        self.word(" is ");
        match result {
            ResultSpec::Void(_) => self.word("void"),
            ResultSpec::Successor { ty, .. } => {
                self.word("^");
                self.ty(ty);
            }
            ResultSpec::Type(ty) => self.ty(ty),
        }
    }

    fn ty(&mut self, ty: &TypeExpr) {
        match &ty.kind {
            TypeKind::Named { path, args } => {
                let name: Vec<&str> = path.iter().map(|part| part.text.as_str()).collect();
                self.word(&name.join("."));
                if !args.is_empty() {
                    self.word("<");
                    for (index, arg) in args.iter().enumerate() {
                        if index > 0 {
                            self.word(", ");
                        }
                        self.ty(arg);
                    }
                    self.word(">");
                }
            }
            TypeKind::Nested { owner, name } => {
                self.ty(owner);
                self.word("::");
                self.word(&name.text);
            }
            TypeKind::Array(element) => {
                self.word("@");
                self.ty(element);
            }
            TypeKind::Map { key, value } => {
                self.word("%");
                self.ty(key);
                self.word(":");
                self.ty(value);
            }
            TypeKind::Action {
                is_async,
                params,
                result,
            } => {
                if *is_async {
                    self.word("async ");
                }
                self.word("(");
                for (index, param) in params.iter().enumerate() {
                    if index > 0 {
                        self.word(", ");
                    }
                    self.ty(param);
                }
                self.word(") is ");
                match result {
                    ActionResult::Void(_) => self.word("void"),
                    ActionResult::Type(ty) => self.ty(ty),
                }
            }
        }
    }

    /// Список в скобках: в строку или по элементу на строке (не помещается,
    /// автор разбил, висячая запятая, комментарии внутри). Последний
    /// многострочный элемент (блок, closure) остаётся «прижатым»: `f(a, b -> {`.
    /// `close` — смещение закрывающей скобки или позже (висячая запятая
    /// ищется между последним элементом и ним).
    fn list<T>(
        &mut self,
        open: &str,
        close: &str,
        items: &[T],
        end: u32,
        print: impl Fn(&mut Self, &T),
        span: impl Fn(&T) -> Span,
    ) {
        if items.is_empty() {
            self.word(open);
            self.word(close);
            return;
        }
        let first = span(&items[0]);
        let last = span(&items[items.len() - 1]);
        let start = self.text[..first.start as usize]
            .rfind(open)
            .map_or(first.start, |at| at as u32);
        let trailing = self.comma_between(last.end, end);
        let mut broken = trailing || self.has_comment_in(Span::new(start, end));
        for pair in items.windows(2) {
            broken |= self.broken(span(&pair[0]).end, span(&pair[1]).start);
        }
        broken |= self.broken(start, first.start);
        let mut widths = Vec::new();
        if !broken {
            for item in items {
                widths.push(self.flat(|p| print(p, item)));
            }
            let hug = widths[..widths.len() - 1].iter().all(Option::is_some)
                && widths[widths.len() - 1].is_none();
            if hug {
                // Прижатый последний элемент: первая строка целиком должна помещаться.
                let before: usize = widths[..widths.len() - 1]
                    .iter()
                    .flatten()
                    .map(|width| width + 2)
                    .sum();
                let head = self.first_line(|p| print(p, &items[items.len() - 1]));
                broken = self.column() + open.len() + before + head > self.width;
            } else {
                match widths.iter().copied().collect::<Option<Vec<usize>>>() {
                    Some(widths) => {
                        let total: usize = widths.iter().sum::<usize>()
                            + 2 * (widths.len() - 1)
                            + open.len()
                            + close.len();
                        broken = self.column() + total > self.width;
                    }
                    None => broken = true,
                }
            }
        }
        self.word(open);
        if !broken {
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    self.word(", ");
                }
                print(self, item);
            }
            self.word(close);
            return;
        }
        let saved = (self.indent, self.stmt_indent);
        self.indent += 1;
        self.stmt_indent = self.indent;
        for (index, item) in items.iter().enumerate() {
            self.leading_comments(span(item).start, index == 0);
            self.newline();
            print(self, item);
            if index + 1 < items.len() || trailing {
                self.word(",");
            }
            let next = items.get(index + 1).map_or(end, |next| span(next).start);
            self.trailing_comment_before(span(item).end, next);
        }
        self.leading_comments(end.saturating_sub(1), false);
        self.newline();
        self.indent = saved.0;
        self.stmt_indent = saved.1;
        self.word(close);
    }

    /// Комментарий в конце строки элемента списка (после его запятой).
    fn trailing_comment_before(&mut self, end: u32, limit: u32) {
        if let Some(comment) = self.comments.get(self.next_comment) {
            if comment.span.start < limit {
                self.trailing_comment(end);
            }
        }
    }

    // ---- блоки и операторы ----

    fn block(&mut self, block: &Block) {
        self.word("{");
        if block.stmts.is_empty() && !self.has_comment_in(block.span) {
            self.word("}");
            return;
        }
        self.indent += 1;
        for (index, stmt) in block.stmts.iter().enumerate() {
            self.line(stmt.span, index == 0, false, |p| p.stmt(stmt));
        }
        self.leading_comments(block.span.end.saturating_sub(1), block.stmts.is_empty());
        self.newline();
        self.indent -= 1;
        self.word("}");
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Let { name, ty, value } => {
                self.word("let ");
                self.word(&name.text);
                if let Some(ty) = ty {
                    self.word(" is ");
                    self.ty(ty);
                }
                self.word(" = ");
                self.expr(value);
                self.word(";");
            }
            StmtKind::Defer { value } => {
                self.word("defer ");
                self.expr(value);
                self.word(";");
            }
            StmtKind::Try(body) => self.try_body(body),
            StmtKind::Throw { value } => {
                self.word("throw");
                if let Some(value) = value {
                    self.word(" ");
                    self.expr(value);
                }
                self.word(";");
            }
            StmtKind::Return { value } => {
                self.word("return");
                if let Some(value) = value {
                    self.word(" ");
                    self.expr(value);
                }
                self.word(";");
            }
            StmtKind::Confirm {
                condition,
                fallback,
            } => {
                self.word("confirm ");
                let lines = self.out.matches('\n').count();
                self.expr(condition);
                let multiline = self.out.matches('\n').count() > lines;
                if let Some(fallback) = fallback {
                    let (keyword, word, value) = match fallback {
                        ConfirmFallback::Panic { keyword, value } => (*keyword, "panic ", value),
                        ConfirmFallback::Throw { keyword, value } => (*keyword, "throw ", value),
                    };
                    let tail = self.flat(|p| {
                        p.word(" else ");
                        p.word(word);
                        p.expr(value);
                        p.word(";");
                    });
                    let wide = tail.is_none_or(|width| self.column() + width > self.width);
                    if multiline || self.broken(condition.span.end, keyword.start) || wide {
                        self.continuation();
                        self.word("else ");
                    } else {
                        self.word(" else ");
                    }
                    self.word(word);
                    self.expr(value);
                }
                self.word(";");
            }
            StmtKind::SuccessorAssign { name, value } => {
                self.word("^");
                self.word(&name.text);
                self.word(" = ");
                self.expr(value);
                self.word(";");
            }
            StmtKind::Block(block) => self.block(block),
            StmtKind::If(found) => self.if_stmt(found),
            StmtKind::Match(body) => self.match_body(body),
            StmtKind::For {
                source,
                bindings,
                body,
            } => {
                self.word("for ");
                self.expr(source);
                self.word(" -> ");
                for (index, binding) in bindings.iter().enumerate() {
                    if index > 0 {
                        self.word(", ");
                    }
                    self.word(&binding.name.text);
                    if let Some(batch) = &binding.batch {
                        self.word("[");
                        self.word(self.slice(batch.span));
                        self.word("]");
                    }
                }
                self.word(" ");
                self.block(body);
            }
            StmtKind::Next { condition } => {
                self.word("next");
                self.postfix_if(condition.as_ref());
                self.word(";");
            }
            StmtKind::Last { value, condition } => {
                self.word("last");
                if let Some(value) = value {
                    self.word(" ");
                    self.expr(value);
                }
                self.postfix_if(condition.as_ref());
                self.word(";");
            }
            StmtKind::FieldBind { name, value } => {
                self.word(&name.text);
                self.word(" = ");
                self.expr(value);
                self.word(";");
            }
            StmtKind::SelfFieldBind { name, value, .. } => {
                self.word("self.");
                self.word(&name.text);
                self.word(" = ");
                self.expr(value);
                self.word(";");
            }
            StmtKind::Expr { expr, condition } => {
                self.expr(expr);
                self.postfix_if(condition.as_ref());
                // `;` после оператора-`loop` необязательна: как у автора.
                let loop_without = matches!(expr.kind, ExprKind::Loop { .. })
                    && !self.slice(stmt.span).trim_end().ends_with(';');
                if !loop_without {
                    self.word(";");
                }
            }
        }
    }

    fn postfix_if(&mut self, condition: Option<&Expr>) {
        if let Some(condition) = condition {
            self.word(" if ");
            self.expr(condition);
        }
    }

    fn if_stmt(&mut self, found: &IfStmt) {
        self.word("if ");
        self.expr(&found.condition);
        self.word(" ");
        self.block(&found.then_block);
        match &found.else_branch {
            None => {}
            Some(ElseBranch::Block(block)) => {
                self.else_line(block.span.start);
                self.word("else ");
                self.block(block);
            }
            Some(ElseBranch::If(next)) => {
                self.else_line(next.span.start);
                self.word("else ");
                self.if_stmt(next);
            }
        }
    }

    /// `}` и `else`/`catch` — на разных строках; комментарии перед ними — на своих.
    fn else_line(&mut self, before: u32) {
        self.newline();
        self.leading_comments(before, true);
    }

    fn match_body(&mut self, body: &MatchBody) {
        self.word("match ");
        self.expr(&body.subject);
        self.word(" {");
        self.indent += 1;
        for (index, case) in body.cases.iter().enumerate() {
            self.line(case.span, index == 0, false, |p| {
                p.word("case ");
                for (index, pattern) in case.patterns.iter().enumerate() {
                    if index > 0 {
                        p.word(", ");
                    }
                    p.expr(pattern);
                }
                p.word(" ");
                p.block(&case.body);
            });
        }
        if let Some(block) = &body.else_block {
            let span = Span::new(self.else_keyword(block.span.start), block.span.end);
            self.line(span, body.cases.is_empty(), false, |p| {
                p.word("else ");
                p.block(block);
            });
        }
        self.leading_comments(body.span.end.saturating_sub(1), false);
        self.newline();
        self.indent -= 1;
        self.word("}");
    }

    /// Начало `else` перед блоком, начинающимся в `block`.
    fn else_keyword(&self, block: u32) -> u32 {
        self.text[..block as usize]
            .rfind("else")
            .map_or(block, |at| at as u32)
    }

    fn try_body(&mut self, body: &TryBody) {
        self.word("try ");
        self.block(&body.body);
        for catch in &body.catches {
            self.else_line(catch.span.start);
            self.word("catch ");
            self.word(&catch.name.text);
            self.word(" is ");
            self.ty(&catch.ty);
            self.word(" ");
            self.block(&catch.body);
        }
    }

    // ---- выражения ----

    fn expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Literal(_) => self.word(self.slice(expr.span)),
            ExprKind::Name(name) => self.word(&name.text),
            ExprKind::SelfValue => self.word("self"),
            ExprKind::Grouping(inner) => {
                self.word("(");
                self.expr(inner);
                self.word(")");
            }
            ExprKind::Unary { op, operand, .. } => {
                self.word(op.symbol());
                // `- -x`, `- -1`: не склеивать в другой токен.
                let operand_text = self.slice(operand.span);
                if *op == UnaryOp::Negate && operand_text.starts_with('-') {
                    self.word(" ");
                }
                self.expr(operand);
            }
            ExprKind::Binary {
                op,
                left,
                right,
                op_span,
            } => {
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    self.logical_chain(expr);
                } else {
                    self.expr(left);
                    self.word(" ");
                    self.word(op.symbol());
                    if self.broken(left.span.end, op_span.start)
                        || self.broken(op_span.end, right.span.start)
                    {
                        self.continuation();
                    } else {
                        self.word(" ");
                    }
                    self.expr(right);
                }
            }
            ExprKind::StreamWrite {
                sink,
                source,
                op_span,
            } => {
                self.expr(sink);
                self.word(" <-");
                if self.broken(op_span.end, source.span.start) {
                    self.continuation();
                } else {
                    self.word(" ");
                }
                self.expr(source);
            }
            ExprKind::Range {
                inclusive,
                start,
                end,
                ..
            } => {
                self.expr(start);
                self.word(if *inclusive { "..=" } else { ".." });
                self.expr(end);
            }
            ExprKind::Call { callee, args } => {
                self.expr(callee);
                self.args(args, expr.span.end);
            }
            ExprKind::ArrayCast { target, args, .. } => {
                self.word("@");
                self.word(&target.text);
                self.args(args, expr.span.end);
            }
            ExprKind::Get { object, name } => {
                self.expr(object);
                if self.broken(object.span.end, name.span.start) {
                    self.continuation();
                }
                self.word(".");
                self.word(&name.text);
            }
            ExprKind::Select { object, name } => {
                self.expr(object);
                self.word("::");
                self.word(&name.text);
            }
            ExprKind::OwnerField { owner, name } => {
                self.word(&owner.text);
                self.word("::");
                self.word(&name.text);
            }
            ExprKind::Index {
                object,
                index,
                fallback,
            } => {
                self.expr(object);
                self.word("[");
                self.expr(index);
                self.word("]");
                if let Some(fallback) = fallback {
                    self.word(" else ");
                    self.expr(fallback);
                }
            }
            ExprKind::Unpack { operand, .. } => {
                self.expr(operand);
                self.word("...");
            }
            ExprKind::Projection {
                prefix,
                construction,
            } => {
                self.word(&prefix.text);
                self.word(" ");
                self.expr(construction);
            }
            ExprKind::Functional {
                operation_name,
                receiver,
                args,
                bindings,
                body,
                ..
            } => {
                self.expr(receiver);
                if self.broken(receiver.span.end, operation_name.span.start) {
                    self.continuation();
                }
                self.word(".");
                self.word(&operation_name.text);
                self.args(args, body.span.start);
                self.word(" ->");
                for (index, binding) in bindings.iter().enumerate() {
                    self.word(if index > 0 { ", " } else { " " });
                    self.word(&binding.text);
                }
                self.word(" ");
                self.block(body);
            }
            ExprKind::AnonymousAction {
                params,
                result,
                body,
            } => {
                self.params(params, body.span.start);
                self.word(" is ");
                match result {
                    ActionResult::Void(_) => self.word("void"),
                    ActionResult::Type(ty) => self.ty(ty),
                }
                self.word(" ");
                self.block(body);
            }
            ExprKind::Array { elements } => {
                self.list(
                    "[",
                    "]",
                    elements,
                    expr.span.end,
                    |p, element| p.expr(element),
                    |element| element.span,
                );
            }
            ExprKind::Map { entries } => {
                if entries.is_empty() {
                    self.word("[:]");
                } else {
                    self.list(
                        "[",
                        "]",
                        entries,
                        expr.span.end,
                        |p, entry| {
                            p.expr(&entry.key);
                            p.word(": ");
                            p.expr(&entry.value);
                        },
                        |entry| entry.span,
                    );
                }
            }
            ExprKind::If {
                condition,
                then_block,
                else_block,
            } => {
                self.word("if ");
                self.expr(condition);
                self.word(" ");
                self.block(then_block);
                self.else_line(else_block.span.start);
                self.word("else ");
                self.block(else_block);
            }
            ExprKind::Match(body) => self.match_body(body),
            ExprKind::Loop { body } => {
                self.word("loop ");
                self.block(body);
            }
            ExprKind::Try(body) => self.try_body(body),
            ExprKind::Panic { value } => {
                self.word("panic ");
                self.expr(value);
            }
            ExprKind::Await { value } => {
                self.word("await ");
                self.expr(value);
            }
            ExprKind::Spawn { call } => {
                self.word("spawn ");
                self.expr(call);
            }
        }
    }

    fn args(&mut self, args: &[Argument], end: u32) {
        self.list(
            "(",
            ")",
            args,
            end,
            |p, arg| {
                if let Some(name) = &arg.name {
                    p.word(&name.text);
                    p.word(": ");
                }
                p.expr(&arg.value);
            },
            |arg| arg.span,
        );
    }

    /// `a && b || c`: в строку, если помещается и автор не переносил; иначе
    /// перенос перед каждым `&&`/`||` цепочки одного оператора.
    fn logical_chain(&mut self, expr: &Expr) {
        let ExprKind::Binary { op, .. } = &expr.kind else {
            return self.expr(expr);
        };
        let mut operands = Vec::new();
        let mut spans = Vec::new();
        collect_chain(expr, *op, &mut operands, &mut spans);
        let user_broken = operands
            .windows(2)
            .any(|pair| self.broken(pair[0].span.end, pair[1].span.start));
        let flat = self.flat(|p| {
            for (index, operand) in operands.iter().enumerate() {
                if index > 0 {
                    p.word(" ");
                    p.word(op.symbol());
                    p.word(" ");
                }
                p.expr(operand);
            }
        });
        let fits = flat.is_some_and(|width| self.column() + width <= self.width);
        for (index, operand) in operands.iter().enumerate() {
            if index > 0 {
                if user_broken || !fits {
                    self.continuation();
                } else {
                    self.word(" ");
                }
                self.word(op.symbol());
                self.word(" ");
            }
            self.expr(operand);
        }
    }
}

/// Операнды левоассоциативной цепочки одного логического оператора.
fn collect_chain<'e>(expr: &'e Expr, op: BinaryOp, out: &mut Vec<&'e Expr>, spans: &mut Vec<Span>) {
    match &expr.kind {
        ExprKind::Binary {
            op: inner,
            left,
            right,
            op_span,
        } if *inner == op => {
            collect_chain(left, op, out, spans);
            spans.push(*op_span);
            out.push(right);
        }
        _ => out.push(expr),
    }
}

#[cfg(test)]
mod tests;
