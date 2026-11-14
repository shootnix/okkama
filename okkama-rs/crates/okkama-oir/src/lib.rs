//! OIR-2 is Okkama's typed SSA representation with explicit ownership.
//!
//! The profiles are [`Profile::High`] (lexical intent), [`Profile::Own`]
//! (explicit ownership), [`Profile::Coro`] (coroutine states), and
//! [`Profile::Low`] (lowered cleanup). References and ownership packets are
//! separate values; `scope<S>` names a module-wide lexical scope.
//!
//! This crate depends only on `std`. It currently supplies the independent
//! data model for #563, commit 1: containers can be constructed by hand and
//! do not imply verification. Operation contracts, ownership facts/summaries,
//! builders, editing and text codecs belong to subsequent commits.
//! OIR-1 in `okkama-compiler/src/ir` coexists with OIR-2 until #576; the
//! production lowering and backends still use OIR-1.

mod ids;
mod module;
mod ops;
mod types;
mod value;

pub use ids::*;
pub use module::*;
pub use ops::*;
pub use types::*;
pub use value::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Profile {
    High,
    Own,
    Coro,
    Low,
}

#[cfg(test)]
mod tests;
