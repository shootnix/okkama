//! Conversion of the typed AST into the printer [`Node`] description.
//!
//! Every function matches its AST type exhaustively (no `_` arms), so the
//! compiler flags any node kind the printers would otherwise miss.

use super::{Node, Value};
use crate::source::Span;
use crate::syntax::ast::*;
use crate::syntax::token::{IntLiteral, StrLiteral, StringForm};

/// The printer description of a whole program.
pub fn program_node(program: &Program) -> Node {
    Node::new("Program", program.span)
        .field("module", Value::node(module_node(&program.module)))
        .field("imports", list(&program.imports, import_node))
        .field("items", list(&program.items, item_node))
}

fn list<T>(items: &[T], f: impl Fn(&T) -> Node) -> Value {
    Value::List(items.iter().map(|item| Value::node(f(item))).collect())
}

fn name(name: &Name) -> Value {
    Value::Name {
        text: name.text.clone(),
        span: name.span,
    }
}

fn names(items: &[Name]) -> Value {
    Value::List(items.iter().map(name).collect())
}

fn opt_name(item: Option<&Name>) -> Value {
    item.map_or(Value::Absent, name)
}

fn opt_node<T>(item: Option<&T>, f: impl Fn(&T) -> Node) -> Value {
    item.map_or(Value::Absent, |item| Value::node(f(item)))
}

fn text(value: &str) -> Value {
    Value::Text(value.to_string())
}

fn module_node(module: &ModuleDecl) -> Node {
    Node::new("ModuleDecl", module.span)
        .field("path", names(&module.path))
        .field(
            "version",
            module.version.as_ref().map_or(Value::Absent, |version| Value::Name {
                text: version.text(),
                span: version.span,
            }),
        )
}

fn import_node(import: &ImportDecl) -> Node {
    Node::new("ImportDecl", import.span)
        .field("path", text(&import.path.literal.value))
        .field("alias", opt_name(import.alias.as_ref()))
        .field("take", names(&import.take))
}

fn item_node(item: &Item) -> Node {
    match item {
        Item::Object(decl) => object_node(decl),
        Item::Role(decl) => role_node(decl),
        Item::Enum(decl) => Node::new("EnumDecl", decl.span)
            .field("name", name(&decl.name))
            .field("cases", names(&decl.cases)),
        Item::Test(decl) => test_node(decl),
        Item::NativeAction(decl) => Node::new("NativeActionDecl", decl.span)
            .field("async", Value::Bool(decl.is_async))
            .field("name", name(&decl.name))
            .field("params", list(&decl.params, param_node))
            .field("result", Value::node(result_node(&decl.result))),
        Item::NativeObject(decl) => {
            Node::new("NativeObjectDecl", decl.span).field("name", name(&decl.name))
        }
        Item::Cap(decl) => cap_node(decl),
        Item::Action(decl) => action_node(decl),
    }
}

fn generic_node(param: &GenericParam) -> Node {
    Node::new("GenericParam", param.span)
        .field("name", name(&param.name))
        .field("bound", opt_node(param.bound.as_ref(), type_node))
}

fn object_node(decl: &ObjectDecl) -> Node {
    Node::new("ObjectDecl", decl.span)
        .field("name", name(&decl.name))
        .field("generics", list(&decl.generics, generic_node))
        .field("roles", list(&decl.roles, type_node))
        .field("members", list(&decl.members, member_node))
}

fn member_node(member: &ObjectMember) -> Node {
    match member {
        ObjectMember::Has(decl) => Node::new("HasDecl", decl.span)
            .field("names", names(&decl.names))
            .field("type", opt_node(decl.ty.as_ref(), type_node))
            .field("default", opt_node(decl.default.as_ref(), expr_node)),
        ObjectMember::Cap(decl) => cap_node(decl),
        ObjectMember::Constructor(decl) => constructor_node(decl),
        ObjectMember::Destroy(decl) => {
            Node::new("DestroyDecl", decl.span).field("body", Value::node(block_node(&decl.body)))
        }
        ObjectMember::Test(decl) => test_node(decl),
        ObjectMember::Action(decl) => action_node(decl),
        ObjectMember::Private(section) => Node::new("PrivateSection", section.span)
            .field("actions", list(&section.actions, action_node)),
    }
}

