//! Массивы, диапазоны, словари и функциональные операции (§12, §19).
//!
//! Все обходы — один цикл по целому счётчику [`BodyLowerer::counted_loop`]:
//! `for` по диапазону и по массиву, `map`, `filter`, `reduce`. Состояние цикла
//! (сборщик результата, аккумулятор) — параметры заголовка рядом с
//! перепривязываемыми переменными. Тело функционального блока встраивается в
//! функцию, а не становится замыканием: `return` из него выходит из действия.

use super::{Arm, BodyLowerer, LoopFrame, Out, Typed, unsupported};
use crate::analysis::thir::{
    self, ArrayPart, CastPart, CastTarget, ExprKind, FunctionalOp, LocalId,
};
use crate::analysis::ty::Ty;
use crate::ir::{
    CompareOp, Constant, InstKind, IntBinaryOp, IntValue, Target, TermKind, TrapCode, Type, ValueId,
};
use crate::lowering::Result;
use crate::source::Span;

/// Литерал длиннее этого собирается потоково (`streamed_array`).
const LARGE_LITERAL: usize = 64;

/// Источник обхода: диапазон, массив или словарь (пары в порядке ключей).
enum Source {
    Range {
        start: ValueId,
        end: ValueId,
        inclusive: bool,
    },
    Array {
        array: ValueId,
        element: Type,
    },
    Map {
        keys: ValueId,
        values: ValueId,
        key: Type,
        value: Type,
    },
    /// Курсор `collections/iteration.Cursor<T>`: `available`, `item`, `successor`.
    Cursor {
        cursor: Typed,
        element: Type,
        available: usize,
        item: usize,
        successor: usize,
    },
}

impl Source {
    fn items(&self) -> Vec<Type> {
        match self {
            Source::Range { .. } => vec![Type::Int],
            Source::Array { element, .. } => vec![element.clone()],
            Source::Map { key, value, .. } => vec![key.clone(), value.clone()],
            Source::Cursor { element, .. } => vec![element.clone()],
        }
    }

    /// Пустой нетегированный источник: тело не выполняется ни разу.
    fn untagged(&self) -> bool {
        self.items().contains(&Type::Never)
    }
}

impl<'r> BodyLowerer<'_, 'r, '_> {
    /// Цикл по `start..bound` (`..=` при `inclusive`). `state` — значения,
    /// которые тело получает и возвращает на каждой итерации; тело
    /// возвращает `None`, если управление ушло. Результат — итог слияния
    /// выходов и значения состояния после исчерпания счётчика.
    #[allow(clippy::too_many_arguments)]
    fn counted_loop(
        &mut self,
        start: ValueId,
        bound: ValueId,
        inclusive: bool,
        state: Vec<Typed>,
        scanned: &thir::Block,
        span: Span,
        value: bool,
        mut body: impl FnMut(&mut Self, ValueId, Vec<Typed>) -> Result<Option<Vec<ValueId>>>,
    ) -> Result<(Out, Vec<Typed>)> {
        let base = self.vars.len();
        let carried = self.carried_vars(scanned, span)?;
        let state_types: Vec<Type> = state.iter().map(|value| value.ty.clone()).collect();
        let mut extra = vec![Type::Int];
        extra.extend(state_types.iter().cloned());
        let (head, head_params) = self.block_for_vars(&extra, &carried);
        let index = head_params[0];
        let state_params: Vec<Typed> = head_params[1..=state.len()]
            .iter()
            .zip(&state_types)
            .map(|(value, ty)| Typed {
                value: *value,
                ty: ty.clone(),
            })
            .collect();
        let mut args = vec![start];
        args.extend(state.iter().map(|value| value.value));
        args.extend(self.carried_args(&carried, span));
        self.b
            .terminate(TermKind::Br(Target { block: head, args }), span);
        self.b.switch_to(head);
        self.bind_carried(&carried, &head_params[1 + state.len()..]);
        let more = self.emit(
            InstKind::IntCompare {
                op: if inclusive {
                    CompareOp::Le
                } else {
                    CompareOp::Lt
                },
                left: index,
                right: bound,
            },
            Type::Bool,
            span,
        );
        let iteration = self.b.new_block();
        let finished = self.b.new_block();
        self.b.terminate(
            TermKind::Cond {
                condition: more.value,
                then_target: Target::new(iteration),
                else_target: Target::new(finished),
            },
            span,
        );
        self.b.switch_to(finished);
        let exhausted = if value {
            let last = self.last_iteration(&state_params, span);
            self.arm(Out::Value(last))
        } else {
            self.arm(Out::Unit)
        }
        .expect("the exhausted edge is open");
        let (step, step_params) = self.block_for_vars(&state_types, &carried);

        self.b.switch_to(iteration);
        self.loops.push(LoopFrame {
            next: step,
            state: state_params.iter().map(|value| value.value).collect(),
            carried: carried.clone(),
            exits: vec![exhausted],
            base,
            depth: self.scopes.len(),
            want: value,
        });
        let body_out = body(self, index, state_params.clone());
        if let Ok(Some(next_state)) = &body_out {
            if self.b.is_open() {
                let mut args = next_state.clone();
                args.extend(self.carried_args(&carried, span));
                self.b
                    .terminate(TermKind::Br(Target { block: step, args }), span);
            }
        }
        let frame = self.loops.pop().expect("loop frame pushed above");
        body_out?;

        self.b.switch_to(step);
        self.bind_carried(&carried, &step_params[state.len()..]);
        // Следующее значение счётчика: `int` точный (R01), переполнения нет.
        let one = self.constant(Constant::Int(IntValue::from_i64(1)), span);
        let advanced = self.emit(
            InstKind::IntBinary {
                op: IntBinaryOp::Add,
                left: index,
                right: one.value,
            },
            Type::Int,
            span,
        );
        let mut args = vec![advanced.value];
        args.extend(step_params[..state.len()].iter().copied());
        args.extend(self.carried_args(&carried, span));
        self.b
            .terminate(TermKind::Br(Target { block: head, args }), span);
        let out = self.join(frame.exits, frame.base, value, span)?;
        Ok((out, state_params))
    }

