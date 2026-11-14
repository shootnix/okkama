//! Trusted, finite synchronous P1 boundary catalog. No generic fallback contracts.
//!
//! Atomic's four entries are VM operations, not its placeholder Okkama bodies.
//! `$int`/`$string` identify closed OIR-2 ABI specializations; no dictionaries remain.
//! All held values are plain: replacement stores a GC reference, never an ownership
//! obligation. Consequently all parameter retention, result aliases and deps are empty.
//! - `new_value(T) -> Atomic<T>`: call boundary, no semantic failure.
//! - `value(Atomic<T>) -> T`: observation, may throw AtomicClosed.
//! - `replace(Atomic<T>, T) -> void`: observation, may throw AtomicClosed before storing.
//! - `close(Atomic<T>) -> void`: observation, idempotent, no semantic failure.
//!
//! These registered observations keep admission at the caller.
//!
//! `Error.text(Error) -> string` observes the builtin plain Error view without failure.
//! `utf8_encode` and `stdout_write` are native call boundaries. Their results are a
//! plain byte array and an accepted byte count; host failures have Native kind.
//! All entries are synchronous, with no async destroy, publication receipt or resource result.
use std::collections::BTreeMap;

use crate::oir2 as ir;
use ir::own::{ExternalBoundary, ExternalSummary, NativeSummaries, NativeSummaryError, Substitution};

#[derive(Default)]
pub(crate) struct P1NativeSummaries {
    natives: BTreeMap<String, (ir::Signature, ir::Summary)>,
    intrinsics: BTreeMap<String, (ir::Signature, ir::Summary)>,
    services: BTreeMap<ir::TypeId, ir::Summary>,
}

fn error(message: impl Into<String>) -> NativeSummaryError {
    NativeSummaryError { message: message.into() }
}

impl P1NativeSummaries {
    pub fn for_module(module: &ir::Module) -> Result<Self, NativeSummaryError> {
        let mut catalog = Self::default();
        for (_, function) in module.functions.iter() {
            for attr in &function.attrs {
                let (native, name) = match attr {
                    ir::FunctionAttr::Extern(name) => (true, name),
                    ir::FunctionAttr::Intrinsic { name, .. } => (false, name),
                    _ => continue,
                };
                let summary = contract(module, name, native, &function.signature)?;
                if let ir::FunctionAttr::Intrinsic { objects, helpers, .. } = attr
                    && (*objects != intrinsic_objects(module, name, &function.signature)? || !helpers.is_empty()) {
                    return Err(error(format!("invalid P1 intrinsic descriptors for {name}")));
                }
                let entries = if native { &mut catalog.natives } else { &mut catalog.intrinsics };
                let entry = (function.signature.clone(), summary);
                if entries.get(name).is_some_and(|existing| existing != &entry) {
                    return Err(error(format!("conflicting P1 ABI {name}")));
                }
                entries.insert(name.clone(), entry);
            }
        }
        // Classification treats builtin objects/roles as services. These closed
        // plain specializations have no automatic close/destroy work. Atomic.close
        // remains an explicit source operation; plain Error views own no carrier.
        for (id, ty) in module.types.iter() {
            let decl = match ty {
                ir::Type::Object { decl, args } | ir::Type::Role { decl, args } if args.is_empty() => *decl,
                _ => continue,
            };
            let decl = module.declarations.get(decl).map_err(|err| error(err.to_string()))?;
            let atomic = matches!(decl.name.as_str(), "async.atomic.Atomic_int" | "async.atomic.Atomic_string")
                && matches!(&decl.kind, ir::DeclarationKind::Object {
                    builtin: true, destroy: None, fields, witnesses,
                } if fields.is_empty() && witnesses.is_empty());
            let plain_error = decl.name == "Error"
                && matches!(decl.kind, ir::DeclarationKind::Role { builtin: true, .. });
            if atomic || plain_error {
                catalog.services.insert(id, ir::Summary {
                    entry_kind: ir::EntryKind::Internal,
                    parameters: vec![], results: vec![],
                    contract: ir::CallContract {
                        results: vec![], failures: vec![], suspends: false, async_destroy: false,
                        publication: ir::Publication::Atomic, protocol: None,
                    },
                });
            }
        }
        Ok(catalog)
    }
}

