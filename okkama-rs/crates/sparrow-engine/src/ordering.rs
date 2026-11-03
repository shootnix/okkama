//! Stable bulk ordering. Scalar comparisons stay in Rust; object comparisons
//! use ordinary VM calls, so throws, GC and suspension budgets retain their ABI.
use crate::{Error, FunctionPlan, Op, Program, Type, array::ArrayOp, plan::Plan, value::*};
use okkama_sparrow::Cmp;
pub(crate) fn lower(program: &Program, index: usize) -> Result<FunctionPlan, Error> {
    let f = &program.functions[index];
    let meta = f.intrinsic.as_ref().expect("ordering intrinsic");
    let invalid = || Error("invalid array ordering signature".into());
    if f.is_async || f.params.len() != 1 || !meta.helpers.is_empty() || !meta.objects.is_empty() {
        return Err(invalid());
    }
    let source = f.params[0].0 as usize;
    let input = f.registers[source];
    if f.result != Some(input) {
        return Err(invalid());
    }
    let Some(Type::Array(element)) = program.ty(input) else {
        return Err(invalid());
    };
    let builtin = matches!(
        program.ty(*element),
        Some(Type::Int | Type::Float | Type::Rat | Type::Byte | Type::Char | Type::Str)
    );
    let n = f.registers.len();
    let (
        src,
        len,
        width,
        left,
        mid,
        end,
        i,
        j,
        builder,
        cond,
        a,
        b,
        order,
        greater,
        one,
        zero,
        scratch,
    ) = (
        n,
        n + 1,
        n + 2,
        n + 3,
        n + 4,
        n + 5,
        n + 6,
        n + 7,
        n + 8,
        n + 9,
        n + 10,
        n + 11,
        n + 12,
        n + 13,
        n + 14,
        n + 15,
        n + 16,
    );
    let mut p = Plan::default();
    if builtin {
        p.push(Op::Intrinsic(
            crate::intrinsic::Operation::ScalarOrdering,
            src,
            [Some(source), None, None],
        ));
        p.push(Op::Return(src));
    } else {
        let (role, slot, enum_id) = program
            .roles
            .iter()
            .enumerate()
            .find_map(|(r, decl)| {
                if !decl.name.starts_with("compare::Comparing<") {
                    return None;
                }
                let (s, method) = decl
                    .methods
                    .iter()
                    .enumerate()
                    .find(|(_, m)| m.name == "order" && m.params == [*element])?;
                let Type::Enum(e) = program.ty(method.result?)? else {
                    return None;
                };
                let en = program.enums.get(e.0 as usize)?;
                if en.name != "compare::Order" || en.cases != ["less", "equal", "greater"] {
                    return None;
                }
                let accepts = match program.ty(*element)? {
                    Type::Object(o) => program.objects[o.0 as usize]
                        .does
                        .iter()
                        .any(|w| w.role.0 as usize == r),
                    Type::Role(expected) => expected.0 as usize == r,
                    _ => false,
                };
                accepts.then_some((r as u32, s, e.0))
            })
            .ok_or_else(invalid)?;
        let array = |op, dst, args| Op::Array(op, dst, args);
        let arith = |dst, x, y| Op::Arithmetic(Arithmetic::Add, dst, x, y);
        p.push(Op::Const(zero, Slot::small(0)));
        p.push(Op::Const(one, Slot::small(1)));
        p.push(Op::Const(
            greater,
            Slot {
                tag: ENUM,
                bits: ((enum_id as u64) << 32) | 2,
            },
        ));
        p.push(array(ArrayOp::Length, len, [Some(source), None, None]));
        p.push(array(
            ArrayOp::Slice,
            src,
            [Some(source), Some(zero), Some(len)],
        ));
        p.push(Op::Copy(width, one));
        p.label("pass");
        p.push(Op::Compare(Cmp::Lt, cond, width, len));
        p.branch(cond, "begin_pass", "done");
        p.label("begin_pass");
        p.push(array(ArrayOp::BuilderNew, builder, [None; 3]));
        p.push(Op::Copy(left, zero));
        p.label("block");
        p.push(Op::Compare(Cmp::Lt, cond, left, len));
        p.branch(cond, "bounds", "finish_pass");
        p.label("bounds");
        p.push(arith(mid, left, width));
        p.push(Op::Compare(Cmp::Gt, cond, mid, len));
        p.branch(cond, "clip_mid", "end_bound");
        p.label("clip_mid");
        p.push(Op::Copy(mid, len));
        p.label("end_bound");
        p.push(arith(end, mid, width));
        p.push(Op::Compare(Cmp::Gt, cond, end, len));
        p.branch(cond, "clip_end", "indices");
        p.label("clip_end");
        p.push(Op::Copy(end, len));
        p.label("indices");
        p.push(Op::Copy(i, left));
        p.push(Op::Copy(j, mid));
        p.label("merge");
        p.push(Op::Compare(Cmp::Lt, cond, i, mid));
        p.branch(cond, "check_right", "right_rest");
        p.label("check_right");
        p.push(Op::Compare(Cmp::Lt, cond, j, end));
        p.branch(cond, "compare", "left_rest");
        p.label("compare");
        p.push(array(ArrayOp::Get, a, [Some(src), Some(i), None]));
        p.push(array(ArrayOp::Get, b, [Some(src), Some(j), None]));
        p.push(Op::CallRole {
            dst: Some(order),
            receiver: a,
            role,
            slot,
            args: vec![b],
            tail: false,
        });
        p.push(Op::Equal(Cmp::Eq, cond, order, greater));
        p.branch(cond, "take_right", "take_left");
        p.label("take_left");
        p.push(array(
            ArrayOp::Push,
            scratch,
            [Some(builder), Some(a), None],
        ));
        p.push(arith(i, i, one));
        p.jump("merge");
        p.label("take_right");
        p.push(array(
            ArrayOp::Push,
            scratch,
            [Some(builder), Some(b), None],
        ));
        p.push(arith(j, j, one));
        p.jump("merge");
        p.label("left_rest");
        p.push(Op::Compare(Cmp::Lt, cond, i, mid));
        p.branch(cond, "left_append", "next_block");
        p.label("left_append");
        p.push(array(ArrayOp::Get, a, [Some(src), Some(i), None]));
        p.push(array(
            ArrayOp::Push,
            scratch,
            [Some(builder), Some(a), None],
        ));
        p.push(arith(i, i, one));
        p.jump("left_rest");
        p.label("right_rest");
        p.push(Op::Compare(Cmp::Lt, cond, j, end));
        p.branch(cond, "right_append", "next_block");
        p.label("right_append");
        p.push(array(ArrayOp::Get, b, [Some(src), Some(j), None]));
        p.push(array(
            ArrayOp::Push,
            scratch,
            [Some(builder), Some(b), None],
        ));
        p.push(arith(j, j, one));
        p.jump("right_rest");
        p.label("next_block");
        p.push(Op::Copy(left, end));
        p.jump("block");
        p.label("finish_pass");
        p.push(array(ArrayOp::Finish, src, [Some(builder), None, None]));
        p.push(arith(width, width, width));
        p.jump("pass");
        p.label("done");
        p.push(Op::Return(src));
    }
    let plan = p.finish();
    let live = crate::liveness::analyze(&plan, &[]);
    let call_saved = crate::liveness::call_saved(&plan, &live, &[]);
    Ok(FunctionPlan {
        native: None,
        is_async: false,
        cold_entry: 0,
        cached: false,
        symbol: f.symbol.clone(),
        recurrence_loop: false,
        elided_objects: 0,
        params: vec![source],
        registers: n + 17,
        plan,
        compiled: Default::default(),
        jit_kinds: None,
        live,
        call_saved,
        handlers: vec![],
    })
}
impl Heap {
    pub(crate) fn scalar_ordering(&mut self, source: Slot) -> Result<Slot, Error> {
        let mut values = self.elements(source)?.to_vec();
        let mut failure = None;
        values.sort_by(|a, b| {
            if failure.is_some() {
                return std::cmp::Ordering::Equal;
            }
            if a.tag == FLOAT && b.tag == FLOAT {
                return f64::from_bits(a.bits).total_cmp(&f64::from_bits(b.bits));
            }
            self.compare(*a, *b).unwrap_or_else(|e| {
                failure = Some(e);
                std::cmp::Ordering::Equal
            })
        });
        if let Some(error) = failure {
            return Err(error);
        }
        self.allocate_array(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn float_ordering_distinguishes_negative_zero_like_the_origin() {
        let mut heap = Heap::new(&[], &[]);
        let floats = [0.0f64, -0.0, 1.0, -1.0].map(|n| Slot {
            tag: FLOAT,
            bits: n.to_bits(),
        });
        let original = heap.allocate_array(floats.to_vec()).unwrap();
        let sorted = heap.scalar_ordering(original).unwrap();
        let bits: Vec<_> = heap
            .elements(sorted)
            .unwrap()
            .iter()
            .map(|s| s.bits)
            .collect();
        assert_eq!(bits, [-1.0f64, -0.0, 0.0, 1.0].map(f64::to_bits));
        assert_eq!(heap.elements(original).unwrap()[0].bits, 0.0f64.to_bits());
    }
}