    /// `start..end`: проверка порядка границ (§12.4) и значения границ.
    pub(super) fn range_bounds(
        &mut self,
        start: &'r thir::Expr,
        end: &'r thir::Expr,
        span: Span,
    ) -> Result<Option<(ValueId, ValueId)>> {
        let Some(first) = self.value(start)? else {
            return Ok(None);
        };
        let Some(bound) = self.value(end)? else {
            return Ok(None);
        };
        let ordered = self.emit(
            InstKind::IntCompare {
                op: CompareOp::Le,
                left: first.value,
                right: bound.value,
            },
            Type::Bool,
            span,
        );
        let valid = self.b.new_block();
        let invalid = self.b.new_block();
        self.b.terminate(
            TermKind::Cond {
                condition: ordered.value,
                then_target: Target::new(valid),
                else_target: Target::new(invalid),
            },
            span,
        );
        self.b.switch_to(invalid);
        self.b.terminate(
            TermKind::Trap {
                code: TrapCode::Range,
                message: "Range start must be less than or equal to range end".to_string(),
            },
            span,
        );
        self.b.switch_to(valid);
        Ok(Some((first.value, bound.value)))
    }

    /// Источник обхода; `None` — управление ушло.
    fn source(&mut self, source: &'r thir::Source, span: Span) -> Result<Option<Source>> {
        match source {
            thir::Source::Range {
                start,
                end,
                inclusive,
            } => Ok(self
                .range_bounds(start, end, span)?
                .map(|(start, end)| Source::Range {
                    start,
                    end,
                    inclusive: *inclusive,
                })),
            thir::Source::Array(expr) => {
                let Some(value) = self.value(expr)? else {
                    return Ok(None);
                };
                let value = self.take(value, expr.span);
                if value.ty == Type::Range {
                    // Значение диапазона — тот же int-цикл до эксклюзивной границы.
                    let mut part = |end| {
                        self.emit(
                            InstKind::RangePart {
                                range: value.value,
                                end,
                            },
                            Type::Int,
                            expr.span,
                        )
                        .value
                    };
                    let (start, end) = (part(false), part(true));
                    return Ok(Some(Source::Range {
                        start,
                        end,
                        inclusive: false,
                    }));
                }
                let Type::Array(element) = &value.ty else {
                    return Err(unsupported(expr.span, "iteration over this value"));
                };
                Ok(Some(Source::Array {
                    array: value.value,
                    element: (**element).clone(),
                }))
            }
            thir::Source::Varargs(local) => {
                let var = self.local_var(*local, span)?;
                let value = self.vars[var].value.clone().expect("locals hold a value");
                let Type::Array(element) = &value.ty else {
                    unreachable!("variadic bindings are arrays")
                };
                Ok(Some(Source::Array {
                    array: value.value,
                    element: (**element).clone(),
                }))
            }
            thir::Source::Map(expr) => {
                let Some(value) = self.value(expr)? else {
                    return Ok(None);
                };
                let value = self.take(value, expr.span);
                let Type::Map(key, item) = &value.ty else {
                    unreachable!("map sources are maps")
                };
                let keys = self.emit(
                    InstKind::MapKeys(value.value),
                    Type::Array(key.clone()),
                    expr.span,
                );
                let values = self.emit(
                    InstKind::MapValues(value.value),
                    Type::Array(item.clone()),
                    expr.span,
                );
                Ok(Some(Source::Map {
                    keys: keys.value,
                    values: values.value,
                    key: (**key).clone(),
                    value: (**item).clone(),
                }))
            }
            thir::Source::Iterating(expr) => self.cursor_source(expr),
        }
    }

    /// `Iterating<T>`: один `cursor()` перед обходом (§9.9).
    fn cursor_source(&mut self, expr: &'r thir::Expr) -> Result<Option<Source>> {
        let reg = self.ctx.reg;
        let (Some(iterating), Some(cursor_role)) = (reg.standard.iterating, reg.standard.cursor)
        else {
            return Err(unsupported(
                expr.span,
                "iteration without collections/iteration",
            ));
        };
        let Some(crate::analysis::ty::Ty::Role(_, args)) = &expr.ty else {
            return Err(unsupported(expr.span, "iteration over this value"));
        };
        let element_decl = args[0].subst(&self.env);
        let element = self.ctx.ir_type(&element_decl, expr.span)?;
        let cursor_id = self
            .ctx
            .role_instance(cursor_role, vec![element_decl], expr.span)?;
        let slot = |role: crate::analysis::ty::RoleId, name: &str| {
            reg.roles[role.0]
                .methods
                .iter()
                .position(|method| method.name == name && method.params.is_empty())
        };
        let (Some(open), Some(available), Some(item), Some(successor)) = (
            slot(iterating, "cursor"),
            slot(cursor_role, "available"),
            slot(cursor_role, "item"),
            slot(cursor_role, "successor"),
        ) else {
            return Err(unsupported(expr.span, "this iteration protocol"));
        };
        let Some(value) = self.value(expr)? else {
            return Ok(None);
        };
        let cursor = self.emit(
            InstKind::CallRole {
                receiver: value.value,
                slot: open,
                args: Vec::new(),
            },
            Type::Role(cursor_id),
            expr.span,
        );
        Ok(Some(Source::Cursor {
            cursor,
            element,
            available,
            item,
            successor,
        }))
    }

