//! Builtins built from existing instructions (#210, #211, #231): searches and
//! shapes of arrays are loops over `array.get`, so every engine runs them
//! unchanged; string searches come from the runtime as an index or -1 and
//! become a `Maybe<int>` here.

use super::matching::compare_inst;
use super::{BodyLowerer, Out, Typed};
use crate::analysis::thir::{self, CompareKind};
use crate::ir::{
    CompareOp, Constant, InstKind, IntBinaryOp, IntValue, Target, TermKind, TrapCode, Type, ValueId,
    VariantId,
};
use crate::lowering::{LoweringError, Result};
use crate::source::Span;

impl BodyLowerer<'_, '_, '_> {
    fn int(&mut self, value: i64, span: Span) -> ValueId {
        self.constant(Constant::Int(IntValue::from_i64(value)), span).value
    }

    /// The case and its field for a payload field numbered across all cases
    /// as the analysis numbers them (from 1: 0 was the tag).
    pub(super) fn case_field(&self, id: VariantId, field: usize) -> (usize, usize) {
        let mut offset = 1;
        for (case, (_, object)) in self.ctx.variants[id.0].cases.iter().enumerate() {
            let count = self.ctx.objects[object.0].fields.len();
            if field < offset + count {
                return (case, field - offset);
            }
            offset += count;
        }
        unreachable!("a payload field of the variant")
    }

    /// The `Maybe<int>` variant of `expr`.
    fn maybe_type(&mut self, expr: &thir::Expr, span: Span) -> Result<Type> {
        match self.expr_type(expr)? {
            ty @ Type::Variant(_) => Ok(ty),
            _ => Err(LoweringError::invalid(
                span,
                "internal compiler error: a search builtin does not produce Maybe<int>",
            )),
        }
    }

    /// `Maybe.some(index)`, or `Maybe.none` for a negative index.
    pub(super) fn maybe_index(&mut self, index: ValueId, expr: &thir::Expr, span: Span) -> Result<Out> {
        let maybe = self.maybe_type(expr, span)?;
        let zero = self.int(0, span);
        let found = self
            .emit(InstKind::IntCompare { op: CompareOp::Ge, left: index, right: zero }, Type::Bool, span)
            .value;
        let (join, params) = self.block_with(&[], &[maybe.clone()]);
        let (some, none) = (self.b.new_block(), self.b.new_block());
        self.b.terminate(
            TermKind::Cond { condition: found, then_target: Target::new(some), else_target: Target::new(none) },
            span,
        );
        for (block, case) in [(some, 0), (none, 1)] {
            self.b.switch_to(block);
            self.sync_unwind();
            let fields = if case == 0 { vec![index] } else { Vec::new() };
            let value = self.emit(InstKind::VariantNew { case, fields }, maybe.clone(), span).value;
            self.b.terminate(TermKind::Br(Target { block: join, args: vec![value] }), span);
        }
        self.b.switch_to(join);
        self.sync_unwind();
        Ok(Out::Value(Typed { value: params[0], ty: maybe }))
    }

    /// The index of the first element equal to `needle` (`==` of `kind`),
    /// or -1: `contains` and `index_of` (#211).
    pub(super) fn array_search(
        &mut self,
        array: &Typed,
        needle: ValueId,
        kind: CompareKind,
        span: Span,
    ) -> ValueId {
        let missing = self.int(-1, span);
        let Type::Array(element) = &array.ty else {
            return missing;
        };
        // An untyped empty array has no elements to compare.
        if **element == Type::Never {
            return missing;
        }
        let element = (**element).clone();
        let length = self.emit(InstKind::ArrayLen(array.value), Type::Int, span).value;
        let one = self.int(1, span);
        let start = self.int(0, span);
        let (head, head_params) = self.block_with(&[], &[Type::Int]);
        let (done, done_params) = self.block_with(&[], &[Type::Int]);
        self.b.terminate(TermKind::Br(Target { block: head, args: vec![start] }), span);
        self.b.switch_to(head);
        self.sync_unwind();
        let index = head_params[0];
        let more = self
            .emit(InstKind::IntCompare { op: CompareOp::Lt, left: index, right: length }, Type::Bool, span)
            .value;
        let body = self.b.new_block();
        self.b.terminate(
            TermKind::Cond {
                condition: more,
                then_target: Target::new(body),
                else_target: Target { block: done, args: vec![missing] },
            },
            span,
        );
        self.b.switch_to(body);
        self.sync_unwind();
        let item = self.emit(InstKind::ArrayGet { array: array.value, index }, element, span).value;
        let equal = self
            .emit(compare_inst(kind, CompareOp::Eq, item, needle), Type::Bool, span)
            .value;
        let next = self.b.new_block();
        self.b.terminate(
            TermKind::Cond {
                condition: equal,
                then_target: Target { block: done, args: vec![index] },
                else_target: Target::new(next),
            },
            span,
        );
        self.b.switch_to(next);
        self.sync_unwind();
        let following = self
            .emit(InstKind::IntBinary { op: IntBinaryOp::Add, left: index, right: one }, Type::Int, span)
            .value;
        self.b.terminate(TermKind::Br(Target { block: head, args: vec![following] }), span);
        self.b.switch_to(done);
        self.sync_unwind();
        done_params[0]
    }

    /// The elements from the last to the first (#211).
    pub(super) fn array_reversed(&mut self, array: &Typed, span: Span) -> Typed {
        let Type::Array(element) = &array.ty else {
            return array.clone();
        };
        if **element == Type::Never {
            return array.clone();
        }
        let element = (**element).clone();
        let length = self.emit(InstKind::ArrayLen(array.value), Type::Int, span).value;
        let (zero, one) = (self.int(0, span), self.int(1, span));
        let builder_ty = Type::Builder(Box::new(element.clone()));
        let builder = self.emit(InstKind::BuilderNew, builder_ty.clone(), span).value;
        // head(remaining, builder): while remaining > 0, push item remaining - 1.
        // The builder is linear: the exit uses the head's own parameter.
        let (head, head_params) = self.block_with(&[], &[Type::Int, builder_ty.clone()]);
        let done = self.b.new_block();
        self.b.terminate(TermKind::Br(Target { block: head, args: vec![length, builder] }), span);
        self.b.switch_to(head);
        self.sync_unwind();
        let (remaining, partial) = (head_params[0], head_params[1]);
        let more = self
            .emit(InstKind::IntCompare { op: CompareOp::Gt, left: remaining, right: zero }, Type::Bool, span)
            .value;
        let body = self.b.new_block();
        self.b.terminate(
            TermKind::Cond {
                condition: more,
                then_target: Target::new(body),
                else_target: Target::new(done),
            },
            span,
        );
        self.b.switch_to(body);
        self.sync_unwind();
        let index = self
            .emit(InstKind::IntBinary { op: IntBinaryOp::Sub, left: remaining, right: one }, Type::Int, span)
            .value;
        let item = self.emit(InstKind::ArrayGet { array: array.value, index }, element, span).value;
        let pushed = self.emit(InstKind::BuilderPush { builder: partial, value: item }, builder_ty, span).value;
        self.b.terminate(TermKind::Br(Target { block: head, args: vec![index, pushed] }), span);
        self.b.switch_to(done);
        self.sync_unwind();
        self.emit(InstKind::BuilderFinish(partial), array.ty.clone(), span)
    }

    /// `take(count)` (`drop` when `drop`): at most `count` elements from the
    /// start, or all but them; a negative count is an error (#211).
    pub(super) fn array_take(&mut self, array: &Typed, count: ValueId, drop: bool, span: Span) -> Typed {
        let zero = self.int(0, span);
        let negative = self
            .emit(InstKind::IntCompare { op: CompareOp::Lt, left: count, right: zero }, Type::Bool, span)
            .value;
        let (fail, check) = (self.b.new_block(), self.b.new_block());
        self.b.terminate(
            TermKind::Cond { condition: negative, then_target: Target::new(fail), else_target: Target::new(check) },
            span,
        );
        self.b.switch_to(fail);
        self.sync_unwind();
        let name = if drop { "drop" } else { "take" };
        self.b.terminate(
            TermKind::Trap { code: TrapCode::Range, message: format!("{name} needs a non-negative count") },
            span,
        );
        self.b.switch_to(check);
        self.sync_unwind();
        // The cut: min(count, size).
        let length = self.emit(InstKind::ArrayLen(array.value), Type::Int, span).value;
        let short = self
            .emit(InstKind::IntCompare { op: CompareOp::Lt, left: count, right: length }, Type::Bool, span)
            .value;
        let (cut, cut_params) = self.block_with(&[], &[Type::Int]);
        let (within, all) = (self.b.new_block(), self.b.new_block());
        self.b.terminate(
            TermKind::Cond { condition: short, then_target: Target::new(within), else_target: Target::new(all) },
            span,
        );
        for (block, value) in [(within, count), (all, length)] {
            self.b.switch_to(block);
            self.sync_unwind();
            self.b.terminate(TermKind::Br(Target { block: cut, args: vec![value] }), span);
        }
        self.b.switch_to(cut);
        self.sync_unwind();
        let (start, end) = if drop { (cut_params[0], length) } else { (zero, cut_params[0]) };
        self.emit(InstKind::ArraySlice { array: array.value, start, end }, array.ty.clone(), span)
    }
}
