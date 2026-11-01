//! Бюджет синтаксического дерева (H-03): глубина и число узлов.
//!
//! Парсер ограничивает глубину своей рекурсии (`max_nesting`), но
//! левоассоциативные цепочки (`a + a + … + a`, `x.a.b.c…`) он строит циклом,
//! и дерево получается глубиной в длину цепочки. Анализ, lowering, принтеры
//! и форматтер обходят дерево рекурсивно, поэтому глубина дерева проверяется
//! здесь, сразу после разбора, обходом с явным стеком — до любого
//! рекурсивного потребителя. Глубина — число узлов выражений, операторов и
//! блоков на пути от объявления.

use crate::diagnostics::Diagnostic;
use crate::source::Span;
use crate::syntax::ast::{
    Block, ConfirmFallback, ConstructorForm, ElseBranch, Expr, ExprKind, FakeBody, IfStmt, Item,
    MatchBody, ObjectMember, Program, RoleMember, Stmt, StmtKind, TestBody, TryBody,
};

/// Наибольшие глубина и число узлов программы.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct TreeSize {
    pub depth: usize,
    pub nodes: usize,
}

/// Узел обхода.
#[derive(Copy, Clone)]
enum Node<'a> {
    Expr(&'a Expr),
    Stmt(&'a Stmt),
    Block(&'a Block),
    If(&'a IfStmt),
    Match(&'a MatchBody),
    Try(&'a TryBody),
}

impl Node<'_> {
    fn span(self) -> Span {
        match self {
            Node::Expr(expr) => expr.span,
            Node::Stmt(stmt) => stmt.span,
            Node::Block(block) => block.span,
            Node::If(stmt) => stmt.span,
            Node::Match(body) => body.span,
            Node::Try(body) => body.span,
        }
    }
}

/// Глубина и размер дерева; при превышении предела — место первого узла
/// сверх предела.
pub fn measure<'a>(program: &'a Program, max_depth: usize, max_nodes: usize) -> Result<TreeSize, (Span, TreeSize)> {
    let mut stack: Vec<(Node<'a>, usize)> = Vec::new();
    for item in &program.items {
        push_item(item, &mut stack);
    }
    let mut size = TreeSize::default();
    while let Some((node, depth)) = stack.pop() {
        size.nodes += 1;
        size.depth = size.depth.max(depth);
        if depth > max_depth || size.nodes > max_nodes {
            return Err((node.span(), size));
        }
        let next = depth + 1;
        let mut push = |child: Node<'a>| stack.push((child, next));
        match node {
            Node::Expr(expr) => expr_children(expr, &mut push),
            Node::Stmt(stmt) => stmt_children(stmt, &mut push),
            Node::Block(block) => block.stmts.iter().for_each(|stmt| push(Node::Stmt(stmt))),
            Node::If(stmt) => {
                push(Node::Expr(&stmt.condition));
                push(Node::Block(&stmt.then_block));
                match &stmt.else_branch {
                    Some(ElseBranch::Block(block)) => push(Node::Block(block)),
                    Some(ElseBranch::If(nested)) => push(Node::If(nested)),
                    None => {}
                }
            }
            Node::Match(body) => {
                push(Node::Expr(&body.subject));
                for case in &body.cases {
                    case.patterns.iter().for_each(|pattern| push(Node::Expr(pattern)));
                    push(Node::Block(&case.body));
                }
                if let Some(block) = &body.else_block {
                    push(Node::Block(block));
                }
            }
            Node::Try(body) => {
                push(Node::Block(&body.body));
                body.catches.iter().for_each(|catch| push(Node::Block(&catch.body)));
            }
        }
    }
    Ok(size)
}

/// Проверка дерева после разбора: синтаксическая ошибка сверх предела.
pub fn check(program: &Program, max_depth: usize, max_nodes: usize) -> Result<(), Diagnostic> {
    match measure(program, max_depth, max_nodes) {
        Ok(_) => Ok(()),
        Err((span, size)) => {
            let message = if size.nodes > max_nodes {
                format!("Program is too large (the limit is {max_nodes} syntax nodes).")
            } else {
                format!("Expression nesting is too deep (the limit is {max_depth} levels).")
            };
            Err(Diagnostic::syntax_limit(message, span))
        }
    }
}

fn push_item<'a>(item: &'a Item, stack: &mut Vec<(Node<'a>, usize)>) {
    let mut block = |block: &'a Block| stack.push((Node::Block(block), 1));
    match item {
        Item::Object(decl) => push_members(&decl.members, &mut block),
        Item::Role(decl) => {
            for member in &decl.members {
                match member {
                    RoleMember::Signature(_) => {}
                    RoleMember::Fake(fake) => {
                        if let FakeBody::Parsed(members) = &fake.body {
                            push_members(members, &mut block);
                        }
                    }
                    RoleMember::Test(test) => {
                        if let TestBody::Parsed(body) = &test.body {
                            block(body);
                        }
                    }
                }
            }
        }
        Item::Test(test) => {
            if let TestBody::Parsed(body) = &test.body {
                block(body);
            }
        }
        Item::Cap(cap) => cap.fields.iter().for_each(|field| block(&field.body)),
        Item::Action(action) => block(&action.body),
        Item::Enum(_) | Item::NativeAction(_) | Item::NativeObject(_) => {}
    }
    // Значения полей по умолчанию — выражения на глубине 1.
    if let Item::Object(decl) = item {
        push_defaults(&decl.members, stack);
    }
}

fn push_members<'a>(members: &'a [ObjectMember], block: &mut impl FnMut(&'a Block)) {
    for member in members {
        match member {
            ObjectMember::Has(_) => {}
            ObjectMember::Cap(cap) => cap.fields.iter().for_each(|field| block(&field.body)),
            ObjectMember::Constructor(ctor) => match &ctor.form {
                ConstructorForm::Full { body, .. } => block(body),
                ConstructorForm::Shorthand { body, .. } => {
                    if let Some(body) = body {
                        block(body);
                    }
                }
            },
            ObjectMember::Destroy(destroy) => block(&destroy.body),
            ObjectMember::Test(test) => {
                if let TestBody::Parsed(body) = &test.body {
                    block(body);
                }
            }
            ObjectMember::Action(action) => block(&action.body),
            ObjectMember::Private(section) => section.actions.iter().for_each(|action| block(&action.body)),
        }
    }
}

fn push_defaults<'a>(members: &'a [ObjectMember], stack: &mut Vec<(Node<'a>, usize)>) {
    for member in members {
        if let ObjectMember::Has(has) = member
            && let Some(default) = &has.default
        {
            stack.push((Node::Expr(default), 1));
        }
    }
}

fn expr_children<'a>(expr: &'a Expr, push: &mut impl FnMut(Node<'a>)) {
    let mut expr_node = |child: &'a Expr| push(Node::Expr(child));
    match &expr.kind {
        ExprKind::Grouping(operand)
        | ExprKind::Unary { operand, .. }
        | ExprKind::Unpack { operand, .. }
        | ExprKind::Get { object: operand, .. }
        | ExprKind::Select { object: operand, .. }
        | ExprKind::Projection { construction: operand, .. }
        | ExprKind::Panic { value: operand }
        | ExprKind::Await { value: operand }
        | ExprKind::Spawn { call: operand } => expr_node(operand),
        ExprKind::Binary { left, right, .. }
        | ExprKind::StreamWrite { sink: left, source: right, .. }
        | ExprKind::Range { start: left, end: right, .. } => {
            expr_node(left);
            expr_node(right);
        }
        ExprKind::Index { object, index, fallback } => {
            expr_node(object);
            expr_node(index);
            if let Some(fallback) = fallback {
                expr_node(fallback);
            }
        }
        ExprKind::Call { callee, args } => {
            expr_node(callee);
            args.iter().for_each(|arg| expr_node(&arg.value));
        }
        ExprKind::ArrayCast { args, .. } => args.iter().for_each(|arg| expr_node(&arg.value)),
        ExprKind::Functional { receiver, args, body, .. } => {
            expr_node(receiver);
            args.iter().for_each(|arg| expr_node(&arg.value));
            push(Node::Block(body));
        }
        ExprKind::AnonymousAction { body, .. } | ExprKind::Loop { body } => push(Node::Block(body)),
        ExprKind::Array { elements } => elements.iter().for_each(expr_node),
        ExprKind::Map { entries } => {
            for entry in entries {
                expr_node(&entry.key);
                expr_node(&entry.value);
            }
        }
        ExprKind::If { condition, then_block, else_block } => {
            expr_node(condition);
            push(Node::Block(then_block));
            push(Node::Block(else_block));
        }
        ExprKind::Match(body) => push(Node::Match(body)),
        ExprKind::Try(body) => push(Node::Try(body)),
        ExprKind::Literal(_) | ExprKind::Name(_) | ExprKind::SelfValue | ExprKind::OwnerField { .. } => {}
    }
}

fn stmt_children<'a>(stmt: &'a Stmt, push: &mut impl FnMut(Node<'a>)) {
    let optional = |value: &'a Option<Expr>, push: &mut dyn FnMut(Node<'a>)| {
        if let Some(value) = value {
            push(Node::Expr(value));
        }
    };
    match &stmt.kind {
        StmtKind::Let { value, .. }
        | StmtKind::Defer { value }
        | StmtKind::SuccessorAssign { value, .. }
        | StmtKind::FieldBind { value, .. }
        | StmtKind::SelfFieldBind { value, .. } => push(Node::Expr(value)),
        StmtKind::Throw { value } | StmtKind::Return { value } | StmtKind::Next { condition: value } => {
            optional(value, push)
        }
        StmtKind::Last { value, condition } => {
            optional(value, push);
            optional(condition, push);
        }
        StmtKind::Expr { expr, condition } => {
            push(Node::Expr(expr));
            optional(condition, push);
        }
        StmtKind::Confirm { condition, fallback } => {
            push(Node::Expr(condition));
            match fallback {
                Some(ConfirmFallback::Panic { value, .. } | ConfirmFallback::Throw { value, .. }) => {
                    push(Node::Expr(value))
                }
                None => {}
            }
        }
        StmtKind::Try(body) => push(Node::Try(body)),
        StmtKind::Block(block) => push(Node::Block(block)),
        StmtKind::If(stmt) => push(Node::If(stmt)),
        StmtKind::Match(body) => push(Node::Match(body)),
        StmtKind::For { source, body, .. } => {
            push(Node::Expr(source));
            push(Node::Block(body));
        }
    }
}