    /// Цикл по курсору: заголовок получает текущий курсор и состояние; на
    /// каждой итерации `available()`, `item()`, тело, затем `successor()`.
    #[allow(clippy::too_many_arguments)]
    fn cursor_loop(
        &mut self,
        start: Typed,
        element: Type,
        (available, item, successor): (usize, usize, usize),
        state: Vec<Typed>,
        scanned: &thir::Block,
        span: Span,
        value: bool,
        mut body: impl FnMut(&mut Self, Vec<Typed>, Vec<Typed>) -> Result<Option<Vec<ValueId>>>,
    ) -> Result<(Out, Vec<Typed>)> {
        let base = self.vars.len();
        let carried = self.carried_vars(scanned, span)?;
        let state_types: Vec<Type> = state.iter().map(|value| value.ty.clone()).collect();
        let mut extra = vec![start.ty.clone()];
        extra.extend(state_types.iter().cloned());
        let (head, head_params) = self.block_for_vars(&extra, &carried);
        let cursor = head_params[0];
        let state_params: Vec<Typed> = head_params[1..=state.len()]
            .iter()
            .zip(&state_types)
            .map(|(value, ty)| Typed {
                value: *value,
                ty: ty.clone(),
            })
            .collect();
        let mut args = vec![start.value];
        args.extend(state.iter().map(|value| value.value));
        args.extend(self.carried_args(&carried, span));
        self.b
            .terminate(TermKind::Br(Target { block: head, args }), span);
        self.b.switch_to(head);
        self.bind_carried(&carried, &head_params[1 + state.len()..]);
        let more = self.emit(
            InstKind::CallRole {
                receiver: cursor,
                slot: available,
                args: Vec::new(),
            },
            Type::Bool,
            span,
        );
        let iteration = self.b.new_block();
        let finished = self.b.new_block();
        self.b.terminate(
            TermKind::Cond {
                condition: more.value,
                then_target: Target::new(iteration),
                else_target: Target::new(finished),
            },
            span,
        );
        self.b.switch_to(finished);
        let exhausted = if value {
            let last = self.last_iteration(&state_params, span);
            self.arm(Out::Value(last))
        } else {
            self.arm(Out::Unit)
        }
        .expect("the exhausted edge is open");
        let (step, step_params) = self.block_for_vars(&state_types, &carried);

        self.b.switch_to(iteration);
        self.loops.push(LoopFrame {
            next: step,
            state: state_params.iter().map(|value| value.value).collect(),
            carried: carried.clone(),
            exits: vec![exhausted],
            base,
            depth: self.scopes.len(),
            want: value,
        });
        let current = self.emit(
            InstKind::CallRole {
                receiver: cursor,
                slot: item,
                args: Vec::new(),
            },
            element,
            span,
        );
        let body_out = body(self, vec![current], state_params.clone());
        if let Ok(Some(next_state)) = &body_out {
            if self.b.is_open() {
                let mut args = next_state.clone();
                args.extend(self.carried_args(&carried, span));
                self.b
                    .terminate(TermKind::Br(Target { block: step, args }), span);
            }
        }
        let frame = self.loops.pop().expect("loop frame pushed above");
        body_out?;

        self.b.switch_to(step);
        self.bind_carried(&carried, &step_params[state.len()..]);
        let advanced = self.emit(
            InstKind::CallRole {
                receiver: cursor,
                slot: successor,
                args: Vec::new(),
            },
            start.ty.clone(),
            span,
        );
        let mut args = vec![advanced.value];
        args.extend(step_params[..state.len()].iter().copied());
        args.extend(self.carried_args(&carried, span));
        self.b
            .terminate(TermKind::Br(Target { block: head, args }), span);
        let out = self.join(frame.exits, frame.base, value, span)?;
        Ok((out, state_params))
    }

    /// Обход источника: тело получает элемент(ы) и состояние.
    fn traverse(
        &mut self,
        source: Source,
        state: Vec<Typed>,
        scanned: &thir::Block,
        span: Span,
        body: impl FnMut(&mut Self, Vec<Typed>, Vec<Typed>) -> Result<Option<Vec<ValueId>>>,
    ) -> Result<(Out, Vec<Typed>)> {
        self.traverse_with(source, state, scanned, span, false, body)
    }

