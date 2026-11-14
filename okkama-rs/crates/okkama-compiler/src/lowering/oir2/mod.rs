//! Developer-only THIR -> OIR-2 high producer for #566 and #567 S1/S2.
//!
//! This module is compiled only for library tests. It never enters the OIR-1
//! pipeline. Every declared source function must have a lowered body.
//! Unsupported operations and destroy bodies belong to later slices and
//! produce source diagnostics. Result exits are centralized in `BodyLowerer`
//! so D11 can replace the pre-D11 instruction-local encoding in one place.

mod body;
mod construction;
mod declarations;
mod defer;
mod high;
mod objects;
mod walk;
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
            message: format!("unsupported in OIR-2 lowering: {construct}"),
        }
    }

    fn in_function(self, symbol: &str) -> Self {
        match self {
            Self::Unsupported { span, message } => Self::Unsupported {
                span, message: format!("{message} in function {symbol}"),
            },
            error => error,
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
    // Check ordinary/default/cleanup bodies before constructor specializations.
    let mut bodies: Vec<_> = analysis.bodies.iter().collect();
    bodies.sort_by_key(|body| matches!(body.owner, thir::BodyOwner::Ctor { .. }));
    for body in bodies {
        let function = declarations.functions.get(&body.owner).copied().ok_or_else(|| {
            LowerError::unsupported(body.span, format!("body {:?}", body.owner))
        })?;
        let symbol = declarations.builder.module().functions.get(function)?.symbol.clone();
        if matches!(body.owner, thir::BodyOwner::Destroy { .. }) {
            return Err(LowerError::unsupported(body.span, format!("function {symbol}: destroy body (#567)")));
        }
        let mut variants = vec![function];
        if let thir::BodyOwner::Ctor { object, ctor } = body.owner {
            variants = declarations.constructors.iter().filter_map(|((o, c, _), &id)| {
                (*o == object && *c == ctor).then_some(id)
            }).collect();
            variants.sort();
        }
        for function in variants {
            let symbol = declarations.builder.module().functions.get(function)?.symbol.clone();
            let root = declarations.builder.declare_scope(ir::ScopeDecl {
                function,
                parent: None,
                kind: ir::ScopeKind::Action,
                loc: None,
            })?;
            let builder = declarations.builder.function_builder(function, root)?;
            let listed = match body.owner {
                thir::BodyOwner::Ctor { object, ctor } => {
                    match &analysis.registry.objects[object.0].ctors[ctor].shape {
                        crate::analysis::registry::CtorShape::Shorthand { fields, .. } => Some(fields.clone()),
                        _ => None,
                    }
                }
                _ => None,
            };
            let context = body::BodyContext {
                functions: &declarations.functions,
                objects: &declarations.objects,
                constructors: &declarations.constructors,
                given: declarations.ctor_fields.get(&function).cloned().unwrap_or_default(),
                listed,
                defers: &declarations.defers,
                deferred: None,
            };
            let lowered = body::BodyLowerer::lower(builder, body, root, &declarations.types, context)
                .map_err(|error| error.in_function(&symbol))?;
            declarations.builder.define_function(function, lowered)?;
        }
    }
    for deferred in &declarations.defers {
        let function = deferred.function;
        let root = declarations.builder.declare_scope(ir::ScopeDecl {
            function, parent: None, kind: ir::ScopeKind::Action, loc: None,
        })?;
        let symbol = declarations.builder.module().functions.get(function)?.symbol.clone();
        let builder = declarations.builder.function_builder(function, root)?;
        let context = body::BodyContext {
            functions: &declarations.functions,
            objects: &declarations.objects,
            constructors: &declarations.constructors,
            given: vec![], listed: None, defers: &declarations.defers,
            deferred: Some(deferred.receiver),
        };
        let lowered = body::BodyLowerer::lower(builder, &deferred.source, root, &declarations.types, context)
            .map_err(|error| error.in_function(&symbol))?;
        declarations.builder.define_function(function, lowered)?;
    }
    let mut module = declarations.builder.finish()?;
    structure::finish(&mut module)?;
    structure::check(&module)?;
    Ok(module)
}
