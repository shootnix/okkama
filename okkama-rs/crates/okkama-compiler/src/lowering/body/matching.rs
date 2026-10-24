//! Операторы, `&&`/`||`, `match` и `try`/`catch` (§16, §19.2–19.3).
//!
//! Вид оператора выбран анализом (`BinaryOp`, `UnaryOp`); здесь — только
//! инструкции. `match` сравнивает субъект с образцами тем же `==`, что и
//! выражения; `try` строит dispatch по типам `catch` в порядке исходника.

use super::calls::ir_compare;
use super::{Arm, BodyLowerer, Out, TryCtx, Typed};
use crate::analysis::thir::{self, ArithOp, BinaryOp, BitOp, CompareKind, NumericKind, UnaryOp};
use crate::ir::{
    self, CatchType, Constant, InstKind, IntBinaryOp, IntUnaryOp, Target, TermKind, Type,
};
use crate::lowering::Result;
use crate::source::Span;

/// Сравнение двух значений одной категории `kind`.
pub(super) fn compare_inst(
    kind: CompareKind,
    op: ir::CompareOp,
    left: ir::ValueId,
    right: ir::ValueId,
) -> InstKind {
    match kind {
        CompareKind::Int => InstKind::IntCompare { op, left, right },
        CompareKind::Byte => InstKind::ByteCompare { op, left, right },
        CompareKind::Float => InstKind::FloatCompare { op, left, right },
        CompareKind::Rat => InstKind::RatCompare { op, left, right },
        CompareKind::Bool => InstKind::BoolCompare { op, left, right },
        CompareKind::Char => InstKind::CharCompare { op, left, right },
        CompareKind::Str => InstKind::StrCompare { op, left, right },
        CompareKind::Enum => InstKind::EnumCompare { op, left, right },
        CompareKind::Value => InstKind::ValCompare { op, left, right },
    }
}

impl<'r> BodyLowerer<'_, 'r, '_> {
    pub(super) fn unary(
        &mut self,
        op: UnaryOp,
        operand: &'r thir::Expr,
        span: Span,
    ) -> Result<Out> {
        let Some(value) = self.value(operand)? else {
            return Ok(Out::Diverged);
        };
        let (kind, ty) = match op {
            UnaryOp::Not => (InstKind::BoolNot(value.value), Type::Bool),
            UnaryOp::Negate(NumericKind::Int) => (
                InstKind::IntUnary {
                    op: IntUnaryOp::Neg,
                    operand: value.value,
                },
                Type::Int,
            ),
            UnaryOp::Negate(NumericKind::Float) => (
                InstKind::Float {
                    op: ir::FloatOp::Neg,
                    args: vec![value.value],
                },
                Type::Float,
            ),
            // Арифметика над `byte` даёт `int` (§16.2).
            UnaryOp::Negate(NumericKind::Byte) => {
                let widened = self.emit(InstKind::Cvt(value.value), Type::Int, span);
                (
                    InstKind::IntUnary {
                        op: IntUnaryOp::Neg,
                        operand: widened.value,
                    },
                    Type::Int,
                )
            }
            UnaryOp::Negate(NumericKind::Rat) => (
                InstKind::Rat {
                    op: ir::RatOp::Neg,
                    args: vec![value.value],
                },
                Type::Rat,
            ),
            UnaryOp::BitNot { byte: false } => (
                InstKind::IntUnary {
                    op: IntUnaryOp::Not,
                    operand: value.value,
                },
                Type::Int,
            ),
            UnaryOp::BitNot { byte: true } => (InstKind::ByteNot(value.value), Type::Byte),
        };
        Ok(Out::Value(self.emit(kind, ty, span)))
    }

