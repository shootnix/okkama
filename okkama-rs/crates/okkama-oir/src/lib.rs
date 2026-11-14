//! OIR-2 is Okkama's typed SSA representation with explicit ownership.
//!
//! The profiles are [`Profile::High`] (lexical intent), [`Profile::Own`]
//! (explicit ownership), [`Profile::Coro`] (coroutine states), and
//! [`Profile::Low`] (lowered cleanup). References and ownership packets are
//! separate values; `scope<S>` names a module-wide lexical scope.
//!
//! This crate depends only on `std`. It currently supplies the independent
//! data model, operation contracts, builders, editing and text codec for #563,
//! commits 1–4.
//! Builder completion is structural, not semantic verification (#564).
//! Public containers can also be edited directly for invalid verifier fixtures.
//! The v2 text codec preserves structural IR without semantic verification.
//! OIR-1 in `okkama-compiler/src/ir` coexists with OIR-2 until #576; the
//! production lowering and backends still use OIR-1.

mod census;
pub use census::DynamicCensus;
mod ids;
mod module;
pub mod ops;
pub mod contracts;
pub mod cfg;
pub mod builder;
pub mod visit;
pub mod text;
mod types;
mod value;

pub use contracts::*;
pub use builder::*;
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

#[cfg(test)]
mod builder_tests;

#[cfg(test)]
mod text_tests;
