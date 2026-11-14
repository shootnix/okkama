/// Numeric payloads use no compiler/runtime arithmetic types. Callers supply
/// normalized decimal integers and rationals, and finite binary64 float bits.
/// Bits preserve negative zero. Literal validation belongs to the codec/verifier.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Literal {
    Int(String),
    Bool(bool),
    Byte(u8),
    Float(u64),
    Rat { numerator: String, denominator: String },
    Char(char),
    Str(String),
    Enum { decl: DeclId, case: u32 },
}

use crate::*;
use crate::contracts::{BufferAction, Rule, Spec};

// One declaration generates the enum, exhaustive traversal, registry entry and
// test representatives. Adding a variant without semantics is a compile error.
macro_rules! operation_group {
    ($group:ident { $( $variant:ident { $( $field:ident : $ty:ty ),* $(,)? }
        => ($name:literal, [ $( $operand:ident : $mode:ident ),* $(,)? ], $rule:expr); )* }) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum $group { $( $variant { $( $field: $ty ),* }, )* }
        impl crate::visit::Rewrite for $group {
            fn rewrite(&mut self, visitor: &mut impl crate::visit::VisitorMut) {
                match self { $( Self::$variant { $( $field ),* } => {
                    $( $field.rewrite(visitor); )*
                }, )* }
            }
        }
        impl $group {
            pub(crate) fn spec(&self) -> Spec {
                match self { $( Self::$variant { $( $field ),* } => {
                    // Metadata fields are intentionally not SSA uses.
                    $( let _ = $field; )*
                    let operands = Vec::new();
                    $( let operands = append_operand(operands, $operand, OperandMode::$mode); )*
                    Spec { name: $name, operands, rule: $rule }
                }, )* }
            }
            #[cfg(test)]
            pub(crate) fn declared_operands(&self) -> Vec<OperandRef> {
                match self { $( Self::$variant { $( $field ),* } => {
                    #[allow(unused_mut)]
                    let mut refs = Vec::new();
                    $( crate::contract_tests::DeclaredOperands::append_declared($field, &mut refs); )*
                    refs
                }, )* }
            }
            pub const OPCODES: &'static [&'static str] = &[ $( $name ),* ];
            #[cfg(test)]
            pub(crate) fn representatives() -> Vec<Self> {
                vec![ $( Self::$variant { $( $field: crate::contract_tests::sample::<$ty>() ),* } ),* ]
            }
        }
    }
}

pub(crate) trait OperandSource {
    fn append(&self, mode: OperandMode, out: &mut Vec<ReferencedOperand>);
}
fn append_operand<T: OperandSource>(mut out: Vec<ReferencedOperand>, input: &T, mode: OperandMode) -> Vec<ReferencedOperand> {
    input.append(mode, &mut out); out
}
macro_rules! operand_id {
    ($id:ty, $variant:ident) => {
        impl OperandSource for $id {
            fn append(&self, mode: OperandMode, out: &mut Vec<ReferencedOperand>) {
                out.push(ReferencedOperand { reference: OperandRef::$variant(*self), mode });
            }
        }
    }
}
operand_id!(ValueId, Value);
operand_id!(SelectionId, Selection);
operand_id!(DepExprId, Dependency);
operand_id!(KeyExprId, Key);
impl<T: OperandSource> OperandSource for Vec<T> {
    fn append(&self, mode: OperandMode, out: &mut Vec<ReferencedOperand>) {
        for item in self { item.append(mode, out); }
    }
}
impl<T: OperandSource> OperandSource for Option<T> {
    fn append(&self, mode: OperandMode, out: &mut Vec<ReferencedOperand>) {
        if let Some(item) = self { item.append(mode, out); }
    }
}
impl<A: OperandSource, B: OperandSource> OperandSource for (A, B) {
    fn append(&self, mode: OperandMode, out: &mut Vec<ReferencedOperand>) {
        self.0.append(mode, out); self.1.append(mode, out);
    }
}
impl OperandSource for ResourceValue {
    fn append(&self, _: OperandMode, out: &mut Vec<ReferencedOperand>) {
        self.value.append(OperandMode::Borrow, out); self.packet.append(OperandMode::Consume, out);
    }
}

pub mod core;
pub mod lexical;
pub mod ownership;
pub mod async_ops;
pub mod cleanup;
pub use core::*;
pub use lexical::*;
pub use ownership::*;
pub use async_ops::*;
pub use cleanup::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    Const(Literal), Core(CoreOp), Lexical(LexicalOp), Ownership(OwnershipOp),
    Call(CallOp), Collection(CollectionOp), Async(AsyncOp), Carrier(CarrierOp),
    Exit(ExitOp), Cleanup(CleanupOp),
}
impl Operation {
    pub(crate) fn spec(&self) -> Spec {
        match self {
            Self::Const(value) => Spec { name: "const", operands: vec![], rule: Rule::Data {
                ty: match value {
                    Literal::Int(_) => ResultType::Scalar(ScalarType::Int),
                    Literal::Bool(_) => ResultType::Scalar(ScalarType::Bool),
                    Literal::Byte(_) => ResultType::Scalar(ScalarType::Byte),
                    Literal::Float(_) => ResultType::Scalar(ScalarType::Float),
                    Literal::Rat { .. } => ResultType::Scalar(ScalarType::Rat),
                    Literal::Char(_) => ResultType::Scalar(ScalarType::Char),
                    Literal::Str(_) => ResultType::Scalar(ScalarType::Str),
                    Literal::Enum { decl, .. } => ResultType::ExactEnum(*decl),
                }, trap: false,
            } },
            Self::Core(op) => op.spec(), Self::Lexical(op) => op.spec(),
            Self::Ownership(op) => op.spec(), Self::Call(op) => op.spec(),
            Self::Collection(op) => op.spec(), Self::Async(op) => op.spec(),
            Self::Carrier(op) => op.spec(), Self::Exit(op) => op.spec(),
            Self::Cleanup(op) => op.spec(),
        }
    }
    #[cfg(test)]
    pub(crate) fn declared_operands(&self) -> Vec<OperandRef> {
        match self {
            Self::Const(_) => vec![], Self::Core(op) => op.declared_operands(),
            Self::Lexical(op) => op.declared_operands(), Self::Ownership(op) => op.declared_operands(),
            Self::Call(op) => op.declared_operands(), Self::Collection(op) => op.declared_operands(),
            Self::Async(op) => op.declared_operands(), Self::Carrier(op) => op.declared_operands(),
            Self::Exit(op) => op.declared_operands(), Self::Cleanup(op) => op.declared_operands(),
        }
    }
    pub fn opcode(&self) -> &'static str { self.spec().name }
    pub fn operand_refs(&self) -> Vec<ReferencedOperand> { self.spec().operands }
    pub fn opcode_inventory() -> Vec<&'static str> {
        [vec!["const"], CoreOp::OPCODES.to_vec(), LexicalOp::OPCODES.to_vec(),
            OwnershipOp::OPCODES.to_vec(), CallOp::OPCODES.to_vec(), CollectionOp::OPCODES.to_vec(),
            AsyncOp::OPCODES.to_vec(), CarrierOp::OPCODES.to_vec(), ExitOp::OPCODES.to_vec(),
            CleanupOp::OPCODES.to_vec()].concat()
    }
}