    pub(super) fn logical(
        &mut self,
        and: bool,
        left: &'r thir::Expr,
        right: &'r thir::Expr,
        span: Span,
    ) -> Result<Out> {
        let Some(lhs) = self.value(left)? else {
            return Ok(Out::Diverged);
        };
        let base = self.vars.len();
        let rhs_block = self.b.new_block();
        let short_block = self.b.new_block();
        let (then_target, else_target) = if and {
            (Target::new(rhs_block), Target::new(short_block))
        } else {
            (Target::new(short_block), Target::new(rhs_block))
        };
        self.b.terminate(
            TermKind::Cond {
                condition: lhs.value,
                then_target,
                else_target,
            },
            span,
        );
        let mut arms = Vec::new();
        // Короткое замыкание: `false` для `&&`, `true` для `||`.
        self.b.switch_to(short_block);
        let short = self.constant(Constant::Bool(!and), span);
        arms.extend(self.arm(Out::Value(short)));
        self.b.switch_to(rhs_block);
        if let Some(rhs) = self.value(right)? {
            arms.extend(self.arm(Out::Value(rhs)));
        }
        self.join(arms, base, true, span)
    }

    /// `a.order(b)` → `compare.Order`: `less`, иначе `equal`, иначе `greater`
    /// (порядок случаев в объявлении `enum Order`).
    pub(super) fn order(
        &mut self,
        kind: CompareKind,
        left: ir::ValueId,
        right: ir::ValueId,
        order: ir::EnumId,
        span: Span,
    ) -> Result<Out> {
        let base = self.vars.len();
        let mut arms = Vec::new();
        for (case, op) in [(0, ir::CompareOp::Lt), (1, ir::CompareOp::Eq)] {
            let condition = self.emit(compare_inst(kind, op, left, right), Type::Bool, span);
            let hit = self.b.new_block();
            let rest = self.b.new_block();
            self.b.terminate(
                TermKind::Cond {
                    condition: condition.value,
                    then_target: Target::new(hit),
                    else_target: Target::new(rest),
                },
                span,
            );
            self.b.switch_to(hit);
            let value = self.emit(
                InstKind::EnumCase { ty: order, case },
                Type::Enum(order),
                span,
            );
            arms.extend(self.arm(Out::Value(value)));
            self.b.switch_to(rest);
        }
        let greater = self.emit(
            InstKind::EnumCase { ty: order, case: 2 },
            Type::Enum(order),
            span,
        );
        arms.extend(self.arm(Out::Value(greater)));
        self.join(arms, base, true, span)
    }

    pub(super) fn binary(
        &mut self,
        op: BinaryOp,
        left: &'r thir::Expr,
        right: &'r thir::Expr,
        span: Span,
    ) -> Result<Out> {
        // Операнды вычисляются ровно один раз, слева направо.
        let Some(lhs) = self.value(left)? else {
            return Ok(Out::Diverged);
        };
        let Some(rhs) = self.value(right)? else {
            return Ok(Out::Diverged);
        };
        Ok(Out::Value(self.binary_values(op, lhs, rhs, span)?))
    }

