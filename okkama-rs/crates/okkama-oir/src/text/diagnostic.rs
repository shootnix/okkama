use std::fmt;
use crate::Module;

/// Half-open byte offsets into the OIR input, never an Okkama DebugLoc.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span { pub start: usize, pub end: usize }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticKind {
    Header, UnsupportedVersion, MissingProfile, UnknownProfile,
    Lexical, Syntax, DuplicateName, UnknownName, Limit, Literal, Structure, OutcomeSlot,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic { pub kind: DiagnosticKind, pub span: Span, pub message: String }
impl Diagnostic {
    pub(crate) fn new(kind: DiagnosticKind, span: Span, message: impl Into<String>) -> Self {
        Self { kind, span, message: message.into() }
    }
}
impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} at {}..{}: {}", self.kind, self.span.start, self.span.end, self.message)
    }
}
impl std::error::Error for Diagnostic {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrintError { pub message: String }
impl PrintError { pub(crate) fn new(message: impl Into<String>) -> Self { Self { message: message.into() } } }
impl fmt::Display for PrintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.message) }
}
impl std::error::Error for PrintError {}

/// A deliberately non-canonical dump for verifier reports, including broken IDs.
/// It cannot be mistaken for a versioned OIR module.
pub fn print_diagnostic(module: &Module) -> String {
    format!("NON-CANONICAL OIR DIAGNOSTIC DUMP\n{module:#?}\n")
}
