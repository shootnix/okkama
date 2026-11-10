//! Prototype persistent arrays for release 0.4.3 (#246, phase 1).
//!
//! Okkama arrays are immutable values; today every operation that yields a
//! new array copies the whole origin. This crate holds the candidate
//! representations, isolated from the engines, so that their cost can be
//! measured before any engine changes:
//!
//! - [`Trie`]: a 32-way trie with a tail buffer (Bagwell; Clojure's
//!   `PersistentVector`). `push` and `set` copy one path; reads walk
//!   ⌈log₃₂ n⌉ levels.
//! - [`Frontier`]: one flat buffer shared by the versions of an append
//!   chain. Each version knows its own length; `push` writes in place when
//!   the version ends at the buffer's frontier, and copies otherwise. Reads
//!   stay flat (one more indirection than a plain array).
//!
//! The flat copy of today is a `Rc<[T]>` cloned into a new slice per
//! operation. `examples/pbench.rs` measures all three.

pub mod frontier;
pub mod pmap;
pub mod trie;

pub use frontier::Frontier;
pub use pmap::PMap;
pub use trie::Trie;
