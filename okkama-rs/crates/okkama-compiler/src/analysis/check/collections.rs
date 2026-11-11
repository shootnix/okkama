//! Обходы: `for` и функциональные операции `map`/`filter`/`reduce` (§6.8, §9.9).

use super::stmt::{Flow, strip_grouping};
use super::{Checker, LoopCtx};
use crate::analysis::thir::{Constant, Expr, ExprKind, Functional, FunctionalOp, LocalId, Source, StmtKind};
use crate::analysis::ty::Ty;
use crate::source::Span;
use crate::syntax::ast;

/// Источник с типами значений, которые обход даёт телу.
struct Checked {
    source: Source,
    items: Vec<Ty>,
    /// Имя категории для сообщений: `Array`, `Range`, `Map`, `Iterable`.
    name: &'static str,
    /// Пустой нетегированный источник.
    untagged: bool,
    diverges: bool,
}

impl Checker<'_, '_> {
    /// Элемент обхода `collections/iteration.Iterating<T>`: сама роль, её
    /// сужение из другой роли с `cursor() is Cursor<T>` (`List<T>`,
    /// `Sequencing<T>`; проверка во время исполнения) или тип, который её делает.
    pub(super) fn iterating_item(&self, ty: &Ty) -> Option<Ty> {
        let iterating = self.reg.standard.iterating?;
        if let Ty::Role(role, args) = ty {
            if *role == iterating {
                return args.first().cloned();
            }
            let cursor = self.reg.standard.cursor?;
            let item = self.reg.role_actions(ty).into_iter().find_map(
                |(name, params, result, _, _)| match (name.as_str(), params.is_empty(), result) {
                    ("cursor", true, Some(Ty::Role(id, args))) if id == cursor => {
                        args.into_iter().next()
                    }
                    _ => None,
                },
            )?;
            let target = Ty::Role(iterating, vec![item.clone()]);
            return self.reg.narrows(ty, &target).then_some(item);
        }
        self.reg
            .roles_of(ty)
            .into_iter()
            .find_map(|role| match role {
                Ty::Role(id, args) if id == iterating => args.into_iter().next(),
                _ => None,
            })
    }

    /// Элемент пачечного обхода `collections/iteration.Batching<T>` (#465): сама
    /// роль или тип, который её делает. Пачки быстрее курсора, поэтому тип,
    /// который делает обе роли, обходится пачками.
    pub(super) fn batching_item(&self, ty: &Ty) -> Option<Ty> {
        let batching = self.reg.standard.batching?;
        if let Ty::Role(role, args) = ty
            && *role == batching
        {
            return args.first().cloned();
        }
        self.reg
            .roles_of(ty)
            .into_iter()
            .find_map(|role| match role {
                Ty::Role(id, args) if id == batching => args.into_iter().next(),
                _ => None,
            })
    }