    /// Обход; `value` — `for` в позиции значения: состояние `[ran, last]`,
    /// выходы (исчерпание и `last v`) сливаются значением.
    fn traverse_with(
        &mut self,
        source: Source,
        state: Vec<Typed>,
        scanned: &thir::Block,
        span: Span,
        as_value: bool,
        mut body: impl FnMut(&mut Self, Vec<Typed>, Vec<Typed>) -> Result<Option<Vec<ValueId>>>,
    ) -> Result<(Out, Vec<Typed>)> {
        if source.untagged() {
            return Ok((Out::Unit, state));
        }
        match source {
            Source::Range {
                start,
                end,
                inclusive,
            } => self.counted_loop(
                start,
                end,
                inclusive,
                state,
                scanned,
                span,
                as_value,
                |lowerer, index, state| {
                    let index = Typed {
                        value: index,
                        ty: Type::Int,
                    };
                    body(lowerer, vec![index], state)
                },
            ),
            Source::Array { array, element } => {
                let zero = self.constant(Constant::Int(IntValue::from_i64(0)), span);
                let size = self.emit(InstKind::ArrayLen(array), Type::Int, span);
                self.counted_loop(
                    zero.value,
                    size.value,
                    false,
                    state,
                    scanned,
                    span,
                    as_value,
                    |lowerer, index, state| {
                        let item = lowerer.emit(
                            InstKind::ArrayGet { array, index },
                            element.clone(),
                            span,
                        );
                        body(lowerer, vec![item], state)
                    },
                )
            }
            Source::Map {
                keys,
                values,
                key,
                value,
            } => {
                let zero = self.constant(Constant::Int(IntValue::from_i64(0)), span);
                let size = self.emit(InstKind::ArrayLen(keys), Type::Int, span);
                self.counted_loop(
                    zero.value,
                    size.value,
                    false,
                    state,
                    scanned,
                    span,
                    as_value,
                    |lowerer, index, state| {
                        let k = lowerer.emit(
                            InstKind::ArrayGet { array: keys, index },
                            key.clone(),
                            span,
                        );
                        let v = lowerer.emit(
                            InstKind::ArrayGet {
                                array: values,
                                index,
                            },
                            value.clone(),
                            span,
                        );
                        body(lowerer, vec![k, v], state)
                    },
                )
            }
            Source::Cursor {
                cursor,
                element,
                available,
                item,
                successor,
            } => self.cursor_loop(
                cursor,
                element,
                (available, item, successor),
                state,
                scanned,
                span,
                as_value,
                body,
            ),
        }
    }

    /// `for source -> binding { body }`; `value` — сам `for` в позиции значения.
    pub(super) fn for_expr(
        &mut self,
        source: &'r thir::Source,
        binding: LocalId,
        body: &'r thir::Block,
        value: Option<&thir::Expr>,
        span: Span,
    ) -> Result<Out> {
        let Some(source) = self.source(source, span)? else {
            return Ok(Out::Diverged);
        };
        if let Some(expr) = value {
            return self.for_value(source, binding, body, expr, span);
        }
        let (out, _) = self.traverse(source, Vec::new(), body, span, |lowerer, mut items, _| {
            lowerer.push_scope();
            lowerer.bind_local(binding, items.remove(0));
            let out = lowerer.block(body, false);
            lowerer.pop_scope();
            out?;
            Ok(Some(Vec::new()))
        })?;
        Ok(out)
    }

    /// Значение `for` — значение тела последней итерации. Состояние обхода —
    /// признак «тело исполнялось» и последнее значение (`undef` до первой
    /// итерации); без итераций — ошибка Perl `A void result cannot be used as
    /// a value` (анализ предупреждает, если источник может быть пуст).
    fn for_value(
        &mut self,
        source: Source,
        binding: LocalId,
        body: &'r thir::Block,
        expr: &thir::Expr,
        span: Span,
    ) -> Result<Out> {
        let ty = self.expr_type(expr)?.taken();
        let ran = self.constant(Constant::Bool(false), span);
        let last = self.emit(InstKind::Undef, ty.clone(), span);
        let (out, _) = self.traverse_with(
            source,
            vec![ran, last],
            body,
            span,
            true,
            |lowerer, mut items, _| {
                lowerer.push_scope();
                lowerer.bind_local(binding, items.remove(0));
                let out = lowerer.block(body, true);
                lowerer.pop_scope();
                match out? {
                    Out::Value(value) => {
                        let value = lowerer.normalize(&value, &ty, span);
                        let ran = lowerer.constant(Constant::Bool(true), span);
                        Ok(Some(vec![ran.value, value.value]))
                    }
                    Out::Diverged => Ok(None),
                    Out::Unit => Err(crate::lowering::LoweringError::invalid(
                        body.span,
                        "internal compiler error: a for body in value position produced no value",
                    )),
                }
            },
        )?;
        Ok(out)
    }

    /// Исчерпание обхода-значения: последнее значение, если тело исполнялось.
    fn last_iteration(&mut self, state: &[Typed], span: Span) -> Typed {
        let done = self.b.new_block();
        let missing = self.b.new_block();
        self.b.terminate(
            TermKind::Cond {
                condition: state[0].value,
                then_target: Target::new(done),
                else_target: Target::new(missing),
            },
            span,
        );
        self.b.switch_to(missing);
        self.sync_unwind();
        self.b.terminate(
            TermKind::Trap {
                code: TrapCode::Empty,
                message: "A void result cannot be used as a value".to_string(),
            },
            span,
        );
        self.b.switch_to(done);
        self.sync_unwind();
        state[1].clone()
    }