    fn binary_values(&mut self, op: BinaryOp, lhs: Typed, rhs: Typed, span: Span) -> Result<Typed> {
        let int_op = |op: ArithOp| match op {
            ArithOp::Add => Some(IntBinaryOp::Add),
            ArithOp::Sub => Some(IntBinaryOp::Sub),
            ArithOp::Mul => Some(IntBinaryOp::Mul),
            ArithOp::Mod => Some(IntBinaryOp::Mod),
            ArithOp::Div => None,
        };
        let bit_op = |op: BitOp| match op {
            BitOp::And => IntBinaryOp::And,
            BitOp::Or => IntBinaryOp::Or,
            BitOp::Xor => IntBinaryOp::Xor,
            BitOp::Shl => IntBinaryOp::Shl,
            BitOp::Shr => IntBinaryOp::Shr,
            BitOp::Ushr => IntBinaryOp::Ushr,
        };
        let (kind, ty) = match op {
            BinaryOp::Arith(NumericKind::Int, arith) => match int_op(arith) {
                Some(op) => (
                    InstKind::IntBinary {
                        op,
                        left: lhs.value,
                        right: rhs.value,
                    },
                    Type::Int,
                ),
                // `int / int` — точное `rat` (делитель 0 — trap).
                None => (
                    InstKind::Rat {
                        op: ir::RatOp::New,
                        args: vec![lhs.value, rhs.value],
                    },
                    Type::Rat,
                ),
            },
            // Арифметика над двумя `byte` идёт в `int` (§16.2).
            BinaryOp::Arith(NumericKind::Byte, arith) => match int_op(arith) {
                Some(op) => {
                    let left = self.emit(InstKind::Cvt(lhs.value), Type::Int, span);
                    let right = self.emit(InstKind::Cvt(rhs.value), Type::Int, span);
                    (
                        InstKind::IntBinary {
                            op,
                            left: left.value,
                            right: right.value,
                        },
                        Type::Int,
                    )
                }
                None => {
                    let left = self.emit(InstKind::Cvt(lhs.value), Type::Int, span);
                    let right = self.emit(InstKind::Cvt(rhs.value), Type::Int, span);
                    (
                        InstKind::Rat {
                            op: ir::RatOp::New,
                            args: vec![left.value, right.value],
                        },
                        Type::Rat,
                    )
                }
            },
            BinaryOp::Arith(NumericKind::Float, arith) => (
                InstKind::Float {
                    op: match arith {
                        ArithOp::Add => ir::FloatOp::Add,
                        ArithOp::Sub => ir::FloatOp::Sub,
                        ArithOp::Mul => ir::FloatOp::Mul,
                        ArithOp::Div => ir::FloatOp::Div,
                        ArithOp::Mod => ir::FloatOp::Mod,
                    },
                    args: vec![lhs.value, rhs.value],
                },
                Type::Float,
            ),
            BinaryOp::Arith(NumericKind::Rat, arith) => (
                InstKind::Rat {
                    op: match arith {
                        ArithOp::Add => ir::RatOp::Add,
                        ArithOp::Sub => ir::RatOp::Sub,
                        ArithOp::Mul => ir::RatOp::Mul,
                        ArithOp::Div => ir::RatOp::Div,
                        ArithOp::Mod => ir::RatOp::Mod,
                    },
                    args: vec![lhs.value, rhs.value],
                },
                Type::Rat,
            ),
            BinaryOp::Bits { byte: false, op } => (
                InstKind::IntBinary {
                    op: bit_op(op),
                    left: lhs.value,
                    right: rhs.value,
                },
                Type::Int,
            ),
            BinaryOp::Bits { byte: true, op } => (
                InstKind::ByteBinary {
                    op: bit_op(op),
                    left: lhs.value,
                    right: rhs.value,
                },
                Type::Byte,
            ),
            BinaryOp::Concat => (
                InstKind::StrConcat {
                    left: lhs.value,
                    right: rhs.value,
                },
                Type::Str,
            ),
            BinaryOp::Compare(kind, compare) => {
                // Глубокое равенство: вычисление над значением потребляет successor.
                let (lhs, rhs) = if kind == CompareKind::Value {
                    (self.take(lhs, span), self.take(rhs, span))
                } else {
                    (lhs, rhs)
                };
                (
                    compare_inst(kind, ir_compare(compare), lhs.value, rhs.value),
                    Type::Bool,
                )
            }
        };
        Ok(self.emit(kind, ty, span))
    }

    /// Равенство субъекта `match` с образцом.
    fn pattern_equal(&mut self, subject: &Typed, value: Typed, span: Span) -> Result<Typed> {
        let kind = match subject.ty.taken() {
            Type::Int => CompareKind::Int,
            Type::Byte => CompareKind::Byte,
            Type::Float => CompareKind::Float,
            Type::Bool => CompareKind::Bool,
            Type::Char => CompareKind::Char,
            Type::Str => CompareKind::Str,
            Type::Enum(_) => CompareKind::Enum,
            _ => CompareKind::Value,
        };
        self.binary_values(
            BinaryOp::Compare(kind, thir::CompareOp::Eq),
            subject.clone(),
            value,
            span,
        )
    }