    fn source(&mut self, expr: &ast::Expr, allow_map: bool) -> Option<Checked> {
        let inner = strip_grouping(expr);
        if let ast::ExprKind::Range {
            inclusive,
            start,
            end,
            ..
        } = &inner.kind
        {
            let (start, end) = self.range_bounds(start, end);
            let diverges = start.diverges || end.diverges;
            return Some(Checked {
                source: Source::Range {
                    start: Box::new(start),
                    end: Box::new(end),
                    inclusive: *inclusive,
                },
                items: vec![Ty::Int],
                name: "Range",
                untagged: false,
                diverges,
            });
        }
        if let ast::ExprKind::Unpack { operand, .. } = &inner.kind {
            if let ast::ExprKind::Name(name) = &strip_grouping(operand).kind
                && let Some(local) = self.local(&name.text)
                    && self.locals[local.0].variadic {
                        let item = match &self.locals[local.0].ty {
                            Ty::Varargs(item) => (**item).clone(),
                            _ => Ty::Error,
                        };
                        return Some(Checked {
                            source: Source::Varargs(local),
                            items: vec![item],
                            name: "Array",
                            untagged: false,
                            diverges: false,
                        });
                    }
            let array = self.unpacked(operand, expr.span);
            let item = match Self::ty_of(&array) {
                Ty::Array(item) => *item,
                _ => Ty::Error,
            };
            let diverges = array.diverges;
            return Some(Checked {
                untagged: item == Ty::Never,
                source: Source::Array(Box::new(array)),
                items: vec![item],
                name: "Array",
                diverges,
            });
        }
        let value = self.value(expr, None);
        let ty = Self::ty_of(&value).taken();
        let diverges = value.diverges;
        let value = if Self::ty_of(&value) != ty {
            self.coerce(value, &ty, expr.span, "Source")
        } else {
            value
        };
        match &ty {
            Ty::Array(item) => Some(Checked {
                untagged: **item == Ty::Never,
                items: vec![(**item).clone()],
                source: Source::Array(Box::new(value)),
                name: "Array",
                diverges,
            }),
            Ty::Range => Some(Checked {
                items: vec![Ty::Int],
                source: Source::Array(Box::new(value)),
                name: "Range",
                untagged: false,
                diverges,
            }),
            Ty::Map(key, item) if allow_map => Some(Checked {
                untagged: **key == Ty::Never,
                items: vec![(**key).clone(), (**item).clone()],
                source: Source::Map(Box::new(value)),
                name: "Map",
                diverges,
            }),
            Ty::Map(..) => {
                self.error(expr.span, "For source must be an array or range");
                None
            }
            Ty::Error => None,
            other if self.batching_item(other).is_some() => {
                let item = self.batching_item(other).expect("checked above");
                let batching = self.reg.standard.batching.expect("found an item");
                let target = Ty::Role(batching, vec![item.clone()]);
                let value = self.coerce(value, &target, expr.span, "Iteration source");
                Some(Checked {
                    items: vec![item],
                    source: Source::Batching(Box::new(value)),
                    name: "Batching",
                    untagged: false,
                    diverges,
                })
            }
            other => match self.iterating_item(other) {
                Some(item) => {
                    let iterating = self.reg.standard.iterating.expect("found an item");
                    let target = Ty::Role(iterating, vec![item.clone()]);
                    let value = self.coerce(value, &target, expr.span, "Iteration source");
                    Some(Checked {
                        items: vec![item],
                        source: Source::Iterating(Box::new(value)),
                        name: "Iterating",
                        untagged: false,
                        diverges,
                    })
                }
                None => {
                    let message = if allow_map {
                        format!(
                            "Functional operations require an array, range, typed map, Iterating or Batching receiver, got {}",
                            self.type_name(other)
                        )
                    } else {
                        format!(
                            "For source must be an array, range, Iterating or Batching, got {}",
                            self.type_name(other)
                        )
                    };
                    self.error(expr.span, message);
                    None
                }
            },
        }
    }

