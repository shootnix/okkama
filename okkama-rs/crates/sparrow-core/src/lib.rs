//! Sparrow execution core (rebuild, see `docs/new-runtime/core-rebuild.md`).
//!
//! Model, validated by `crates/sparrow-proto` (`docs/new-runtime/core-spike.md`):
//! - synchronous functions are compiled whole and call each other directly on
//!   the native stack; roles dispatch through per-descriptor tables;
//! - async functions are resumable state machines whose heap frame is also
//!   their Future; `spawn` starts eagerly, Tokio drives external waits;
//! - registers carry no tags where the SPB type is static; objects are pointers
//!   to `[header, fields…]` with inline allocation;
//! - a precise collector finds roots through Cranelift stack maps, async
//!   frames and Rust-side visitors, and runs only at safepoints, never inside a
//!   native provider; it does not move objects (decision R1);
//! - no preemption inside synchronous code: loop back-edges poll an interrupt
//!   flag so cancellation and timeouts stop runaway loops with a typed failure.
//!
//! Stages: 1 heap and values, 2 synchronous execution, 3 async, 4 natives and
//! CLI (`sparrow-next` switches here), 5 performance and std/db, 6 removal of
//! `sparrow-engine`.

/// Engine-level failure: an invalid program or an internal invariant, never a
/// language-level error (those are values unwound by the program).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}
