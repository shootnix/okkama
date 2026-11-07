//! Nominal variant construction, pattern bindings, and optional fallback.
use super::generics::Inference;
use super::stmt::strip_grouping;
use super::{Block, Checker, Expr, ExprKind, Stmt, StmtKind};
use crate::analysis::{
    thir::{Constant, Match},
    ty::{EnumId, Ty},
};
use crate::{source::Span, syntax::ast};

impl Checker<'_, '_> {
    pub(super) fn variant_receiver(
        &mut self,
        receiver: &ast::Expr,
    ) -> Option<(EnumId, Option<Vec<Ty>>)> {
        if let ast::ExprKind::TypeValue(ty) = &strip_grouping(receiver).kind {
            let generics = self.generics.clone();
            return match self.reg.resolve(ty, &generics) {
                Ty::Variant(id, args) => Some((id, Some(args))),
                _ => None,
            };
        }
        let id = self.enum_receiver(receiver)?;
        self.reg.enums[id.0].variant.then_some((id, None))
    }

    pub(super) fn variant_construct(
        &mut self,
        id: EnumId,
        explicit: Option<Vec<Ty>>,
        name: &ast::Name,
        args: &[ast::Argument],
        expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        let decl = self.reg.enums[id.0].clone();
        let Some(case) = decl.cases.iter().position(|c| c == &name.text) else {
            self.error(
                name.span,
                format!("Enum {} has no case {}", decl.name, name.text),
            );
            return Self::poisoned(span);
        };
        let fields = &decl.payloads[case];
        if args.len() != fields.len() || args.iter().any(|a| a.name.is_some()) {
            self.error(
                span,
                format!(
                    "Variant {}.{} expects {} positional payload field(s)",
                    decl.name,
                    name.text,
                    fields.len()
                ),
            );
            return Self::poisoned(span);
        }
        let context = explicit.or_else(|| match expected {
            Some(Ty::Variant(other, args)) if *other == id => Some(args.clone()),
            _ => None,
        });
        let mut inference = Inference {
            object: decl.name.clone(),
            params: decl.params.clone(),
            bindings: vec![None; decl.params.len()],
            conflict: None,
        };
        if let Some(context) = context {
            for (slot, ty) in inference.bindings.iter_mut().zip(context) {
                *slot = Some(ty);
            }
        }
        let mut values = Vec::new();
        for (arg, (_, pattern)) in args.iter().zip(fields) {
            let env: Vec<_> = inference
                .params
                .iter()
                .zip(&inference.bindings)
                .filter_map(|(p, t)| t.clone().map(|t| (*p, t)))
                .collect();
            let target = pattern.subst(&env);
            let value = self.value(&arg.value, Some(&target));
            self.unify(&mut inference, pattern, &Self::ty_of(&value).taken());
            values.push(value);
        }
        if let Some(message) = inference.conflict {
            self.error(span, message);
            return Self::poisoned(span);
        }
        let Some(types): Option<Vec<Ty>> = inference.bindings.into_iter().collect() else {
            self.error(span, format!("Cannot infer type arguments for {}.{}; provide an expected type or explicit type arguments", decl.name, name.text));
            return Self::poisoned(span);
        };
        let env: Vec<_> = decl.params.iter().copied().zip(types.clone()).collect();
        for (value, (_, pattern)) in values.iter_mut().zip(fields) {
            let target = pattern.subst(&env);
            *value = self.coerce(value.clone(), &target, value.span, "Variant payload");
        }
        Self::typed(
            ExprKind::VariantNew { case, values },
            Ty::Variant(id, types),
            span,
        )
    }

