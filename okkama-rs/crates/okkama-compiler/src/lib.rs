//! Okkama compiler library.
//!
//! The crate currently implements the syntactic front end and initial IR:
//!
//! * [`source`] — source files, byte spans and line/column mapping;
//! * [`diagnostics`] — structured lexical and syntax diagnostics;
//! * [`syntax`] — tokens, lexer, typed AST, parser, production pruning and
//!   AST printers (`tree` and `json`);
//! * [`ir`] — OIR-1: typed SSA modules, objects, functions and blocks, the
//!   canonical text form and the verifier (`docs/compiler/OIR-1.md`);
//! * [`lowering`] — typed AST-to-OIR lowering of the supported subset:
//!   scalars, control flow, loops, objects, constructors, actions, successors;
//! * [`codegen`] — backend selection and direct JVM class-file generation.
//!
//! The later pipeline stages are present only as documented module
//! boundaries: [`analysis`] and [`jit`]. General analysis and a native-code
//! backend are not implemented yet. JVM-generated programs run on a JVM.
//!
//! The library never reads command-line arguments, never prints and never
//! terminates the process. Callers receive values and diagnostics and decide
//! how to present them (see the `okkama-cli` crate).

pub mod analysis;
pub mod codegen;
pub mod diagnostics;
pub mod ide;
pub mod ir;
pub mod jit;
pub mod lowering;
pub mod source;
pub mod syntax;

pub use diagnostics::{Diagnostic, DiagnosticKind};
pub use source::{LineColumn, SourceFile, Span};