    pub(super) fn match_body(
        &mut self,
        body: &'r thir::Match,
        want: bool,
        span: Span,
    ) -> Result<Out> {
        let Some(subject) = self.value(&body.subject)? else {
            return Ok(Out::Diverged);
        };
        let subject = self.take(subject, body.subject.span);
        let base = self.vars.len();
        let mut arms: Vec<Arm> = Vec::new();
        for (index, (patterns, block)) in body.cases.iter().enumerate() {
            if body.exhaustive && index + 1 == body.cases.len() {
                let out = self.branch(block, want)?;
                arms.extend(self.arm(out));
                return self.join(arms, base, want, span);
            }
            let Some(hit) = self.case_matches(&subject, patterns, span)? else {
                // Образец ушёл (`return` в нём): дальше case недостижимы.
                return self.join(arms, base, want, span);
            };
            let taken = self.b.new_block();
            let next = self.b.new_block();
            self.b.terminate(
                TermKind::Cond {
                    condition: hit.value,
                    then_target: Target::new(taken),
                    else_target: Target::new(next),
                },
                block.span,
            );
            let env = self.snapshot();
            self.b.switch_to(taken);
            let out = self.branch(block, want)?;
            arms.extend(self.arm(out));
            self.restore(&env);
            self.b.switch_to(next);
        }
        let out = match &body.else_block {
            Some(block) => self.branch(block, want)?,
            None => Out::Unit,
        };
        arms.extend(self.arm(out));
        self.join(arms, base, want, span)
    }

    /// `subject == p1 || subject == p2 || …` с коротким замыканием.
    fn case_matches(
        &mut self,
        subject: &Typed,
        patterns: &'r [thir::Expr],
        span: Span,
    ) -> Result<Option<Typed>> {
        let base = self.vars.len();
        let mut arms: Vec<Arm> = Vec::new();
        let first_span = patterns.first().map_or(span, |pattern| pattern.span);
        for (index, pattern) in patterns.iter().enumerate() {
            let Some(value) = self.value(pattern)? else {
                break;
            };
            let equal = self.pattern_equal(subject, value, pattern.span)?;
            if index + 1 == patterns.len() {
                arms.extend(self.arm(Out::Value(equal)));
                break;
            }
            let hit = self.b.new_block();
            let rest = self.b.new_block();
            self.b.terminate(
                TermKind::Cond {
                    condition: equal.value,
                    then_target: Target::new(hit),
                    else_target: Target::new(rest),
                },
                pattern.span,
            );
            self.b.switch_to(hit);
            let yes = self.constant(Constant::Bool(true), pattern.span);
            arms.extend(self.arm(Out::Value(yes)));
            self.b.switch_to(rest);
        }
        Ok(match self.join(arms, base, true, first_span)? {
            Out::Value(value) => Some(value),
            _ => None,
        })
    }

    /// Переменные, которые меняются в теле `try` или `catch` и видны после него.
    fn tracked_for_try(&mut self, body: &thir::Try, span: Span) -> Vec<usize> {
        let mut locals = Vec::new();
        let mut fields = Vec::new();
        super::assigned_in_block(&body.body, &mut locals, &mut fields);
        for catch in &body.catches {
            super::assigned_in_block(&catch.body, &mut locals, &mut fields);
        }
        let mut tracked: Vec<usize> = Vec::new();
        for local in locals {
            if let Some(var) = self.local_vars[local.0] {
                if self.vars[var].rebindable && !tracked.contains(&var) {
                    tracked.push(var);
                }
            }
        }
        for field in fields {
            if let Some(var) = self.field_vars.get(field).copied() {
                if !tracked.contains(&var) {
                    tracked.push(var);
                }
            }
        }
        tracked.sort_unstable();
        // Staged-поле без значения получает исходное; несвязанное поле не передаётся.
        for var in &tracked {
            if let (None, super::VarKind::Staged(field)) =
                (&self.vars[*var].value, self.vars[*var].kind)
            {
                let original = self.read_field(field, span);
                self.vars[*var].value = Some(original);
            }
        }
        tracked.retain(|var| self.vars[*var].value.is_some());
        // Отслеживаемые `^C` гасятся: обязательства проверил анализ.
        for var in tracked.clone() {
            let value = self.vars[var].value.clone().expect("retained above");
            if matches!(value.ty, Type::Successor(_)) {
                self.take(value, span);
            }
        }
        tracked
    }

    fn catch_type(&mut self, catch: &thir::CatchType, span: Span) -> Result<CatchType> {
        Ok(match catch {
            thir::CatchType::Object(object, args) => {
                let args = args.iter().map(|arg| arg.subst(&self.env)).collect();
                CatchType::Object(self.ctx.object_instance(*object, args, span)?)
            }
            thir::CatchType::Role(role, args) => {
                let args = args.iter().map(|arg| arg.subst(&self.env)).collect();
                CatchType::Role(self.ctx.role_instance(*role, args, span)?)
            }
        })
    }