    /// Async iteration is checked as a hygienic block/loop/match expansion.
    /// Reusing these nodes preserves await, exception, defer and loop-control
    /// semantics on every backend without a second execution protocol.
    pub(super) fn for_await_stmt(
        &mut self,
        source: &ast::Expr,
        bindings: &[ast::ForBinding],
        body: &ast::Block,
        span: Span,
    ) -> (StmtKind, Flow) {
        let [binding] = bindings else {
            self.error(span, "for await requires exactly one binding");
            let value = Self::poisoned(span);
            return (StmtKind::Expr(value.clone()), Flow { ty: value.ty, diverges: false });
        };
        if binding.batch.is_some() {
            self.error(binding.span, "for await does not support batched bindings");
        }
        let expr = |kind| ast::Expr { kind, span };
        let name = |text: &str| ast::Name { text: text.into(), span };
        // NUL cannot occur in a source identifier; nested scopes isolate it.
        let temporary = name("\0async_iteration_source");
        let member = |object, text| expr(ast::ExprKind::Get {
            object: Box::new(object), name: name(text),
        });
        let variant = |text| member(expr(ast::ExprKind::Name(name("Maybe"))), text);
        let call = expr(ast::ExprKind::Call {
            callee: Box::new(member(expr(ast::ExprKind::Name(temporary.clone())), "next")),
            args: vec![],
        });
        let subject = expr(ast::ExprKind::Await { value: Box::new(call) });
        let some = expr(ast::ExprKind::Call {
            callee: Box::new(variant("some")),
            args: vec![ast::Argument {
                name: None,
                value: expr(ast::ExprKind::Name(binding.name.clone())),
                span: binding.span,
            }],
        });
        let matched = ast::Stmt {
            kind: ast::StmtKind::Match(ast::MatchBody {
                subject,
                cases: vec![
                    ast::MatchCase {
                        patterns: vec![variant("none")],
                        body: ast::Block { stmts: vec![ast::Stmt {
                            kind: ast::StmtKind::Last { value: None, condition: None }, span,
                        }], span },
                        span,
                    },
                    ast::MatchCase { patterns: vec![some], body: body.clone(), span: body.span },
                ],
                else_block: None,
                span,
            }),
            span,
        };
        let block = ast::Block {
            stmts: vec![
                ast::Stmt { kind: ast::StmtKind::Let {
                    name: temporary, ty: None, value: source.clone(),
                }, span: source.span },
                ast::Stmt { kind: ast::StmtKind::Expr {
                    expr: expr(ast::ExprKind::Loop { body: ast::Block { stmts: vec![matched], span } }),
                    condition: None,
                }, span },
            ],
            span,
        };
        let value = self.block(&block, false, None);
        let flow = Flow { ty: value.ty.clone(), diverges: value.diverges };
        (StmtKind::Expr(value), flow)
    }

    /// `for source -> binding { body }`.
    pub(super) fn for_stmt(
        &mut self,
        source: &ast::Expr,
        bindings: &[ast::ForBinding],
        body: &ast::Block,
        want: bool,
        expected: Option<&Ty>,
        span: Span,
    ) -> (StmtKind, Flow) {
        let checked = self.source(source, false);
        let [binding] = bindings else {
            self.error(source.span, "This `for` source binds exactly one value");
            return (
                StmtKind::Expr(Self::poisoned(span)),
                Flow {
                    ty: None,
                    diverges: false,
                },
            );
        };
        if binding.batch.is_some() {
            self.unsupported(
                binding.span,
                "Analysis of batched `for` bindings is not implemented yet.",
            );
        }
        let (item, source_thir, diverges) = match checked {
            // Пустой нетегированный источник: тело не исполняется ни разу.
            Some(checked) if checked.untagged => (Ty::Error, checked.source, checked.diverges),
            Some(checked) => (checked.items[0].clone(), checked.source, checked.diverges),
            None => (
                Ty::Error,
                Source::Array(Box::new(Self::poisoned(source.span))),
                false,
            ),
        };
        self.push_scope();
        let local = self.declare(&binding.name, item, false);
        // `for` в позиции значения: хвост последней итерации или `last value`
        // (§9.10); если тело не исполнилось, значения нет — ошибка исполнения.
        self.loops.push(LoopCtx {
            functional: false,
            want,
            exits: Vec::new(),
            plain_exit: true,
            bare_last_ok: true,
            expected: expected.cloned(),
        });
        let out = self.block(body, want, expected);
        let frame = self.loops.pop().expect("loop frame pushed above");
        self.pop_scope();
        let ty = if diverges {
            Some(Ty::Never)
        } else if want {
            let mut arms: Vec<Expr> = frame
                .exits
                .iter()
                .map(|ty| Self::typed(ExprKind::Unreachable, ty.clone(), span))
                .collect();
            if !out.diverges {
                arms.push(out.clone());
            }
            let refs: Vec<&Expr> = arms.iter().collect();
            if refs.is_empty() {
                None
            } else {
                self.join(&refs, true, span).0
            }
        } else {
            None
        };
        // Значение `for` есть, только если тело исполнилось хоть раз (§9.10).
        if want && ty.is_some() && !diverges {
            match iterations(&source_thir) {
                Some(false) => self.error(
                    span,
                    "For loop has no value: its source is empty, so the body never runs",
                ),
                Some(true) => {}
                None => self.warn(
                    span,
                    "For loop value is undefined when its source is empty: the body may not run",
                ),
            }
        }
        let body = self.block_of(out);
        let expr = Expr {
            kind: ExprKind::For {
                source: source_thir,
                binding: local,
                body,
            },
            ty: ty.clone(),
            diverges,
            span,
        };
        (StmtKind::Expr(expr), Flow { ty, diverges })
    }