fn cap_node(decl: &CapDecl) -> Node {
    Node::new("CapDecl", decl.span)
        .field("owner", opt_name(decl.owner.as_ref()))
        .field("name", name(&decl.name))
        .field(
            "fields",
            list(&decl.fields, |field| {
                Node::new("ComputedField", field.span)
                    .field("name", name(&field.name))
                    .field("type", Value::node(type_node(&field.ty)))
                    .field("body", Value::node(block_node(&field.body)))
            }),
        )
}

fn constructor_node(decl: &ConstructorDecl) -> Node {
    match &decl.form {
        ConstructorForm::Full { params, body } => Node::new("FullConstructor", decl.span)
            .field("name", name(&decl.name))
            .field("params", list(params, param_node))
            .field("body", Value::node(block_node(body))),
        ConstructorForm::Shorthand { fields, body } => Node::new("ShorthandConstructor", decl.span)
            .field("name", name(&decl.name))
            .field("fields", names(fields))
            .field("body", opt_node(body.as_ref(), block_node)),
    }
}

fn skipped_node(span: Span) -> Node {
    Node::new("SkippedBody", span)
}

fn test_node(decl: &TestDecl) -> Node {
    let body = match &decl.body {
        TestBody::Parsed(block) => block_node(block),
        TestBody::Skipped(span) => skipped_node(*span),
    };
    Node::new("TestDecl", decl.span)
        .field("async", Value::Bool(decl.is_async))
        .field("name", text(&decl.name.literal.value))
        .field("body", Value::node(body))
}

fn action_node(decl: &ActionDecl) -> Node {
    Node::new("ActionDecl", decl.span)
        .field("async", Value::Bool(decl.is_async))
        .field("visibility", Value::word(decl.visibility.as_str()))
        .field(
            "projection",
            opt_node(decl.projection.as_ref(), |directive| {
                Node::new("ProjectionDirective", directive.span)
                    .field("alias", opt_name(directive.alias.as_ref()))
            }),
        )
        .field("name", name(&decl.name))
        .field("params", list(&decl.params, param_node))
        .field("result", Value::node(result_node(&decl.result)))
        .field("body", Value::node(block_node(&decl.body)))
}

fn param_node(param: &Param) -> Node {
    Node::new("Param", param.span)
        .field("name", name(&param.name))
        .field("variadic", Value::Bool(param.variadic))
        .field("type", Value::node(type_node(&param.ty)))
}

fn role_node(decl: &RoleDecl) -> Node {
    Node::new("RoleDecl", decl.span)
        .field("name", name(&decl.name))
        .field("generics", list(&decl.generics, generic_node))
        .field(
            "members",
            list(&decl.members, |member| match member {
                RoleMember::Signature(signature) => Node::new("ActionSignature", signature.span)
                    .field("async", Value::Bool(signature.is_async))
                    .field("name", name(&signature.name))
                    .field("params", list(&signature.params, param_node))
                    .field("result", Value::node(result_node(&signature.result))),
                RoleMember::Fake(fake) => {
                    let body = match &fake.body {
                        FakeBody::Parsed(members) => Node::new("FakeBody", fake.span)
                            .field("members", list(members, member_node)),
                        FakeBody::Skipped(span) => skipped_node(*span),
                    };
                    Node::new("FakeDecl", fake.span).field("body", Value::node(body))
                }
                RoleMember::Test(test) => test_node(test),
            }),
        )
}

// ----- types ---------------------------------------------------------------

fn result_node(result: &ResultSpec) -> Node {
    match result {
        ResultSpec::Void(span) => Node::new("VoidResult", *span),
        ResultSpec::Successor { ty, span } => {
            Node::new("SuccessorResult", *span).field("type", Value::node(type_node(ty)))
        }
        ResultSpec::Type(ty) => type_node(ty),
    }
}

fn action_result_node(result: &ActionResult) -> Node {
    match result {
        ActionResult::Void(span) => Node::new("VoidResult", *span),
        ActionResult::Type(ty) => type_node(ty),
    }
}