    /// `sink <- source` (§15): `sink.write(source)` через роль `Writing`, source —
    /// ссылка `ByteSource` (строка и `@byte` — через адаптеры); `source...`
    /// пишет элементы по порядку. Значение — исходный sink.
    pub(super) fn stream_write(
        &mut self,
        sink: &'r thir::Expr,
        source: &'r thir::Expr,
        unpack: bool,
        span: Span,
    ) -> Result<Out> {
        let reg = self.ctx.reg;
        let (Some(writing), Some(byte_source)) = (reg.standard.writing, reg.standard.byte_source)
        else {
            return Err(crate::lowering::LoweringError::invalid(
                span,
                "internal compiler error: `<-` without the io/stream roles",
            ));
        };
        let slot = reg.roles[writing.0]
            .methods
            .iter()
            .position(|method| method.name == "write")
            .expect("io/stream Writing declares write()");
        let Some(target) = self.value(sink)? else {
            return Ok(Out::Diverged);
        };
        let target = self.take(target, sink.span);
        let writing_id = self.ctx.role_instance(writing, Vec::new(), span)?;
        let writing_ty = Ty::role(writing);
        let writer = self.role_wrap(
            target.clone(),
            sink.ty.as_ref(),
            Some(&writing_ty),
            Type::Role(writing_id),
            sink.span,
        )?;
        let Some(value) = self.value(source)? else {
            return Ok(Out::Diverged);
        };
        let value = self.take(value, source.span);
        let byte_id = self.ctx.role_instance(byte_source, Vec::new(), span)?;
        let byte_ty = Ty::role(byte_source);
        let source_ty = source.ty.as_ref().map(|ty| ty.subst(&self.env).taken());
        let write = |lowerer: &mut Self, item: Typed, item_ty: Option<&Ty>| -> Result<()> {
            let item = lowerer.role_wrap(
                item,
                item_ty,
                Some(&byte_ty),
                Type::Role(byte_id),
                source.span,
            )?;
            lowerer.b.emit_void(
                InstKind::CallRole {
                    receiver: writer.value,
                    slot,
                    args: vec![item.value],
                },
                span,
            );
            Ok(())
        };
        if !unpack {
            write(self, value, source_ty.as_ref())?;
            return Ok(Out::Value(target));
        }
        let element_ty = match &source_ty {
            Some(Ty::Array(element) | Ty::Varargs(element)) => Some((**element).clone()),
            _ => None,
        };
        let Type::Array(element) = value.ty.clone() else {
            return Err(crate::lowering::LoweringError::invalid(
                span,
                "internal compiler error: `<- xs...` of a non-array",
            ));
        };
        let empty = thir::Block {
            stmts: Vec::new(),
            span,
        };
        let source = Source::Array {
            array: value.value,
            element: *element,
        };
        let (out, _) =
            self.traverse(source, Vec::new(), &empty, span, |lowerer, mut items, _| {
                write(lowerer, items.remove(0), element_ty.as_ref())?;
                Ok(Some(Vec::new()))
            })?;
        Ok(match out {
            Out::Diverged => Out::Diverged,
            _ => Out::Value(target),
        })
    }

    /// Тело функционального блока с привязками; `None` — управление ушло.
    fn functional_body(
        &mut self,
        bindings: &[LocalId],
        values: Vec<Typed>,
        body: &'r thir::Block,
    ) -> Result<Option<Typed>> {
        self.push_scope();
        for (local, value) in bindings.iter().zip(values) {
            self.bind_local(*local, value);
        }
        let out = self.block(body, true);
        self.pop_scope();
        match out? {
            Out::Value(value) => Ok(Some(value)),
            Out::Diverged => Ok(None),
            Out::Unit => Err(crate::lowering::LoweringError::invalid(
                body.span,
                "internal compiler error: a functional block produced no value",
            )),
        }
    }