    /// Тело функционального блока с привязками.
    fn functional_body(
        &mut self,
        bindings: &[ast::Name],
        types: &[Ty],
        body: &ast::Block,
        expected: Option<&Ty>,
    ) -> (Vec<LocalId>, Expr) {
        self.push_scope();
        let locals = bindings
            .iter()
            .zip(types)
            .map(|(name, ty)| self.declare(name, ty.clone(), false))
            .collect();
        self.loops.push(LoopCtx {
            functional: true,
            want: false,
            exits: Vec::new(),
            plain_exit: true,
            bare_last_ok: false,
            expected: None,
        });
        let out = self.block(body, true, expected);
        self.loops.pop();
        self.pop_scope();
        if out.ty.is_none() && !out.diverges {
            self.error(body.span, "A functional block must produce a value");
        }
        (locals, out)
    }

    /// `receiver.map() -> v { }`, `.filter() -> v { }`, `.reduce(init) -> acc, v { }`;
    /// у словаря блок получает ключ и значение.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn functional(
        &mut self,
        operation: ast::FunctionalOp,
        receiver: &ast::Expr,
        args: &[ast::Argument],
        bindings: &[ast::Name],
        body: &ast::Block,
        expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        let expected_args = usize::from(operation == ast::FunctionalOp::Reduce);
        if args.len() != expected_args || args.iter().any(|arg| arg.name.is_some()) {
            let message = format!(
                "`{}` takes {expected_args} positional argument(s)",
                operation.as_str()
            );
            self.error(span, message);
            return Self::poisoned(span);
        }
        let Some(checked) = self.source(receiver, true) else {
            for arg in args {
                let _ = self.expr(&arg.value, true, None);
            }
            return Self::poisoned(span);
        };
        let count = checked.items.len() + expected_args;
        if bindings.len() != count {
            let message = format!(
                "{} {} requires exactly {count} binding{}, got {}",
                checked.name,
                operation.as_str(),
                if count == 1 { "" } else { "s" },
                bindings.len()
            );
            self.error(span, message);
            return Self::poisoned(span);
        }
        let is_map = checked.name == "Map";
        if is_map && checked.untagged {
            self.error(
                span,
                "Functional map operations require a typed %K:V receiver",
            );
            return Self::poisoned(span);
        }
        let item_types: Vec<Ty> = if checked.untagged {
            vec![Ty::Error; checked.items.len()]
        } else {
            checked.items.clone()
        };
        let diverges = checked.diverges;
        let mut found = None;
        let (op, initial, locals, block, ty) = match operation {
            ast::FunctionalOp::Map => {
                let target = match expected {
                    Some(Ty::Array(element)) if **element != Ty::Never => Some((**element).clone()),
                    _ => None,
                };
                if checked.untagged && target.is_none() {
                    self.error(span, "Cannot infer map result type from empty source");
                    return Self::poisoned(span);
                }
                let (locals, out) =
                    self.functional_body(bindings, &item_types, body, target.as_ref());
                let output = match (&target, &out.ty) {
                    (Some(target), _) => target.clone(),
                    (None, Some(ty)) if !out.diverges => ty.taken(),
                    _ => Ty::Never,
                };
                let out = if out.ty.is_some() && !out.diverges {
                    self.coerce(out, &output, body.span, "Map result")
                } else {
                    out
                };
                (
                    FunctionalOp::Map,
                    None,
                    locals,
                    self.block_of(out),
                    Ty::array(output),
                )
            }
            ast::FunctionalOp::Filter => {
                let (locals, out) =
                    self.functional_body(bindings, &item_types, body, Some(&Ty::Bool));
                self.expect(&out, &Ty::Bool, body.span, "Filter predicate");
                let ty = match &checked.source {
                    Source::Map(map) => Self::ty_of(map).taken(),
                    _ if checked.untagged => Ty::array(Ty::Never),
                    _ => Ty::array(checked.items[0].clone()),
                };
                (FunctionalOp::Filter, None, locals, self.block_of(out), ty)
            }
            ast::FunctionalOp::Any | ast::FunctionalOp::All | ast::FunctionalOp::Find => {
                // Early exit (#150): the block decides per item; `found` is
                // the value at the first decisive item, `initial` the value
                // when none decides.
                let (locals, out) =
                    self.functional_body(bindings, &item_types, body, Some(&Ty::Bool));
                let label = match operation {
                    ast::FunctionalOp::Any => "Any predicate",
                    ast::FunctionalOp::All => "All predicate",
                    _ => "Find predicate",
                };
                self.expect(&out, &Ty::Bool, body.span, label);
                let boolean = |value: bool| Self::typed(ExprKind::Constant(Constant::Bool(value)), Ty::Bool, span);
                match operation {
                    ast::FunctionalOp::Any => {
                        found = Some(Box::new(boolean(true)));
                        (FunctionalOp::Any, Some(boolean(false)), locals, self.block_of(out), Ty::Bool)
                    }
                    ast::FunctionalOp::All => {
                        found = Some(Box::new(boolean(false)));
                        (FunctionalOp::All, Some(boolean(true)), locals, self.block_of(out), Ty::Bool)
                    }
                    _ => {
                        if is_map {
                            self.error(span, "`find` works on arrays, ranges, Iterating and Batching values; on a map use filter");
                            return Self::poisoned(span);
                        }
                        let maybe = crate::analysis::ty::EnumId(0);
                        let element = match (expected, checked.untagged) {
                            (Some(Ty::Variant(id, args)), _) if *id == maybe && args.len() == 1 => args[0].clone(),
                            (_, false) => item_types[0].clone(),
                            (_, true) => {
                                self.error(span, "Cannot infer find result type from empty source");
                                return Self::poisoned(span);
                            }
                        };
                        let ty = Ty::Variant(maybe, vec![element.clone()]);
                        let item = Self::typed(ExprKind::Local(locals[0]), element, span);
                        // Maybe's cases: some(value), none.
                        found = Some(Box::new(Self::typed(
                            ExprKind::VariantNew { case: 0, values: vec![item] },
                            ty.clone(),
                            span,
                        )));
                        let none = Self::typed(ExprKind::VariantNew { case: 1, values: Vec::new() }, ty.clone(), span);
                        (FunctionalOp::Find, Some(none), locals, self.block_of(out), ty)
                    }
                }
            }
            ast::FunctionalOp::Reduce => {
                let initial = self.value(&args[0].value, expected);
                let accumulator = match expected {
                    Some(ty) if !ty.is_untagged() => ty.clone(),
                    _ => Self::ty_of(&initial).taken(),
                };
                let initial = self.coerce(
                    initial,
                    &accumulator,
                    args[0].value.span,
                    "Reduce initial value",
                );
                let mut types = vec![accumulator.clone()];
                types.extend(item_types.iter().cloned());
                let (locals, out) =
                    self.functional_body(bindings, &types, body, Some(&accumulator));
                // An empty `[]` or `[:]` names no element type, and a block
                // that gives a typed collection cannot supply one: say what to
                // write instead of "must be [], got @int" (#295).
                let typed_block = out
                    .ty
                    .clone()
                    .filter(|ty| accumulator.is_untagged() && !out.diverges && !ty.is_untagged() && !ty.is_error());
                let out = if let Some(block) = typed_block {
                    let message = self.untyped_accumulator(&accumulator, &block);
                    self.error(args[0].value.span, message);
                    out
                } else if out.ty.is_some() && !out.diverges {
                    self.coerce(out, &accumulator, body.span, "Reduce accumulator")
                } else {
                    out
                };
                (
                    FunctionalOp::Reduce,
                    Some(initial),
                    locals,
                    self.block_of(out),
                    accumulator,
                )
            }
        };
        let mut out = Self::typed(
            ExprKind::Functional(Box::new(Functional {
                op,
                source: checked.source,
                initial,
                bindings: locals,
                body: block,
                found,
            })),
            ty,
            span,
        );
        out.diverges = diverges;
        out
    }
}