    fn variant_temp(&mut self, value: Expr) -> (Stmt, Expr) {
        let span = value.span;
        let ty = Self::ty_of(&value);
        let name = ast::Name {
            text: format!("$variant{}", self.locals.len()),
            span,
        };
        let local = self.declare(&name, ty.clone(), false);
        (
            Stmt {
                span,
                kind: StmtKind::Let { local, value },
            },
            Self::typed(ExprKind::Local(local), ty, span),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn variant_match(
        &mut self,
        body: &ast::MatchBody,
        subject: Expr,
        id: EnumId,
        types: Vec<Ty>,
        want: bool,
        expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        let decl = self.reg.enums[id.0].clone();
        let env: Vec<_> = decl.params.iter().copied().zip(types).collect();
        self.push_scope();
        let (binding, subject) = self.variant_temp(subject);
        let mut cases = Vec::new();
        let mut arms = Vec::new();
        let mut seen = vec![false; decl.cases.len()];
        for arm in &body.cases {
            self.push_scope();
            let mut patterns = Vec::new();
            let mut bindings = Vec::new();
            for pattern in &arm.patterns {
                let (callee, args) = match &strip_grouping(pattern).kind {
                    ast::ExprKind::Call { callee, args } => (&**callee, &args[..]),
                    _ => (pattern, &[][..]),
                };
                let ast::ExprKind::Get { object, name } = &strip_grouping(callee).kind else {
                    self.error(pattern.span, "Expected an enum variant pattern");
                    continue;
                };
                let receiver = if matches!(&object.kind, ast::ExprKind::Name(n) if n.text == "$choice")
                {
                    Some((id, None))
                } else {
                    self.variant_receiver(object)
                };
                match receiver {
                    Some((other, explicit)) if other == id => {
                        if let Some(explicit) = explicit {
                            let actual: Vec<_> = env.iter().map(|(_, t)| t.clone()).collect();
                            if explicit != actual {
                                self.error(
                                    pattern.span,
                                    "Pattern has different enum type arguments",
                                );
                            }
                        }
                    }
                    _ => {
                        self.error(pattern.span, "Pattern belongs to a different enum");
                        continue;
                    }
                }
                let Some(case) = decl.cases.iter().position(|c| c == &name.text) else {
                    self.error(name.span, "Unknown enum variant");
                    continue;
                };
                if seen[case] {
                    self.error(pattern.span, "Duplicate variant pattern");
                }
                seen[case] = true;
                let fields = &decl.payloads[case];
                if args.len() != fields.len() {
                    self.error(pattern.span, "Wrong number of variant pattern bindings");
                    continue;
                }
                if arm.patterns.len() > 1 && !args.is_empty() {
                    self.error(pattern.span, "Payload patterns require a separate case arm");
                    continue;
                }
                let offset = 1 + decl.payloads[..case].iter().map(Vec::len).sum::<usize>();
                for (index, (arg, (_, ty))) in args.iter().zip(fields).enumerate() {
                    let ast::ExprKind::Name(name) = &arg.value.kind else {
                        self.error(arg.span, "Expected a binding or _ in variant pattern");
                        continue;
                    };
                    if arg.name.is_some() {
                        self.error(arg.span, "Pattern bindings are positional");
                    }
                    if name.text == "_" {
                        continue;
                    }
                    let ty = ty.subst(&env);
                    let local = self.declare(name, ty.clone(), false);
                    let value = Self::typed(
                        ExprKind::VariantGet {
                            value: Box::new(subject.clone()),
                            field: offset + index,
                        },
                        ty,
                        arg.span,
                    );
                    bindings.push(Stmt {
                        kind: StmtKind::Let { local, value },
                        span: arg.span,
                    });
                }
                patterns.push(Self::typed(
                    ExprKind::Constant(Constant::Int(case.to_string())),
                    Ty::Int,
                    pattern.span,
                ));
            }
            let out = self.block(&arm.body, want, expected);
            arms.push(out.clone());
            let mut block = self.block_of(out);
            bindings.append(&mut block.stmts);
            block.stmts = bindings;
            cases.push((patterns, block));
            self.pop_scope();
        }
        let exhaustive = seen.iter().all(|b| *b);
        if !exhaustive && body.else_block.is_none() {
            let missing: Vec<_> = decl
                .cases
                .iter()
                .zip(&seen)
                .filter(|(_, seen)| !**seen)
                .map(|(name, _)| name.clone())
                .collect();
            self.error(
                span,
                format!("Non-exhaustive match: missing {}", missing.join(", ")),
            );
        }
        let other = body
            .else_block
            .as_ref()
            .map(|b| self.block(b, want, expected));
        if let Some(other) = &other {
            arms.push(other.clone());
        }
        let (ty, diverges) = self.join(&arms.iter().collect::<Vec<_>>(), want, span);
        let result = Expr {
            kind: ExprKind::Match(Box::new(Match {
                subject: Self::typed(ExprKind::VariantTag(Box::new(subject)), Ty::Int, span),
                cases,
                else_block: other.map(|e| self.block_of(e)),
                exhaustive: exhaustive && body.else_block.is_none(),
            })),
            ty: ty.clone(),
            diverges,
            span,
        };
        self.pop_scope();
        Expr {
            kind: ExprKind::Block(Block {
                stmts: vec![
                    binding,
                    Stmt {
                        kind: StmtKind::Expr(result),
                        span,
                    },
                ],
                span,
            }),
            ty,
            diverges,
            span,
        }
    }

    pub(super) fn variant_compare(
        &mut self,
        lhs: Expr,
        rhs: Expr,
        id: EnumId,
        types: Vec<Ty>,
        negate: bool,
        span: Span,
    ) -> Expr {
        let decl = self.reg.enums[id.0].clone();
        let env: Vec<_> = decl.params.iter().copied().zip(types).collect();
        self.push_scope();
        let (left_binding, left) = self.variant_temp(lhs);
        let (right_binding, right) = self.variant_temp(rhs);
        let tag = |value: Expr| Self::typed(ExprKind::VariantTag(Box::new(value)), Ty::Int, span);
        let same_tag = self.binary_values(
            ast::BinaryOp::Equal,
            tag(left.clone()),
            tag(right.clone()),
            span,
        );
        let mut cases = Vec::new();
        let mut offset = 1;
        for (case, fields) in decl.payloads.iter().enumerate() {
            let mut equal = Self::typed(ExprKind::Constant(Constant::Bool(true)), Ty::Bool, span);
            for (_, ty) in fields {
                let ty = ty.subst(&env);
                let a = Self::typed(
                    ExprKind::VariantGet {
                        value: Box::new(left.clone()),
                        field: offset,
                    },
                    ty.clone(),
                    span,
                );
                let b = Self::typed(
                    ExprKind::VariantGet {
                        value: Box::new(right.clone()),
                        field: offset,
                    },
                    ty,
                    span,
                );
                let field_equal = self.binary_values(ast::BinaryOp::Equal, a, b, span);
                equal = Self::typed(
                    ExprKind::Logical {
                        and: true,
                        left: Box::new(equal),
                        right: Box::new(field_equal),
                    },
                    Ty::Bool,
                    span,
                );
                offset += 1;
            }
            cases.push((
                vec![Self::typed(
                    ExprKind::Constant(Constant::Int(case.to_string())),
                    Ty::Int,
                    span,
                )],
                self.block_of(equal),
            ));
        }
        let fields_equal = Self::typed(
            ExprKind::Match(Box::new(Match {
                subject: tag(left),
                cases,
                else_block: None,
                exhaustive: true,
            })),
            Ty::Bool,
            span,
        );
        let mut result = Self::typed(
            ExprKind::Logical {
                and: true,
                left: Box::new(same_tag),
                right: Box::new(fields_equal),
            },
            Ty::Bool,
            span,
        );
        if negate {
            result = Self::typed(
                ExprKind::Unary {
                    op: crate::analysis::thir::UnaryOp::Not,
                    operand: Box::new(result),
                },
                Ty::Bool,
                span,
            );
        }
        self.pop_scope();
        Self::typed(
            ExprKind::Block(Block {
                stmts: vec![
                    left_binding,
                    right_binding,
                    Stmt {
                        kind: StmtKind::Expr(result),
                        span,
                    },
                ],
                span,
            }),
            Ty::Bool,
            span,
        )
    }

    pub(super) fn variant_method(
        &mut self,
        target: Expr,
        id: EnumId,
        types: Vec<Ty>,
        name: &ast::Name,
        args: &[ast::Argument],
        span: Span,
    ) -> Expr {
        // `text()` error enum — слот роли Error (порождённый текст варианта).
        if self.reg.enums[id.0].error && name.text == "text" {
            let error = Ty::role(self.reg.system.error);
            let target = self.coerce(target, &error, span, "Error value");
            let role = self.reg.system.error;
            return self.role_method(target, role, Vec::new(), name, args, span);
        }
        let output = match (id.0, name.text.as_str(), args.len()) {
            (1, "fold", 0) => EnumId(0),
            (0, "as_presence", 0) => EnumId(1),
            _ => {
                self.error(
                    span,
                    "Unknown variant action; use match to inspect its payload",
                );
                return Self::poisoned(span);
            }
        };
        let ty = Ty::Variant(output, types.clone());
        self.push_scope();
        let (binding, local) = self.variant_temp(target);
        let payload = Self::typed(
            ExprKind::VariantGet {
                value: Box::new(local.clone()),
                field: 1,
            },
            types[0].clone(),
            span,
        );
        let some = Self::typed(
            ExprKind::VariantNew {
                case: 0,
                values: vec![payload],
            },
            ty.clone(),
            span,
        );
        let none = Self::typed(
            ExprKind::VariantNew {
                case: 1,
                values: Vec::new(),
            },
            ty.clone(),
            span,
        );
        let result = Self::typed(
            ExprKind::Match(Box::new(Match {
                subject: Self::typed(ExprKind::VariantTag(Box::new(local)), Ty::Int, span),
                cases: vec![(
                    vec![Self::typed(
                        ExprKind::Constant(Constant::Int("0".into())),
                        Ty::Int,
                        span,
                    )],
                    self.block_of(some),
                )],
                else_block: Some(self.block_of(none)),
                exhaustive: false,
            })),
            ty.clone(),
            span,
        );
        self.pop_scope();
        Self::typed(
            ExprKind::Block(Block {
                stmts: vec![
                    binding,
                    Stmt {
                        kind: StmtKind::Expr(result),
                        span,
                    },
                ],
                span,
            }),
            ty,
            span,
        )
    }

    pub(super) fn optional_fallback(
        &mut self,
        source: &ast::Expr,
        fallback: &ast::Expr,
        expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        let optional_expected = expected.map(|t| Ty::Variant(EnumId(0), vec![t.clone()]));
        let value = self.value(source, optional_expected.as_ref());
        let Ty::Variant(id, args) = Self::ty_of(&value) else {
            self.error(span, "Optional fallback requires Maybe<T>");
            return Self::poisoned(span);
        };
        if id != EnumId(0) || args.len() != 1 {
            self.error(
                span,
                "Optional fallback requires Maybe<T>; fold Presence explicitly",
            );
            return Self::poisoned(span);
        }
        self.push_scope();
        let (binding, local) = self.variant_temp(value);
        let payload = Self::typed(
            ExprKind::VariantGet {
                value: Box::new(local.clone()),
                field: 1,
            },
            args[0].clone(),
            span,
        );
        let other = self.value(fallback, Some(&args[0]));
        let other = self.coerce(other, &args[0], fallback.span, "Fallback");
        let result = Self::typed(
            ExprKind::Match(Box::new(Match {
                subject: Self::typed(ExprKind::VariantTag(Box::new(local)), Ty::Int, span),
                cases: vec![(
                    vec![Self::typed(
                        ExprKind::Constant(Constant::Int("0".into())),
                        Ty::Int,
                        span,
                    )],
                    self.block_of(payload),
                )],
                else_block: Some(self.block_of(other)),
                exhaustive: false,
            })),
            args[0].clone(),
            span,
        );
        self.pop_scope();
        Self::typed(
            ExprKind::Block(Block {
                stmts: vec![
                    binding,
                    Stmt {
                        kind: StmtKind::Expr(result),
                        span,
                    },
                ],
                span,
            }),
            args[0].clone(),
            span,
        )
    }
}