    /// `receiver.map() -> v { }`, `.filter() -> v { }`, `.reduce(init) -> acc, v { }`.
    pub(super) fn functional(
        &mut self,
        functional: &'r thir::Functional,
        expr: &'r thir::Expr,
        span: Span,
    ) -> Result<Out> {
        let Some(source) = self.source(&functional.source, span)? else {
            return Ok(Out::Diverged);
        };
        let items = source.items();
        let is_map = matches!(source, Source::Map { .. });
        let body = &functional.body;
        let bindings = &functional.bindings;
        match functional.op {
            FunctionalOp::Map => {
                let result = self.expr_type(expr)?;
                let Type::Array(output) = result.clone() else {
                    unreachable!("map produces an array")
                };
                if source.untagged() {
                    return Ok(Out::Value(self.emit(
                        InstKind::ArrayNew(Vec::new()),
                        result,
                        span,
                    )));
                }
                let builder = self.emit(InstKind::BuilderNew, Type::Builder(output.clone()), span);
                let (out, state) = self.traverse(
                    source,
                    vec![builder],
                    body,
                    span,
                    |lowerer, items, state| {
                        let Some(value) = lowerer.functional_body(bindings, items, body)? else {
                            return Ok(None);
                        };
                        let value = lowerer.normalize(&value, &output, span);
                        let pushed = lowerer.emit(
                            InstKind::BuilderPush {
                                builder: state[0].value,
                                value: value.value,
                            },
                            state[0].ty.clone(),
                            span,
                        );
                        Ok(Some(vec![pushed.value]))
                    },
                )?;
                self.finish_builder(out, &state[0], span)
            }
            FunctionalOp::Filter => {
                if source.untagged() {
                    let never = Box::new(Type::Never);
                    let empty = self.emit(InstKind::ArrayNew(Vec::new()), Type::Array(never), span);
                    return Ok(Out::Value(empty));
                }
                let builder = if is_map {
                    self.emit(
                        InstKind::MapBuilderNew,
                        Type::MapBuilder(Box::new(items[0].clone()), Box::new(items[1].clone())),
                        span,
                    )
                } else {
                    self.emit(
                        InstKind::BuilderNew,
                        Type::Builder(Box::new(items[0].clone())),
                        span,
                    )
                };
                let (out, state) = self.traverse(
                    source,
                    vec![builder],
                    body,
                    span,
                    |lowerer, items, state| {
                        let Some(keep) = lowerer.functional_body(bindings, items.clone(), body)?
                        else {
                            return Ok(None);
                        };
                        let builder = state[0].clone();
                        let push = lowerer.b.new_block();
                        let skip = lowerer.b.new_block();
                        let (join, joined) =
                            lowerer.block_with(&[], std::slice::from_ref(&builder.ty));
                        lowerer.b.terminate(
                            TermKind::Cond {
                                condition: keep.value,
                                then_target: Target::new(push),
                                else_target: Target::new(skip),
                            },
                            body.span,
                        );
                        lowerer.b.switch_to(push);
                        let kind = match items.as_slice() {
                            [key, value] => InstKind::MapBuilderPut {
                                builder: builder.value,
                                key: key.value,
                                value: value.value,
                            },
                            [item] => InstKind::BuilderPush {
                                builder: builder.value,
                                value: item.value,
                            },
                            _ => unreachable!("sources yield one or two values"),
                        };
                        let pushed = lowerer.emit(kind, builder.ty.clone(), span);
                        lowerer.b.terminate(
                            TermKind::Br(Target {
                                block: join,
                                args: vec![pushed.value],
                            }),
                            span,
                        );
                        lowerer.b.switch_to(skip);
                        lowerer.b.terminate(
                            TermKind::Br(Target {
                                block: join,
                                args: vec![builder.value],
                            }),
                            span,
                        );
                        lowerer.b.switch_to(join);
                        lowerer.sync_unwind();
                        Ok(Some(vec![joined[0]]))
                    },
                )?;
                self.finish_builder(out, &state[0], span)
            }
            FunctionalOp::Reduce => {
                let initial = functional
                    .initial
                    .as_ref()
                    .expect("reduce has an initial value");
                let Some(start) = self.value(initial)? else {
                    return Ok(Out::Diverged);
                };
                let start = self.take(start, initial.span);
                let accumulator = start.ty.clone();
                let (out, state) =
                    self.traverse(source, vec![start], body, span, |lowerer, items, state| {
                        let mut values = vec![state[0].clone()];
                        values.extend(items);
                        let Some(value) = lowerer.functional_body(bindings, values, body)? else {
                            return Ok(None);
                        };
                        let value = lowerer.normalize(&value, &accumulator, span);
                        Ok(Some(vec![value.value]))
                    })?;
                Ok(match out {
                    Out::Diverged => Out::Diverged,
                    _ => Out::Value(state[0].clone()),
                })
            }
        }
    }

    fn finish_builder(&mut self, out: Out, builder: &Typed, span: Span) -> Result<Out> {
        if matches!(out, Out::Diverged) {
            return Ok(Out::Diverged);
        }
        let (kind, ty) = match &builder.ty {
            Type::Builder(element) => (
                InstKind::BuilderFinish(builder.value),
                Type::Array(element.clone()),
            ),
            Type::MapBuilder(key, value) => (
                InstKind::MapBuilderFinish(builder.value),
                Type::Map(key.clone(), value.clone()),
            ),
            _ => unreachable!("the loop state is a builder"),
        };
        Ok(Out::Value(self.emit(kind, ty, span)))
    }