/// Известно ли статически, что источник непуст (`Some(true)`) или пуст
/// (`Some(false)`): константный диапазон или литеральная коллекция.
fn iterations(source: &Source) -> Option<bool> {
    match source {
        Source::Range {
            start,
            end,
            inclusive,
        } => {
            let (start, end) = (constant_int(start)?, constant_int(end)?);
            Some(if *inclusive {
                start <= end
            } else {
                start < end
            })
        }
        Source::Array(value) | Source::Map(value) => literal_size(value),
        Source::Varargs(_) | Source::Iterating(_) | Source::Batching(_) => None,
    }
}

fn strip_coercions(expr: &Expr) -> &Expr {
    match &expr.kind {
        ExprKind::Coerce { value, .. } => strip_coercions(value),
        _ => expr,
    }
}

fn constant_int(expr: &Expr) -> Option<i128> {
    match &strip_coercions(expr).kind {
        ExprKind::Constant(crate::analysis::thir::Constant::Int(text)) => text.parse().ok(),
        _ => None,
    }
}

/// Непуст ли литерал коллекции (`[]`, `[:]`, `[1, 2]`, `[0..n]`).
fn literal_size(expr: &Expr) -> Option<bool> {
    match &strip_coercions(expr).kind {
        ExprKind::Array(parts) => {
            let mut unknown = false;
            for part in parts {
                match part {
                    crate::analysis::thir::ArrayPart::Value(_) => return Some(true),
                    crate::analysis::thir::ArrayPart::Range {
                        start,
                        end,
                        inclusive,
                    } => match (constant_int(start), constant_int(end)) {
                        (Some(start), Some(end))
                            if if *inclusive {
                                start <= end
                            } else {
                                start < end
                            } =>
                        {
                            return Some(true);
                        }
                        (Some(_), Some(_)) => {}
                        _ => unknown = true,
                    },
                }
            }
            if unknown { None } else { Some(false) }
        }
        ExprKind::Map(entries) => Some(!entries.is_empty()),
        ExprKind::ArrayCast { parts, .. } => {
            if parts
                .iter()
                .any(|part| matches!(part, crate::analysis::thir::CastPart::One(_)))
            {
                Some(true)
            } else if parts.is_empty() {
                Some(false)
            } else {
                None
            }
        }
        _ => None,
    }
}

impl Checker<'_, '_> {
    /// `reduce([])` or `reduce([:])` with no declared target: the initial
    /// value has no element type, and the block's result says which one to
    /// write (#295).
    fn untyped_accumulator(&self, accumulator: &Ty, block: &Ty) -> String {
        let name = self.type_name(block);
        match (accumulator, block) {
            (Ty::Array(_), Ty::Array(_)) => format!(
                "Reduce starts from [], which has no element type, and its block gives {name}: start from {name}() or declare the result type (let name is {name} = …)"
            ),
            (Ty::Map(..), _) => format!(
                "Reduce starts from [:], which has no key or value type, and its block gives {name}: declare the result type (let name is {name} = …)"
            ),
            _ => format!(
                "Reduce starts from [], which has no element type, and its block gives {name}: declare the result type (let name is {name} = …)"
            ),
        }
    }
}