fn type_node(ty: &TypeExpr) -> Node {
    match &ty.kind {
        TypeKind::Named { path, args } => Node::new("NamedType", ty.span)
            .field("path", names(path))
            .field("args", list(args, type_node)),
        TypeKind::Nested { owner, name: cap } => Node::new("NestedType", ty.span)
            .field("owner", Value::node(type_node(owner)))
            .field("name", name(cap)),
        TypeKind::Array(element) => {
            Node::new("ArrayType", ty.span).field("element", Value::node(type_node(element)))
        }
        TypeKind::Map { key, value } => Node::new("MapType", ty.span)
            .field("key", Value::node(type_node(key)))
            .field("value", Value::node(type_node(value))),
        TypeKind::Action {
            is_async,
            params,
            result,
        } => Node::new("ActionType", ty.span)
            .field("async", Value::Bool(*is_async))
            .field("params", list(params, type_node))
            .field("result", Value::node(action_result_node(result))),
    }
}

// ----- statements ----------------------------------------------------------

fn block_node(block: &Block) -> Node {
    Node::new("Block", block.span).field("statements", list(&block.stmts, stmt_node))
}

fn try_fields(node: Node, body: &TryBody) -> Node {
    node.field("body", Value::node(block_node(&body.body)))
        .field(
            "catches",
            list(&body.catches, |clause| {
                Node::new("CatchClause", clause.span)
                    .field("name", name(&clause.name))
                    .field("type", Value::node(type_node(&clause.ty)))
                    .field("body", Value::node(block_node(&clause.body)))
            }),
        )
}

fn match_fields(node: Node, body: &MatchBody) -> Node {
    node.field("subject", Value::node(expr_node(&body.subject)))
        .field(
            "cases",
            list(&body.cases, |case| {
                Node::new("MatchCase", case.span)
                    .field("patterns", list(&case.patterns, expr_node))
                    .field("body", Value::node(block_node(&case.body)))
            }),
        )
        .field("else", opt_node(body.else_block.as_ref(), block_node))
}

fn if_stmt_node(stmt: &IfStmt) -> Node {
    let else_branch = match &stmt.else_branch {
        None => Value::Absent,
        Some(ElseBranch::Block(block)) => Value::node(block_node(block)),
        Some(ElseBranch::If(nested)) => Value::node(if_stmt_node(nested)),
    };
    Node::new("IfStatement", stmt.span)
        .field("condition", Value::node(expr_node(&stmt.condition)))
        .field("then", Value::node(block_node(&stmt.then_block)))
        .field("else", else_branch)
}

