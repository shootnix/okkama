//! Constant folding with exact arithmetic (H-36).
//!
//! `int.add`, `int.sub`, `int.mul`, `int.mod` with a nonzero divisor,
//! `int.neg` and the six integer comparisons of constant operands become
//! constants, computed with the language's own `IntValue`: a result beyond
//! 64 bits is the exact big integer the engines would promote to. Nothing
//! that can fail is folded (`int.mod` by zero stays and fails at run time),
//! nor the word-level bitwise operations, whose 64-bit semantics each engine
//! implements itself; a result longer than [`MAX_DIGITS`] digits stays a
//! computation, so folding cannot grow the program without bound.
//!
//! Values are SSA: a constant operand is a `const` anywhere in the function.
//! A folded result is a constant for the rest of the pass, so a chain folds
//! in one pass; the pass repeats only while it still folds something.
use super::{CompareOp, Constant, Function, InstKind, IntBinaryOp, IntUnaryOp, IntValue, Module, ValueId};
use okkama_runtime_core::integer;
use std::collections::HashMap;

/// Longest folded integer, in decimal digits.
pub const MAX_DIGITS: usize = 1024;

/// Fold the constant arithmetic of every function; the number folded.
pub fn fold_constants(module: &mut Module) -> usize {
    module.functions.iter_mut().map(fold).sum()
}

fn fold(function: &mut Function) -> usize {
    let mut folded = 0;
    // A folded result joins the constants at once, so a chain in block
    // order folds in one pass; another pass only for uses that precede
    // their definition in block order.
    let mut constants: HashMap<ValueId, Constant> = HashMap::new();
    for block in &function.blocks {
        for inst in &block.insts {
            if let (InstKind::Const(constant), Some(result)) = (&inst.kind, inst.result) {
                constants.insert(result, constant.clone());
            }
        }
    }
    loop {
        let mut changed = 0;
        for block in &mut function.blocks {
            for inst in &mut block.insts {
                let int = |value: &ValueId| match constants.get(value) {
                    Some(Constant::Int(n)) => Some(n.clone()),
                    _ => None,
                };
                let value = match &inst.kind {
                    InstKind::IntBinary { op, left, right } => match (int(left), int(right)) {
                        (Some(a), Some(b)) => binary(*op, &a, &b).map(Constant::Int),
                        _ => None,
                    },
                    InstKind::IntUnary { op: IntUnaryOp::Neg, operand } => int(operand).map(|n| Constant::Int(integer::negate(&n))),
                    InstKind::IntCompare { op, left, right } => match (int(left), int(right)) {
                        (Some(a), Some(b)) => Some(Constant::Bool(compare(*op, integer::compare(&a, &b)))),
                        _ => None,
                    },
                    _ => None,
                };
                let Some(value) = value else { continue };
                let small = match &value {
                    Constant::Int(n) => n.to_string().len() <= MAX_DIGITS,
                    _ => true,
                };
                if small {
                    if let Some(result) = inst.result {
                        constants.insert(result, value.clone());
                    }
                    inst.kind = InstKind::Const(value);
                    changed += 1;
                }
            }
        }
        if changed == 0 {
            return folded;
        }
        folded += changed;
    }
}

/// The exact result, or `None` for what may fail or is not folded.
fn binary(op: IntBinaryOp, a: &IntValue, b: &IntValue) -> Option<IntValue> {
    match op {
        IntBinaryOp::Add => Some(integer::add(a, b)),
        IntBinaryOp::Sub => Some(integer::subtract(a, b)),
        IntBinaryOp::Mul => Some(integer::multiply(a, b)),
        IntBinaryOp::Mod => integer::remainder(a, b),
        IntBinaryOp::And
        | IntBinaryOp::Or
        | IntBinaryOp::Xor
        | IntBinaryOp::Shl
        | IntBinaryOp::Shr
        | IntBinaryOp::Ushr => None,
    }
}

fn compare(op: CompareOp, order: std::cmp::Ordering) -> bool {
    use std::cmp::Ordering::*;
    match op {
        CompareOp::Eq => order == Equal,
        CompareOp::Ne => order != Equal,
        CompareOp::Lt => order == Less,
        CompareOp::Le => order != Greater,
        CompareOp::Gt => order == Greater,
        CompareOp::Ge => order != Less,
    }
}