    /// `[a, b, 1..=3]`: диапазоны раскрываются сборщиком.
    pub(super) fn array_literal(
        &mut self,
        parts: &'r [ArrayPart],
        expr: &'r thir::Expr,
        span: Span,
    ) -> Result<Out> {
        enum Part {
            Value(Typed),
            Range(ValueId, ValueId, bool),
        }
        let array_type = self.expr_type(expr)?;
        // Гетерогенный литерал — `@any` (R08): каждый элемент в box.
        let boxed = array_type == Type::Array(Box::new(Type::Any));
        if parts.len() > LARGE_LITERAL {
            return self.streamed_array(parts, array_type, boxed, span);
        }
        let mut values = Vec::new();
        for part in parts {
            match part {
                ArrayPart::Range { .. } if boxed => {
                    return Err(unsupported(span, "ranges inside `@any` literals"));
                }
                ArrayPart::Range {
                    start,
                    end,
                    inclusive,
                } => {
                    let Some(first) = self.value(start)? else {
                        return Ok(Out::Diverged);
                    };
                    let Some(last) = self.value(end)? else {
                        return Ok(Out::Diverged);
                    };
                    values.push(Part::Range(first.value, last.value, *inclusive));
                }
                // Сохранение в агрегат потребляет successor.
                ArrayPart::Value(value) => {
                    let Some(typed) = self.value(value)? else {
                        return Ok(Out::Diverged);
                    };
                    let typed = self.take(typed, value.span);
                    let typed = if boxed && typed.ty != Type::Any {
                        self.emit(InstKind::AnyBox(typed.value), Type::Any, value.span)
                    } else {
                        typed
                    };
                    values.push(Part::Value(typed));
                }
            }
        }
        let finished = values;
        if finished.iter().all(|part| matches!(part, Part::Value(..))) {
            let ids = finished
                .into_iter()
                .map(|part| match part {
                    Part::Value(value) => value.value,
                    Part::Range(..) => unreachable!("checked above"),
                })
                .collect();
            return Ok(Out::Value(self.emit(
                InstKind::ArrayNew(ids),
                array_type,
                span,
            )));
        }
        let Type::Array(element) = &array_type else {
            unreachable!("array literals are arrays")
        };
        let builder_type = Type::Builder(element.clone());
        let mut builder = self.emit(InstKind::BuilderNew, builder_type.clone(), span);
        for part in finished {
            let kind = match part {
                Part::Value(value) => InstKind::BuilderPush {
                    builder: builder.value,
                    value: value.value,
                },
                Part::Range(start, end, inclusive) => InstKind::BuilderAppendRange {
                    builder: builder.value,
                    start,
                    end,
                    inclusive,
                },
            };
            builder = self.emit(kind, builder_type.clone(), span);
        }
        Ok(Out::Value(self.emit(
            InstKind::BuilderFinish(builder.value),
            array_type,
            span,
        )))
    }

    /// Большой литерал: каждый элемент сразу уходит в сборщик, так что между
    /// элементами живы только сборщик и внешние значения (функцию можно
    /// разрезать для лимита метода JVM; см. `codegen::jvm::outline`).
    fn streamed_array(
        &mut self,
        parts: &'r [ArrayPart],
        array_type: Type,
        boxed: bool,
        span: Span,
    ) -> Result<Out> {
        let Type::Array(element) = &array_type else {
            unreachable!("array literals are arrays")
        };
        let builder_type = Type::Builder(element.clone());
        let mut builder = self.emit(InstKind::BuilderNew, builder_type.clone(), span);
        for part in parts {
            let kind = match part {
                ArrayPart::Range { .. } if boxed => {
                    return Err(unsupported(span, "ranges inside `@any` literals"));
                }
                ArrayPart::Range {
                    start,
                    end,
                    inclusive,
                } => {
                    let Some(first) = self.value(start)? else {
                        return Ok(Out::Diverged);
                    };
                    let Some(last) = self.value(end)? else {
                        return Ok(Out::Diverged);
                    };
                    InstKind::BuilderAppendRange {
                        builder: builder.value,
                        start: first.value,
                        end: last.value,
                        inclusive: *inclusive,
                    }
                }
                ArrayPart::Value(value) => {
                    let Some(typed) = self.value(value)? else {
                        return Ok(Out::Diverged);
                    };
                    let typed = self.take(typed, value.span);
                    let typed = if boxed && typed.ty != Type::Any {
                        self.emit(InstKind::AnyBox(typed.value), Type::Any, value.span)
                    } else {
                        typed
                    };
                    InstKind::BuilderPush {
                        builder: builder.value,
                        value: typed.value,
                    }
                }
            };
            builder = self.emit(kind, builder_type.clone(), span);
        }
        Ok(Out::Value(self.emit(
            InstKind::BuilderFinish(builder.value),
            array_type,
            span,
        )))
    }

    /// `[k: v, …]`: пары слева направо (ключ, затем значение).
    pub(super) fn map_literal(
        &mut self,
        entries: &'r [(thir::Expr, thir::Expr)],
        expr: &'r thir::Expr,
        span: Span,
    ) -> Result<Out> {
        let ty = self.expr_type(expr)?;
        // Гетерогенные значения — `%K:any` (R08): каждое значение в box.
        let boxed = matches!(&ty, Type::Map(_, value) if **value == Type::Any);
        let mut pairs = Vec::new();
        for (key, value) in entries {
            let Some(key) = self.value(key)? else {
                return Ok(Out::Diverged);
            };
            let key = self.take(key, span);
            let Some(value) = self.value(value)? else {
                return Ok(Out::Diverged);
            };
            let value = self.take(value, span);
            let value = if boxed && value.ty != Type::Any {
                self.emit(InstKind::AnyBox(value.value), Type::Any, span)
            } else {
                value
            };
            pairs.push((key.value, value.value));
        }
        Ok(Out::Value(self.emit(InstKind::MapNew(pairs), ty, span)))
    }