fn stmt_node(stmt: &Stmt) -> Node {
    let span = stmt.span;
    match &stmt.kind {
        StmtKind::Let {
            name: binding,
            ty,
            value,
        } => Node::new("LetStatement", span)
            .field("name", name(binding))
            .field("type", opt_node(ty.as_ref(), type_node))
            .field("value", Value::node(expr_node(value))),
        StmtKind::Defer { value } => {
            Node::new("DeferStatement", span).field("value", Value::node(expr_node(value)))
        }
        StmtKind::Try(body) => try_fields(Node::new("TryStatement", span), body),
        StmtKind::Throw { value } => {
            Node::new("ThrowStatement", span).field("value", opt_node(value.as_ref(), expr_node))
        }
        StmtKind::Return { value } => {
            Node::new("ReturnStatement", span).field("value", opt_node(value.as_ref(), expr_node))
        }
        StmtKind::Confirm {
            condition,
            fallback,
        } => {
            let fallback = match fallback {
                None => Value::Absent,
                Some(ConfirmFallback::Panic { keyword, value }) => Value::node(
                    Node::new("ConfirmPanic", keyword.to(value.span))
                        .field("value", Value::node(expr_node(value))),
                ),
                Some(ConfirmFallback::Throw { keyword, value }) => Value::node(
                    Node::new("ConfirmThrow", keyword.to(value.span))
                        .field("value", Value::node(expr_node(value))),
                ),
            };
            Node::new("ConfirmStatement", span)
                .field("condition", Value::node(expr_node(condition)))
                .field("fallback", fallback)
        }
        StmtKind::SuccessorAssign { name: field, value } => {
            Node::new("SuccessorAssignStatement", span)
                .field("name", name(field))
                .field("value", Value::node(expr_node(value)))
        }
        StmtKind::Block(block) => {
            Node::new("BlockStatement", span).field("statements", list(&block.stmts, stmt_node))
        }
        StmtKind::If(if_stmt) => if_stmt_node(if_stmt),
        StmtKind::Match(body) => match_fields(Node::new("MatchStatement", span), body),
        StmtKind::For {
            source,
            bindings,
            body,
        } => Node::new("ForStatement", span)
            .field("source", Value::node(expr_node(source)))
            .field(
                "bindings",
                list(bindings, |binding| {
                    Node::new("ForBinding", binding.span)
                        .field("name", name(&binding.name))
                        .field(
                            "batch",
                            opt_node(binding.batch.as_ref(), |batch| {
                                int_literal_node(&batch.literal, batch.span)
                            }),
                        )
                }),
            )
            .field("body", Value::node(block_node(body))),
        StmtKind::Next { condition } => Node::new("NextStatement", span)
            .field("condition", opt_node(condition.as_ref(), expr_node)),
        StmtKind::Last { value, condition } => Node::new("LastStatement", span)
            .field("value", opt_node(value.as_ref(), expr_node))
            .field("condition", opt_node(condition.as_ref(), expr_node)),
        StmtKind::FieldBind { name: field, value } => Node::new("FieldBindStatement", span)
            .field("name", name(field))
            .field("value", Value::node(expr_node(value))),
        StmtKind::SelfFieldBind {
            receiver: _,
            name: field,
            value,
        } => Node::new("SelfFieldBindStatement", span)
            .field("name", name(field))
            .field("value", Value::node(expr_node(value))),
        StmtKind::Expr { expr, condition } => Node::new("ExpressionStatement", span)
            .field("expression", Value::node(expr_node(expr)))
            .field("condition", opt_node(condition.as_ref(), expr_node)),
    }
}

// ----- expressions -----------------------------------------------------------

fn argument_node(argument: &Argument) -> Node {
    Node::new("Argument", argument.span)
        .field("name", opt_name(argument.name.as_ref()))
        .field("value", Value::node(expr_node(&argument.value)))
}

fn string_literal_node(literal: &StrLiteral, span: Span) -> Node {
    let form = match literal.form {
        StringForm::Quoted => "quoted",
        StringForm::Fenced => "fenced",
        StringForm::Endl => "endl",
    };
    Node::new("StringLiteral", span)
        .field("form", Value::word(form))
        .field("value", text(&literal.value))
}

fn int_literal_node(literal: &IntLiteral, span: Span) -> Node {
    Node::new("IntLiteral", span)
        .field("radix", Value::word(literal.radix.as_str()))
        .field("digits", Value::Word(literal.digits.clone()))
}

