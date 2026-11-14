//! Developer-only THIR -> OIR-2 high producer for #566 S1.
//!
//! This module is compiled only for library tests. It never enters the OIR-1
//! pipeline. Every declared source function must have a lowered body.
//! Unsupported construction and cleanup operations belong to S2/#567 and
//! produce source diagnostics. Result exits are centralized in `BodyLowerer`
//! so D11 can replace the pre-D11 instruction-local encoding in one place.

mod body;
mod declarations;
mod scalar;
mod structure;
#[cfg(test)]
mod tests;

use crate::analysis::{Analysis, thir};
use crate::oir2 as ir;
use crate::source::Span;

#[derive(Debug)]
pub(crate) enum LowerError {
    Unsupported { span: Span, message: String },
    Build(ir::BuildError),
    InvalidThir { span: Span, message: String },
}

impl LowerError {
    fn unsupported(span: Span, construct: impl std::fmt::Display) -> Self {
        Self::Unsupported {
            span,
            message: format!("unsupported in OIR-2 lowering (S1): {construct}"),
        }
    }

    fn invalid(span: Span, message: impl Into<String>) -> Self {
        Self::InvalidThir { span, message: message.into() }
    }
}

impl From<ir::BuildError> for LowerError {
    fn from(error: ir::BuildError) -> Self {
        Self::Build(error)
    }
}

impl From<ir::ArenaError> for LowerError {
    fn from(error: ir::ArenaError) -> Self {
        Self::Build(ir::BuildError::Arena(error))
    }
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported { span, message } | Self::InvalidThir { span, message } => {
                write!(f, "{span}: {message}")
            }
            Self::Build(error) => write!(f, "OIR-2 construction: {error}"),
        }
    }
}

impl std::error::Error for LowerError {}

type Result<T> = std::result::Result<T, LowerError>;

/// Independent developer entry: analysis supplies resolved THIR and declarations.
/// No CLI routing, ownership elaboration or semantic verifier is invoked here.
pub(crate) fn lower_to_oir2(analysis: &Analysis<'_>) -> Result<ir::Module> {
    let mut declarations = declarations::Declarations::new(analysis)?;
    // Check ordinary/default/cleanup bodies before unsupported construction.
    let mut bodies: Vec<_> = analysis.bodies.iter().collect();
    bodies.sort_by_key(|body| matches!(body.owner, thir::BodyOwner::Ctor { .. }));
    for body in bodies {
        let function = declarations.functions.get(&body.owner).copied().ok_or_else(|| {
            LowerError::unsupported(body.span, format!("body {:?}", body.owner))
        })?;
        let symbol = declarations.builder.module().functions.get(function)?.symbol.clone();
        if matches!(body.owner, thir::BodyOwner::Ctor { .. }) {
            return Err(LowerError::unsupported(body.span, format!("function {symbol}: constructor body (S2)")));
        }
        let root = declarations.builder.declare_scope(ir::ScopeDecl {
            function,
            parent: None,
            kind: ir::ScopeKind::Action,
            loc: None,
        })?;
        let builder = declarations.builder.function_builder(function, root)?;
        let lowered = body::BodyLowerer::lower(builder, body, root, &declarations.types).map_err(|error| {
            match error {
                LowerError::Unsupported { span, message } => LowerError::Unsupported {
                    span,
                    message: format!("{message} in function {symbol}"),
                },
                error => error,
            }
        })?;
        declarations.builder.define_function(function, lowered)?;
    }
    let mut module = declarations.builder.finish()?;
    structure::finish(&mut module)?;
    structure::check(&module)?;
    Ok(module)
}
