//! Структура файла по синтаксическому дереву (без анализа).

use super::{LineIndex, OutlineItem, OutlineKind};
use crate::source::Span;
use crate::syntax::ast;

pub fn outline(program: &ast::Program, lines: &LineIndex<'_>) -> Vec<OutlineItem> {
    program
        .items
        .iter()
        .filter_map(|item| item_outline(item, lines))
        .collect()
}

fn item(
    lines: &LineIndex<'_>,
    name: &ast::Name,
    detail: String,
    kind: OutlineKind,
    span: Span,
    children: Vec<OutlineItem>,
) -> OutlineItem {
    OutlineItem {
        name: name.text.clone(),
        detail,
        kind,
        range: lines.range(span.to(name.span)),
        selection: lines.range(name.span),
        children,
    }
}

fn item_outline(entry: &ast::Item, lines: &LineIndex<'_>) -> Option<OutlineItem> {
    Some(match entry {
        ast::Item::Object(decl) => {
            let roles: Vec<String> = decl.roles.iter().map(type_text).collect();
            let detail = if roles.is_empty() {
                String::new()
            } else {
                format!("does {}", roles.join(", "))
            };
            let children = members(&decl.members, lines);
            item(
                lines,
                &decl.name,
                detail,
                OutlineKind::Object,
                decl.span,
                children,
            )
        }
        ast::Item::Role(decl) => {
            let children = decl
                .members
                .iter()
                .filter_map(|member| match member {
                    ast::RoleMember::Signature(signature) => Some(item(
                        lines,
                        &signature.name,
                        signature_text(&signature.params, &signature.result),
                        OutlineKind::Action,
                        signature.span,
                        Vec::new(),
                    )),
                    ast::RoleMember::Test(test) => Some(test_outline(test, lines)),
                    ast::RoleMember::Fake(_) => None,
                })
                .collect();
            item(
                lines,
                &decl.name,
                String::new(),
                OutlineKind::Role,
                decl.span,
                children,
            )
        }
        ast::Item::Enum(decl) => {
            let children = decl
                .cases
                .iter()
                .map(|case| {
                    item(
                        lines,
                        case,
                        String::new(),
                        OutlineKind::EnumCase,
                        case.span,
                        Vec::new(),
                    )
                })
                .collect();
            item(
                lines,
                &decl.name,
                String::new(),
                OutlineKind::Enum,
                decl.span,
                children,
            )
        }
        ast::Item::Test(test) => test_outline(test, lines),
        ast::Item::NativeAction(decl) => item(
            lines,
            &decl.name,
            signature_text(&decl.params, &decl.result),
            OutlineKind::Native,
            decl.span,
            Vec::new(),
        ),
        ast::Item::NativeObject(decl) => item(
            lines,
            &decl.name,
            "native object".to_string(),
            OutlineKind::Native,
            decl.span,
            Vec::new(),
        ),
        ast::Item::Cap(decl) => cap_outline(decl, lines),
        ast::Item::Action(decl) => item(
            lines,
            &decl.name,
            signature_text(&decl.params, &decl.result),
            OutlineKind::Main,
            decl.span,
            Vec::new(),
        ),
    })
}

fn members(members: &[ast::ObjectMember], lines: &LineIndex<'_>) -> Vec<OutlineItem> {
    let mut out = Vec::new();
    for member in members {
        match member {
            ast::ObjectMember::Has(has) => {
                let detail = has.ty.as_ref().map(type_text).unwrap_or_default();
                for name in &has.names {
                    out.push(item(
                        lines,
                        name,
                        detail.clone(),
                        OutlineKind::Field,
                        name.span,
                        Vec::new(),
                    ));
                }
            }
            ast::ObjectMember::Cap(cap) => out.push(cap_outline(cap, lines)),
            ast::ObjectMember::Constructor(ctor) => {
                let detail = match &ctor.form {
                    ast::ConstructorForm::Full { params, .. } => params_text(params),
                    ast::ConstructorForm::Shorthand { fields, .. } => {
                        let names: Vec<String> = fields
                            .iter()
                            .map(|field| format!(":{}", field.text))
                            .collect();
                        format!("({})", names.join(", "))
                    }
                };
                out.push(item(
                    lines,
                    &ctor.name,
                    detail,
                    OutlineKind::Constructor,
                    ctor.span,
                    Vec::new(),
                ));
            }
            ast::ObjectMember::Destroy(_) => {}
            ast::ObjectMember::Test(test) => out.push(test_outline(test, lines)),
            ast::ObjectMember::Action(action) => out.push(action_outline(action, lines)),
            ast::ObjectMember::Private(section) => {
                for action in &section.actions {
                    out.push(action_outline(action, lines));
                }
            }
        }
    }
    out
}