    /// `xs[i]` и `xs[i] else fallback` (fallback вычисляется только при промахе).
    pub(super) fn index(
        &mut self,
        object: &'r thir::Expr,
        index: &'r thir::Expr,
        fallback: Option<&'r thir::Expr>,
        want: bool,
        span: Span,
    ) -> Result<Out> {
        let Some(target) = self.value(object)? else {
            return Ok(Out::Diverged);
        };
        let target = self.take(target, object.span);
        let Some(position) = self.value(index)? else {
            return Ok(Out::Diverged);
        };
        let position = self.take(position, index.span);
        let (element, map) = match &target.ty {
            Type::Array(element) => ((**element).clone(), false),
            Type::Map(key, value) => {
                // `[:][k] else fallback` — всегда fallback.
                if **key == Type::Never {
                    let fallback = fallback.expect("analysis requires a fallback");
                    return Ok(match self.expr(fallback, want)? {
                        Out::Value(value) => Out::Value(self.take(value, fallback.span)),
                        other => other,
                    });
                }
                ((**value).clone(), true)
            }
            _ => return Err(unsupported(object.span, "indexing this value")),
        };
        let Some(fallback) = fallback else {
            let kind = if map {
                InstKind::MapGet {
                    map: target.value,
                    key: position.value,
                }
            } else {
                InstKind::ArrayGet {
                    array: target.value,
                    index: position.value,
                }
            };
            return Ok(Out::Value(self.emit(kind, element, span)));
        };
        let base = self.vars.len();
        let (found, found_params) = self.block_with(&[], std::slice::from_ref(&element));
        let missing = self.b.new_block();
        let lookup = if map {
            TermKind::MapLookup {
                map: target.value,
                key: position.value,
                found: Target::new(found),
                missing: Target::new(missing),
            }
        } else {
            TermKind::ArrayLookup {
                array: target.value,
                index: position.value,
                found: Target::new(found),
                missing: Target::new(missing),
            }
        };
        self.b.terminate(lookup, span);
        let env = self.snapshot();
        self.b.switch_to(found);
        self.sync_unwind();
        let mut arms: Vec<Arm> = self
            .arm(Out::Value(Typed {
                value: found_params[0],
                ty: element,
            }))
            .into_iter()
            .collect();
        self.restore(&env);
        self.b.switch_to(missing);
        let other = self.expr(fallback, true)?;
        let other = match other {
            Out::Value(value) => Out::Value(self.take(value, fallback.span)),
            other => other,
        };
        arms.extend(self.arm(other));
        self.join(arms, base, want, span)
    }

    /// `@int(...)`, `@string(...)`: элементы и `xs...` по порядку.
    pub(super) fn array_cast(
        &mut self,
        element: CastTarget,
        parts: &'r [CastPart],
        span: Span,
    ) -> Result<Out> {
        let element_type = match element {
            CastTarget::Int => Type::Int,
            CastTarget::Byte => Type::Byte,
            CastTarget::Float => Type::Float,
            CastTarget::Str => Type::Str,
            CastTarget::Rat => return Err(unsupported(span, "`rat` values")),
        };
        // Сначала все операнды слева направо, затем преобразования по порядку.
        enum Operand {
            One(Typed, Span, Option<usize>),
            Spread(Typed, Span, usize),
        }
        let mut operands = Vec::new();
        for (index, part) in parts.iter().enumerate() {
            match part {
                CastPart::One(expr) => {
                    // Узел `Cast` элемента: значение, затем преобразование.
                    let (inner, position) = match &expr.kind {
                        ExprKind::Cast {
                            value, position, ..
                        } => (&**value, *position),
                        _ => (expr, Some(index + 1)),
                    };
                    let Some(value) = self.value(inner)? else {
                        return Ok(Out::Diverged);
                    };
                    let value = self.take(value, inner.span);
                    operands.push(Operand::One(value, inner.span, position));
                }
                CastPart::Spread(expr) => {
                    let Some(value) = self.value(expr)? else {
                        return Ok(Out::Diverged);
                    };
                    let value = self.take(value, expr.span);
                    operands.push(Operand::Spread(value, expr.span, index + 1));
                }
            }
        }
        let builder_type = Type::Builder(Box::new(element_type.clone()));
        let mut builder = self.emit(InstKind::BuilderNew, builder_type.clone(), span);
        for operand in operands {
            match operand {
                Operand::One(value, value_span, position) => {
                    let converted = self.convert(value, element, value_span, position)?;
                    builder = self.emit(
                        InstKind::BuilderPush {
                            builder: builder.value,
                            value: converted.value,
                        },
                        builder_type.clone(),
                        span,
                    );
                }
                Operand::Spread(array, array_span, position) => {
                    let Type::Array(source) = &array.ty else {
                        unreachable!("spreads are arrays")
                    };
                    if **source == element_type || **source == Type::Never {
                        builder = self.emit(
                            InstKind::BuilderAppend {
                                builder: builder.value,
                                array: array.value,
                            },
                            builder_type.clone(),
                            span,
                        );
                        continue;
                    }
                    let empty = thir::Block {
                        stmts: Vec::new(),
                        span: array_span,
                    };
                    let source_element = (**source).clone();
                    let (_, state) = self.traverse(
                        Source::Array {
                            array: array.value,
                            element: source_element,
                        },
                        vec![builder],
                        &empty,
                        span,
                        |lowerer, mut items, state| {
                            let item = items.remove(0);
                            let converted =
                                lowerer.convert(item, element, array_span, Some(position))?;
                            let pushed = lowerer.emit(
                                InstKind::BuilderPush {
                                    builder: state[0].value,
                                    value: converted.value,
                                },
                                state[0].ty.clone(),
                                span,
                            );
                            Ok(Some(vec![pushed.value]))
                        },
                    )?;
                    builder = state[0].clone();
                }
            }
        }
        Ok(Out::Value(self.emit(
            InstKind::BuilderFinish(builder.value),
            Type::Array(Box::new(element_type)),
            span,
        )))
    }
}