    /// `try { } catch e is T { } ...` как оператор и как выражение (§19.3).
    pub(super) fn try_body(&mut self, body: &'r thir::Try, want: bool, span: Span) -> Result<Out> {
        let mut catch_types = Vec::new();
        for catch in &body.catches {
            catch_types.push(self.catch_type(&catch.ty, catch.span)?);
        }
        let base = self.vars.len();
        let tracked = self.tracked_for_try(body, span);
        let types: Vec<Type> = tracked
            .iter()
            .map(|var| self.vars[*var].value.as_ref().expect("tracked").ty.clone())
            .collect();
        let entry = self.snapshot();
        let (dispatch, dispatch_params) = self.block_with(&types, &[Type::Failure]);
        self.inherit_taken(&tracked, &dispatch_params);

        // Тело: новый блок, все его блоки уходят в dispatch при отказе.
        let start = self.b.new_block();
        self.b.terminate(TermKind::Br(Target::new(start)), span);
        self.b.switch_to(start);
        self.tries.push(TryCtx {
            target: dispatch,
            tracked: tracked.clone(),
            handled: None,
        });
        self.sync_unwind();
        let body_out = self.branch(&body.body, want);
        self.leave_try();
        let body_out = body_out?;
        let mut arms: Vec<Arm> = self.arm(body_out).into_iter().collect();

        // Выбор catch в порядке исходника; несовпавший отказ идёт дальше.
        self.restore(&entry);
        self.b.switch_to(dispatch);
        self.bind_tracked(&tracked, &types, &dispatch_params);
        let mut failure = *dispatch_params
            .last()
            .expect("dispatch has a failure parameter");
        for (catch, catch_type) in body.catches.iter().zip(catch_types) {
            let caught = catch_type.ty();
            let (handler, handler_params) =
                self.block_with(&types, &[Type::Failure, caught.clone()]);
            let (next, next_params) = self.block_with(&types, &[Type::Failure]);
            self.inherit_taken(&tracked, &handler_params);
            self.inherit_taken(&tracked, &next_params);
            let mut args = self.tracked_args(&tracked);
            args.push(failure);
            self.b.terminate(
                TermKind::Catch {
                    failure,
                    ty: catch_type,
                    handler: Target {
                        block: handler,
                        args: args.clone(),
                    },
                    next: Target { block: next, args },
                },
                catch.span,
            );

            // Тело catch: его отказ получает причину — обрабатываемый отказ.
            let (caused, caused_params) = self.block_with(&types, &[Type::Failure, Type::Failure]);
            self.inherit_taken(&tracked, &caused_params);
            self.restore(&entry);
            self.b.switch_to(handler);
            let count = tracked.len();
            let handled = handler_params[count];
            let error = handler_params[count + 1];
            self.tries.push(TryCtx {
                target: caused,
                tracked: tracked.clone(),
                handled: Some(handled),
            });
            self.bind_tracked(&tracked, &types, &handler_params);
            self.push_scope();
            let error = Typed {
                value: error,
                ty: caught,
            };
            self.bind_local(catch.local, error.clone());
            // Пойманный Error принадлежит телу handler (§7).
            self.pending_adopt.push(error);
            let out = self.branch(&catch.body, want);
            self.pending_adopt.clear();
            self.pop_scope();
            self.leave_try();
            arms.extend(self.arm(out?));

            self.restore(&entry);
            self.b.switch_to(caused);
            self.bind_tracked(&tracked, &types, &caused_params);
            let chained = self.emit(
                InstKind::FailCaused {
                    new: caused_params[count + 1],
                    handled: caused_params[count],
                },
                Type::Failure,
                catch.span,
            );
            self.b.terminate(TermKind::Raise(chained.value), catch.span);

            self.restore(&entry);
            self.b.switch_to(next);
            self.bind_tracked(&tracked, &types, &next_params);
            failure = next_params[count];
        }
        self.b.terminate(TermKind::Raise(failure), span);
        self.restore(&entry);
        self.join(arms, base, want, span)
    }
}
