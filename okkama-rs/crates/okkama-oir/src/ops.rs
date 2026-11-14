use crate::DeclId;

/// Minimal operation skeleton for constructing bodies by hand.
/// The full opcode inventory and its contracts are commit 2, not defined here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    Const(Literal),
}

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