impl NativeSummaries for P1NativeSummaries {
    fn lookup(&self, boundary: ExternalBoundary<'_>, subst: &Substitution)
        -> Result<ExternalSummary, NativeSummaryError> {
        if !subst.types.is_empty() || !subst.descriptors.is_empty() {
            return Err(error("P1 boundaries require a closed specialization"));
        }
        if let ExternalBoundary::Service(ty) = boundary {
            return self.services.get(&ty).cloned().map(|summary| ExternalSummary { summary })
                .ok_or_else(|| error(format!("unregistered P1 service {ty:?}")));
        }
        let entry = match boundary {
            ExternalBoundary::Native(name) => self.natives.get(name),
            ExternalBoundary::Intrinsic(name) => self.intrinsics.get(name),
            _ => None,
        };
        entry.map(|(_, summary)| ExternalSummary { summary: summary.clone() })
            .ok_or_else(|| error(format!("unregistered P1 boundary {boundary:?}")))
    }
}

impl ir::verify::ExternalContracts for P1NativeSummaries {
    fn native(&self, name: &str) -> Option<&ir::Signature> {
        self.natives.get(name).map(|entry| &entry.0)
    }
    fn intrinsic(&self, name: &str) -> Option<&ir::Signature> {
        self.intrinsics.get(name).map(|entry| &entry.0)
    }
    fn native_summary(&self, name: &str) -> Option<&ir::Summary> {
        self.natives.get(name).map(|entry| &entry.1)
    }
    fn intrinsic_summary(&self, name: &str) -> Option<&ir::Summary> {
        self.intrinsics.get(name).map(|entry| &entry.1)
    }
}

pub(super) fn contract(module: &ir::Module, name: &str, native: bool, signature: &ir::Signature)
    -> Result<ir::Summary, NativeSummaryError> {
    let find = |ty: ir::Type| module.types.iter().find_map(|(id, value)| (*value == ty).then_some(id));
    let int = || find(ir::Type::Int).ok_or_else(|| error("missing int"));
    let string = || find(ir::Type::Str).ok_or_else(|| error("missing string"));
    let bytes = find(ir::Type::Byte).and_then(|byte| find(ir::Type::Array(byte)));
    let mut kind = ir::EntryKind::CallBoundary;
    let mut failures = vec![];
    let (params, results) = match (native, name) {
        (true, "runtime/stdio@1::stdout_write/1") => {
            failures.push(ir::FailureKind::Native);
            (vec![bytes.ok_or_else(|| error("missing @byte"))?], vec![int()?])
        }
        (true, "runtime/utf@1::utf8_encode/1") => {
            failures.push(ir::FailureKind::Native);
            (vec![string()?], vec![bytes.ok_or_else(|| error("missing @byte"))?])
        }
        (false, "okkama.Error.text") => {
            kind = ir::EntryKind::ObservationAdmissionStaysCaller;
            let role = module.types.iter().find_map(|(id, ty)| match ty {
                ir::Type::Role { decl, args } if args.is_empty() => {
                    module.declarations.get(*decl).ok().filter(|decl| decl.name == "Error"
                        && matches!(decl.kind, ir::DeclarationKind::Role { builtin: true, .. })).map(|_| id)
                }
                _ => None,
            }).ok_or_else(|| error("missing builtin Error"))?;
            (vec![role], vec![string()?])
        }
        (false, name) => {
            let (operation, suffix) = name.strip_prefix("okkama.atomic.Atomic.")
                .and_then(|name| name.split_once('$')).ok_or_else(|| error(format!("unknown intrinsic {name}")))?;
            let value = match suffix {
                "int" => int()?,
                "string" => string()?,
                _ => return Err(error(format!("unsupported Atomic value {suffix}"))),
            };
            let object = module.types.iter().find_map(|(id, ty)| match ty {
                ir::Type::Object { decl, args } if args.is_empty() => module.declarations.get(*decl).ok()
                    .filter(|decl| decl.name == format!("async.atomic.Atomic_{suffix}"))
                    .filter(|decl| matches!(&decl.kind, ir::DeclarationKind::Object {
                        builtin: true, destroy: None, fields, witnesses,
                    } if fields.is_empty() && witnesses.is_empty())).map(|_| id),
                _ => None,
            }).ok_or_else(|| error("missing plain Atomic specialization"))?;
            match operation {
                "new_value" => (vec![value], vec![object]),
                "value" => {
                    kind = ir::EntryKind::ObservationAdmissionStaysCaller;
                    failures.push(ir::FailureKind::Thrown);
                    (vec![object], vec![value])
                }
                "replace" => {
                    kind = ir::EntryKind::ObservationAdmissionStaysCaller;
                    failures.push(ir::FailureKind::Thrown);
                    (vec![object, value], vec![])
                }
                "close" => {
                    kind = ir::EntryKind::ObservationAdmissionStaysCaller;
                    (vec![object], vec![])
                }
                _ => return Err(error(format!("unknown Atomic operation {operation}"))),
            }
        }
        _ => return Err(error(format!("unknown native {name}"))),
    };
    if signature.params != params || signature.results != results {
        return Err(error(format!("invalid P1 signature for {name}")));
    }
    Ok(ir::Summary {
        entry_kind: kind,
        parameters: params.iter().map(|_| ir::ParameterEffect {
            convention: ir::FreshConvention::Borrow,
            retention: None,
            deps: vec![],
            destination: ir::TransferDestination::None,
        }).collect(),
        results: results.iter().map(|_| ir::ResultRelation {
            fresh: vec![], aliases: vec![], external_deps: vec![], provenance: ir::AdmissionProvenance::UnboundResult,
        }).collect(),
        contract: ir::CallContract {
            results, failures, suspends: false, async_destroy: false, publication: ir::Publication::Atomic,
            protocol: None,
        },
    })
}

/// Allocation and typed-failure descriptors required independently of source catch use.
/// The constructor allocates Self; value/replace construct AtomicClosed.closed.
pub(super) fn intrinsic_objects(module: &ir::Module, name: &str, signature: &ir::Signature)
    -> Result<Vec<ir::DeclId>, NativeSummaryError> {
    if name.starts_with("okkama.atomic.Atomic.new_value$") {
        let result = signature.results.first().ok_or_else(|| error("Atomic constructor needs Self"))?;
        return match module.types.get(*result) {
            Ok(ir::Type::Object { decl, .. }) => Ok(vec![*decl]),
            _ => Err(error("Atomic constructor needs an object descriptor")),
        };
    }
    if name.starts_with("okkama.atomic.Atomic.value$") || name.starts_with("okkama.atomic.Atomic.replace$") {
        let decl = module.declarations.iter().find_map(|(id, decl)| {
            (decl.name == "async.atomic.AtomicClosed" && matches!(&decl.kind,
                ir::DeclarationKind::Enum { cases, is_error: true } if cases == &["closed"])).then_some(id)
        }).ok_or_else(|| error("Atomic access requires AtomicClosed.closed descriptor"))?;
        return Ok(vec![decl]);
    }
    Ok(vec![])
}