fn action_outline(action: &ast::ActionDecl, lines: &LineIndex<'_>) -> OutlineItem {
    let mut detail = signature_text(&action.params, &action.result);
    if action.is_async {
        detail = format!("async {detail}");
    }
    if action.visibility == ast::Visibility::Private {
        detail = format!("private {detail}");
    }
    item(
        lines,
        &action.name,
        detail,
        OutlineKind::Action,
        action.span,
        Vec::new(),
    )
}

fn cap_outline(cap: &ast::CapDecl, lines: &LineIndex<'_>) -> OutlineItem {
    let children = cap
        .fields
        .iter()
        .map(|field| {
            item(
                lines,
                &field.name,
                type_text(&field.ty),
                OutlineKind::Field,
                field.span,
                Vec::new(),
            )
        })
        .collect();
    item(
        lines,
        &cap.name,
        "cap".to_string(),
        OutlineKind::Cap,
        cap.span,
        children,
    )
}

fn test_outline(test: &ast::TestDecl, lines: &LineIndex<'_>) -> OutlineItem {
    let name = ast::Name {
        text: format!("test {}", test.name.literal.value),
        span: test.name.span,
    };
    item(
        lines,
        &name,
        String::new(),
        OutlineKind::Test,
        test.span,
        Vec::new(),
    )
}

/// `(a is int, b is @string) is Result`.
pub(crate) fn signature_text(params: &[ast::Param], result: &ast::ResultSpec) -> String {
    format!("{} is {}", params_text(params), result_text(result))
}

pub(crate) fn params_text(params: &[ast::Param]) -> String {
    let params: Vec<String> = params
        .iter()
        .map(|param| {
            format!(
                "{} is {}{}",
                param.name.text,
                if param.variadic { "..." } else { "" },
                type_text(&param.ty)
            )
        })
        .collect();
    format!("({})", params.join(", "))
}

pub(crate) fn result_text(result: &ast::ResultSpec) -> String {
    match result {
        ast::ResultSpec::Void(_) => "void".to_string(),
        ast::ResultSpec::Successor { ty, .. } => format!("^{}", type_text(ty)),
        ast::ResultSpec::Type(ty) => type_text(ty),
    }
}

/// Тип так, как он записан.
pub(crate) fn type_text(ty: &ast::TypeExpr) -> String {
    match &ty.kind {
        ast::TypeKind::Named { path, args } => {
            let name: Vec<&str> = path.iter().map(|part| part.text.as_str()).collect();
            if args.is_empty() {
                name.join(".")
            } else {
                let args: Vec<String> = args.iter().map(type_text).collect();
                format!("{}<{}>", name.join("."), args.join(", "))
            }
        }
        ast::TypeKind::Nested { owner, name } => format!("{}::{}", type_text(owner), name.text),
        ast::TypeKind::Array(element) => format!("@{}", type_text(element)),
        ast::TypeKind::Map { key, value } => format!("%{}:{}", type_text(key), type_text(value)),
        ast::TypeKind::Action {
            is_async,
            params,
            result,
        } => {
            let params: Vec<String> = params.iter().map(type_text).collect();
            let result = match result {
                ast::ActionResult::Void(_) => "void".to_string(),
                ast::ActionResult::Type(ty) => type_text(ty),
            };
            format!(
                "{}({}) is {result}",
                if *is_async { "async " } else { "" },
                params.join(", ")
            )
        }
    }
}