fn expr_node(expr: &Expr) -> Node {
    let span = expr.span;
    match &expr.kind {
        ExprKind::Literal(literal) => match literal {
            Literal::Int(value) => int_literal_node(value, span),
            Literal::Float(value) => {
                Node::new("FloatLiteral", span).field("value", Value::Word(value.text()))
            }
            Literal::String(value) => string_literal_node(value, span),
            Literal::Char(value) => {
                Node::new("CharLiteral", span).field("value", Value::Text(value.to_string()))
            }
            Literal::Bool(value) => {
                Node::new("BoolLiteral", span).field("value", Value::Bool(*value))
            }
        },
        ExprKind::Name(variable) => Node::new("NameExpression", span).field("name", name(variable)),
        ExprKind::SelfValue => Node::new("SelfExpression", span),
        ExprKind::Grouping(inner) => {
            Node::new("Grouping", span).field("expression", Value::node(expr_node(inner)))
        }
        ExprKind::Unary {
            op,
            op_span: _,
            operand,
        } => Node::new("Unary", span)
            .field("operator", Value::word(op.symbol()))
            .field("operand", Value::node(expr_node(operand))),
        ExprKind::Binary {
            op,
            op_span: _,
            left,
            right,
        } => Node::new("Binary", span)
            .field("operator", Value::word(op.symbol()))
            .field("left", Value::node(expr_node(left)))
            .field("right", Value::node(expr_node(right))),
        ExprKind::StreamWrite {
            op_span: _,
            sink,
            source,
        } => Node::new("StreamWrite", span)
            .field("sink", Value::node(expr_node(sink)))
            .field("source", Value::node(expr_node(source))),
        ExprKind::Range {
            inclusive,
            op_span: _,
            start,
            end,
        } => Node::new("Range", span)
            .field(
                "operator",
                Value::word(if *inclusive { "..=" } else { ".." }),
            )
            .field("start", Value::node(expr_node(start)))
            .field("end", Value::node(expr_node(end))),
        ExprKind::Call { callee, args } => Node::new("Call", span)
            .field("callee", Value::node(expr_node(callee)))
            .field("arguments", list(args, argument_node)),
        ExprKind::ArrayCast {
            marker: _,
            target,
            args,
        } => Node::new("ArrayCast", span)
            .field("target", name(target))
            .field("arguments", list(args, argument_node)),
        ExprKind::Get {
            object,
            name: member,
        } => Node::new("Get", span)
            .field("object", Value::node(expr_node(object)))
            .field("name", name(member)),
        ExprKind::Select { object, name: cap } => Node::new("Select", span)
            .field("object", Value::node(expr_node(object)))
            .field("name", name(cap)),
        ExprKind::OwnerField { owner, name: field } => Node::new("OwnerField", span)
            .field("owner", name(owner))
            .field("name", name(field)),
        ExprKind::Index {
            object,
            index,
            fallback,
        } => Node::new("Index", span)
            .field("object", Value::node(expr_node(object)))
            .field("index", Value::node(expr_node(index)))
            .field("fallback", opt_node(fallback.as_deref(), expr_node)),
        ExprKind::Unpack {
            operand,
            ellipsis: _,
        } => Node::new("Unpack", span).field("operand", Value::node(expr_node(operand))),
        ExprKind::Projection {
            prefix,
            construction,
        } => Node::new("Projection", span)
            .field("prefix", name(prefix))
            .field("construction", Value::node(expr_node(construction))),
        ExprKind::Functional {
            operation: _,
            operation_name,
            receiver,
            args,
            bindings,
            body,
        } => Node::new("Functional", span)
            .field("operation", name(operation_name))
            .field("receiver", Value::node(expr_node(receiver)))
            .field("arguments", list(args, argument_node))
            .field("bindings", names(bindings))
            .field("body", Value::node(block_node(body))),
        ExprKind::AnonymousAction {
            params,
            result,
            body,
        } => Node::new("AnonymousAction", span)
            .field("params", list(params, param_node))
            .field("result", Value::node(action_result_node(result)))
            .field("body", Value::node(block_node(body))),
        ExprKind::Array { elements } => {
            Node::new("ArrayLiteral", span).field("elements", list(elements, expr_node))
        }
        ExprKind::Map { entries } => Node::new("MapLiteral", span).field(
            "entries",
            list(entries, |entry| {
                Node::new("MapEntry", entry.span)
                    .field("key", Value::node(expr_node(&entry.key)))
                    .field("value", Value::node(expr_node(&entry.value)))
            }),
        ),
        ExprKind::If {
            condition,
            then_block,
            else_block,
        } => Node::new("IfExpression", span)
            .field("condition", Value::node(expr_node(condition)))
            .field("then", Value::node(block_node(then_block)))
            .field("else", Value::node(block_node(else_block))),
        ExprKind::Match(body) => match_fields(Node::new("MatchExpression", span), body),
        ExprKind::Loop { body } => {
            Node::new("LoopExpression", span).field("body", Value::node(block_node(body)))
        }
        ExprKind::Try(body) => try_fields(Node::new("TryExpression", span), body),
        ExprKind::Panic { value } => {
            Node::new("Panic", span).field("value", Value::node(expr_node(value)))
        }
        ExprKind::Await { value } => {
            Node::new("Await", span).field("value", Value::node(expr_node(value)))
        }
        ExprKind::Spawn { call } => {
            Node::new("Spawn", span).field("call", Value::node(expr_node(call)))
        }
    }
}
