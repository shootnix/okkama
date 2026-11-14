//! OIR-2 is Okkama's typed SSA representation with explicit ownership.
//!
//! The profiles are [`Profile::High`] (lexical intent), [`Profile::Own`]
//! (explicit ownership), [`Profile::Coro`] (coroutine states), and
//! [`Profile::Low`] (lowered cleanup). References and ownership packets are
//! separate values; `scope<S>` names a module-wide lexical scope.
//!
//! This crate depends only on `std`. It currently supplies the independent
//! data model and operation contracts for #563, commits 1–2. Containers can
//! be constructed by hand and do not imply verification. Builders, editing
//! and text codecs belong to subsequent commits.
//! OIR-1 in `okkama-compiler/src/ir` coexists with OIR-2 until #576; the
//! production lowering and backends still use OIR-1.

mod ids;
mod module;
pub mod ops;
pub mod contracts;
pub mod cfg;
mod types;
mod value;

pub use contracts::*;
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

#[cfg(test)]
mod contract_tests;
