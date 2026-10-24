//! Production pruning: removing test-only declarations from a parsed tree.
//!
//! The parser never drops nodes on its own. A production build that must
//! not see test code (for example to compare with the reference
//! interpreter's production AST, or before semantic analysis of a program
//! that is about to run) calls [`strip_test_items`] explicitly.

use crate::syntax::ast::{
    ConstructorDecl, ConstructorForm, FakeBody, Item, Name, ObjectDecl, ObjectMember, Program,
    ResultSpec, RoleMember, TestDecl, TypeExpr, TypeKind,
};

/// Имя объекта fake роли `Role`: `$` не встречается в идентификаторах
/// Okkama, поэтому исходник его назвать не может.
pub fn fake_object_name(role: &str) -> String {
    format!("{role}$fake")
}

/// Тестовый режим (§14): fake роли становится объектом модуля
/// `Role$fake<T…> does Role<T…>` с конструктором `Role$fake();` (поля — из
/// defaults), тесты объектов и ролей — тестами модуля с именем
/// `Owner :: name` (без `self` и доступа к private).
pub fn desugar_test_items(mut program: Program) -> Program {
    let mut fakes = Vec::new();
    let mut items = Vec::new();
    // Тесты владельца — сразу за ним: исполнитель идёт в порядке исходника.
    for mut item in std::mem::take(&mut program.items) {
        let mut lifted = Vec::new();
        match &mut item {
            Item::Object(object) => {
                let owner = object.name.text.clone();
                object.members.retain(|member| match member {
                    ObjectMember::Test(test) => {
                        lifted.push(qualified(&owner, test));
                        false
                    }
                    _ => true,
                });
            }
            Item::Role(role) => {
                let owner = role.name.text.clone();
                let mut fake = None;
                role.members.retain(|member| match member {
                    RoleMember::Test(test) => {
                        lifted.push(qualified(&owner, test));
                        false
                    }
                    RoleMember::Fake(decl) => {
                        if let FakeBody::Parsed(members) = &decl.body {
                            fake = Some((members.clone(), decl.span));
                        }
                        false
                    }
                    RoleMember::Signature(_) => true,
                });
                if let Some((mut members, span)) = fake {
                    let name = Name {
                        text: fake_object_name(&owner),
                        span: role.name.span,
                    };
                    // `is ^Role` в fake — successor самого fake-объекта.
                    for member in &mut members {
                        if let ObjectMember::Action(action) = member {
                            if let ResultSpec::Successor { ty, .. } = &mut action.result {
                                if let TypeKind::Named { path, .. } = &mut ty.kind {
                                    if path.len() == 1 && path[0].text == owner {
                                        path[0].text = name.text.clone();
                                    }
                                }
                            }
                        }
                    }
                    members.push(ObjectMember::Constructor(ConstructorDecl {
                        name: name.clone(),
                        form: ConstructorForm::Shorthand {
                            fields: Vec::new(),
                            body: None,
                        },
                        span,
                    }));
                    let args = role
                        .generics
                        .iter()
                        .map(|param| TypeExpr {
                            kind: TypeKind::Named {
                                path: vec![param.name.clone()],
                                args: Vec::new(),
                            },
                            span: param.span,
                        })
                        .collect();
                    fakes.push(Item::Object(ObjectDecl {
                        name,
                        generics: role.generics.clone(),
                        roles: vec![TypeExpr {
                            kind: TypeKind::Named {
                                path: vec![role.name.clone()],
                                args,
                            },
                            span: role.name.span,
                        }],
                        members,
                        span,
                    }));
                }
            }
            _ => {}
        }
        items.push(item);
        items.extend(lifted.into_iter().map(Item::Test));
    }
    items.extend(fakes);
    program.items = items;
    program
}

fn qualified(owner: &str, test: &TestDecl) -> TestDecl {
    let mut test = test.clone();
    test.name.literal.value = format!("{owner} :: {}", test.name.literal.value);
    test
}

/// Removes every `test` block (module, object and role scope, sync and
/// async) and every role `fake` block. All other nodes are kept unchanged.
pub fn strip_test_items(mut program: Program) -> Program {
    program.items.retain(|item| !matches!(item, Item::Test(_)));
    for item in &mut program.items {
        match item {
            Item::Object(object) => {
                object
                    .members
                    .retain(|member| !matches!(member, ObjectMember::Test(_)));
            }
            Item::Role(role) => {
                role.members
                    .retain(|member| matches!(member, RoleMember::Signature(_)));
            }
            Item::Enum(_)
            | Item::Test(_)
            | Item::NativeAction(_)
            | Item::NativeObject(_)
            | Item::Cap(_)
            | Item::Action(_) => {}
        }
    }
    program
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceFile;
    use crate::syntax::{ParseMode, ParseOptions, parse};

    const SOURCE: &str = r#"module app v1.0.0;
test "module test" { 1; }
async test "async module test" { 2; }
object Box {
    Box();
    test "object test" { 2; }
    value() is int { 3; }
}
role Readable {
    read() is int;
    fake { read() is int { 42; } }
    test "role test" { 3; }
}
main() is int { 0; }
"#;

    fn parse_in(mode: ParseMode) -> Program {
        let source = SourceFile::new("t.ok", SOURCE).unwrap();
        parse(
            &source,
            &ParseOptions {
                mode,
                ..ParseOptions::default()
            },
        )
        .unwrap()
    }

    fn shape(program: &Program) -> Vec<String> {
        let mut out = Vec::new();
        for item in &program.items {
            match item {
                Item::Object(object) => {
                    out.push(format!("object {}", object.name.text));
                    for member in &object.members {
                        out.push(format!(
                            "  {}",
                            match member {
                                ObjectMember::Test(_) => "test",
                                ObjectMember::Constructor(_) => "constructor",
                                ObjectMember::Action(_) => "action",
                                _ => "other",
                            }
                        ));
                    }
                }
                Item::Role(role) => {
                    out.push(format!("role {}", role.name.text));
                    for member in &role.members {
                        out.push(format!(
                            "  {}",
                            match member {
                                RoleMember::Signature(_) => "signature",
                                RoleMember::Fake(_) => "fake",
                                RoleMember::Test(_) => "test",
                            }
                        ));
                    }
                }
                Item::Test(_) => out.push("test".into()),
                Item::Action(action) => out.push(format!("action {}", action.name.text)),
                _ => out.push("other".into()),
            }
        }
        out
    }

    #[test]
    fn full_parse_keeps_every_test_and_fake() {
        let program = parse_in(ParseMode::Full);
        assert_eq!(
            shape(&program),
            vec![
                "test",
                "test",
                "object Box",
                "  constructor",
                "  test",
                "  action",
                "role Readable",
                "  signature",
                "  fake",
                "  test",
                "action main"
            ]
        );
    }

    #[test]
    fn production_parse_keeps_nodes_with_skipped_bodies() {
        let full = shape(&parse_in(ParseMode::Full));
        let production = shape(&parse_in(ParseMode::Production));
        assert_eq!(full, production, "production parsing alone drops nothing");
    }

    #[test]
    fn pruning_removes_exactly_tests_and_fakes() {
        for mode in [ParseMode::Full, ParseMode::Production] {
            let pruned = strip_test_items(parse_in(mode));
            assert_eq!(
                shape(&pruned),
                vec![
                    "object Box",
                    "  constructor",
                    "  action",
                    "role Readable",
                    "  signature",
                    "action main"
                ]
            );
        }
    }
}
