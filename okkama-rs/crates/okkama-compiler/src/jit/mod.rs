//! Just-in-time compilation is not part of the compiler.
//!
//! Machine code is produced at run time by the Sparrow VM
//! (`crates/sparrow-vm/src/jit.rs`, Cranelift) from verified SPB bytecode:
//! hot functions of a running program are compiled there, so the compiler's
//! output — OIR, JVM class files and SPB — is the same with or without JIT.
//!
//! This module stays empty and must not become a dependency of any earlier
//! stage.
