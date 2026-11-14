//! Independent, read-only verification of OIR-2.
//! Implements structure, types, SSA, edge contracts, authority, summary shapes
//! and core ownership/lifetime conservation. Async protocols and lowered cleanup
//! remain reserved passes; a certificate covers only the implemented subset.
mod diagnostic;
mod limits;
mod index;
mod structure;
mod types;
mod expressions;
pub(crate) mod read;
mod later;
mod low_phases;
mod linearity;
mod graph;
mod dominance;
mod contracts;
mod authority;
mod construction;
mod summaries;
mod high;
mod high_defer;

use crate::*;
pub use diagnostic::*;
pub use limits::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ErrorPolicy { First, #[default] All }
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExecutionMode { #[default] Production, Tests }
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VerifyOptions {
    pub errors: ErrorPolicy,
    pub limits: VerifyLimits,
    pub execution: ExecutionMode,
}

/// Trusted backend catalog. Input declarations cannot extend this catalog.
pub trait ExternalContracts {
    fn native(&self, name: &str) -> Option<&Signature>;
    fn intrinsic(&self, name: &str) -> Option<&Signature>;
    fn native_summary(&self, _: &str) -> Option<&Summary> { None }
    fn intrinsic_summary(&self, _: &str) -> Option<&Summary> { None }
}
#[derive(Debug)]
pub struct NoExternalContracts;
impl ExternalContracts for NoExternalContracts {
    fn native(&self, _: &str) -> Option<&Signature> { None }
    fn intrinsic(&self, _: &str) -> Option<&Signature> { None }
}
static NO_EXTERNAL_CONTRACTS: NoExternalContracts = NoExternalContracts;
#[derive(Clone, Copy)]
pub struct VerifyContext<'a> {
    pub text_locations: Option<&'a dyn TextLocations>,
    pub external_contracts: &'a dyn ExternalContracts,
}
impl std::fmt::Debug for VerifyContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerifyContext")
            .field("has_text_locations", &self.text_locations.is_some())
            .finish_non_exhaustive()
    }
}
impl Default for VerifyContext<'_> {
    fn default() -> Self {
        Self { text_locations: None, external_contracts: &NO_EXTERNAL_CONTRACTS }
    }
}

#[derive(Debug)]
pub struct VerifiedModule<'m> {
    module: &'m Module,
    profile: Profile,
    stats: VerifyStats,
}
impl<'m> VerifiedModule<'m> {
    pub fn module(&self) -> &'m Module { self.module }
    pub fn profile(&self) -> Profile { self.profile }
    pub fn stats(&self) -> &VerifyStats { &self.stats }
}

pub fn verify(module: &Module) -> Result<VerifiedModule<'_>, VerifyReport> {
    verify_with(module, &VerifyOptions::default(), VerifyContext::default())
}
macro_rules! profile_api {
    ($($name:ident => $profile:ident),* $(,)?) => {$(
        pub fn $name(module: &Module) -> Result<VerifiedModule<'_>, VerifyReport> {
            run(module, &VerifyOptions::default(), VerifyContext::default(), Some(Profile::$profile), None)
        }
    )*};
}
profile_api!(verify_high => High, verify_own => Own, verify_coro => Coro, verify_low => Low);
pub fn verify_with<'m>(
    module: &'m Module,
    options: &VerifyOptions,
    context: VerifyContext<'_>,
) -> Result<VerifiedModule<'m>, VerifyReport> {
    run(module, options, context, None, None)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionVerifyReport {
    pub function: FuncId,
    /// None means that the selected function passed the implemented checks.
    pub failure: Option<VerifyFailure>,
    pub diagnostics: Vec<VerifyDiagnostic>,
    pub suppressed: usize,
    pub stats: VerifyStats,
}
pub fn verify_function(
    module: &Module,
    function: FuncId,
    context: VerifyContext<'_>,
    options: &VerifyOptions,
) -> FunctionVerifyReport {
    match run(module, options, context, None, Some(function)) {
        Ok(verified) => FunctionVerifyReport {
            function, failure: None, diagnostics: vec![], suppressed: 0, stats: verified.stats,
        },
        Err(report) => FunctionVerifyReport {
            function, failure: Some(report.failure), diagnostics: report.diagnostics,
            suppressed: report.suppressed, stats: report.stats,
        },
    }
}

struct State<'a> {
    module: &'a Module,
    options: &'a VerifyOptions,
    context: VerifyContext<'a>,
    diagnostics: Vec<VerifyDiagnostic>,
    stats: VerifyStats,
    incomplete: bool,
    suppressed: usize,
    location: EntityLocation,
    source_loc: Option<DebugLoc>,
    poisoned: std::collections::BTreeSet<EntityLocation>,
    positions: std::collections::BTreeMap<(FuncId, InstId), (BlockId, usize)>,
    edge_positions: std::collections::BTreeMap<(FuncId, VerifyEdgeId), BlockId>,
}
impl State<'_> {
    fn at(&mut self, location: EntityLocation, source: Option<DebugLoc>) {
        self.location = location;
        self.source_loc = source;
    }
    fn error(&mut self, rule: RuleId, message: impl Into<String>) {
        if self.incomplete { return; }
        if !self.step(1) { return; }
        if matches!(rule, RuleId::S1 | RuleId::S2 | RuleId::S3)
            && !self.poisoned.contains(&self.location) {
            if !self.memory(64) { return; }
            self.poisoned.insert(self.location.clone());
        }
        if self.options.errors == ErrorPolicy::First
            && let Some(first) = self.diagnostics.first() {
            self.suppressed = self.suppressed.saturating_add(1);
            let old = diagnostic_key(self.module, &self.positions, &self.edge_positions, first.rule,
                &first.primary.entity);
            let next = diagnostic_key(self.module, &self.positions, &self.edge_positions, rule, &self.location);
            if next >= old { return; }
            self.diagnostics.clear();
        }
        if self.diagnostics.len() >= self.options.limits.diagnostics {
            self.suppressed = self.suppressed.saturating_add(1);
            self.incomplete = true;
            return;
        }
        if !self.memory(1024) { return; }
        self.diagnostics.push(VerifyDiagnostic {
            rule,
            primary: IrLocation {
                entity: self.location.clone(),
                text_span: self.context.text_locations.and_then(|table| table.span(&self.location)),
                source_loc: self.source_loc,
            },
            related: vec![], message: message.into(), witness: None,
        });
    }
    fn step(&mut self, count: usize) -> bool {
        if self.incomplete { return false; }
        if !limits::add(&mut self.stats.steps, count, self.options.limits.steps) {
            self.incomplete = true;
        }
        !self.incomplete
    }
    fn memory(&mut self, bytes: usize) -> bool {
        if !limits::add(&mut self.stats.memory_bytes, bytes, self.options.limits.memory_bytes) {
            self.incomplete = true;
        }
        !self.incomplete
    }
    fn array(&mut self, len: usize, size: usize) -> bool {
        match len.checked_mul(size) {
            Some(bytes) => self.memory(bytes),
            None => { self.incomplete = true; false }
        }
    }
    fn entities(&mut self, len: usize) {
        if !limits::add(&mut self.stats.entities, len, self.options.limits.entities) {
            self.incomplete = true;
        }
    }
}

fn run<'m>(
    module: &'m Module,
    options: &VerifyOptions,
    context: VerifyContext<'_>,
    profile: Option<Profile>,
    function: Option<FuncId>,
) -> Result<VerifiedModule<'m>, VerifyReport> {
    let mut state = State {
        module, options, context, diagnostics: vec![], stats: VerifyStats::default(),
        incomplete: false, suppressed: 0, location: EntityLocation::Module, source_loc: None,
        poisoned: std::collections::BTreeSet::new(), positions: std::collections::BTreeMap::new(),
        edge_positions: std::collections::BTreeMap::new(),
    };
    if profile.is_some_and(|expected| expected != module.profile) {
        state.error(RuleId::S1, "module profile does not match the requested verifier");
    }
    structure::check(module, function, &mut state);
    types::check(module, function, &mut state);
    dominance::check(module, function, &mut state);
    contracts::check(module, function, &mut state);
    authority::check(module, function, &mut state);
    summaries::check(module, function, &mut state);
    high::check(module, function, &mut state);
    linearity::check(module, function, &mut state);
    low_phases::check(module, function, &mut state);
    later::async_cleanup_and_profiles(module, function, &mut state);
    order_diagnostics(module, &mut state);
    // Passes emit in traversal order: module arenas, function/block arenas,
    // instruction positions, then operands/slots. No hash iteration controls diagnostics.
    if state.incomplete || !state.diagnostics.is_empty() {
        if options.errors == ErrorPolicy::First && state.diagnostics.len() > 1 {
            state.suppressed = state.suppressed.saturating_add(state.diagnostics.len() - 1);
            state.diagnostics.truncate(1);
        }
        return Err(VerifyReport {
            failure: if state.incomplete { VerifyFailure::Incomplete } else { VerifyFailure::Rejected },
            diagnostics: state.diagnostics, suppressed: state.suppressed, stats: state.stats,
        });
    }
    Ok(VerifiedModule { module, profile: module.profile, stats: state.stats })
}

#[cfg(test)]
mod tests;

type DiagnosticOrder = (u8, u8, u32, u32, u32, usize, u8, OperandPath, RuleId);
type InstructionPositions = std::collections::BTreeMap<(FuncId, InstId), (BlockId, usize)>;
type EdgePositions = std::collections::BTreeMap<(FuncId, VerifyEdgeId), BlockId>;

fn order_diagnostics(module: &Module, state: &mut State<'_>) {
    if !state.array(state.diagnostics.len(), 256) { return; }
    let positions = &state.positions;
    let edge_positions = &state.edge_positions;
    state.diagnostics.sort_by_cached_key(|diagnostic| {
        diagnostic_key(module, positions, edge_positions, diagnostic.rule, &diagnostic.primary.entity)
    });
}
fn diagnostic_key(
    module: &Module,
    positions: &InstructionPositions,
    edge_positions: &EdgePositions,
    rule: RuleId,
    entity: &EntityLocation,
) -> DiagnosticOrder {
    use EntityLocation as L;
    let mut function = None;
    let mut block = 0;
    let mut position = 0;
    let mut minor = 0;
    let mut path = OperandPath::default();
    let instruction_position = |function, instruction| {
        positions.get(&(function, instruction)).map_or((u32::MAX, usize::MAX), |(block, index)| {
            (block.as_u32(), index.saturating_add(1))
        })
    };
    let (class, entity) = match entity {
        L::Module => (0, 0),
        L::Declaration(id) => (1, id.as_u32()),
        L::Summary(id) => (2, id.as_u32()),
        L::Protocol(id) => (3, id.as_u32()),
        L::Type(id) => (4, id.as_u32()),
        L::Scope(id) => (5, id.as_u32()),
        L::Function(id) => { function = Some(*id); (6, 0) }
        L::Block { function: id, block: bid } => {
            function = Some(*id);
            block = bid.as_u32();
            (6, 0)
        }
        L::Instruction { function: id, instruction } => {
            function = Some(*id);
            (block, position) = instruction_position(*id, *instruction);
            (6, 0)
        }
        L::Terminator { function: id, block: bid } => {
            function = Some(*id);
            block = bid.as_u32();
            position = usize::MAX;
            (6, 0)
        }
        L::Operand { site, path: operand_path } => {
            match site {
                OperationSite::Instruction { function: id, instruction } => {
                    function = Some(*id);
                    (block, position) = instruction_position(*id, *instruction);
                }
                OperationSite::Terminator { function: id, block: bid } => {
                    function = Some(*id);
                    block = bid.as_u32();
                    position = usize::MAX;
                }
            }
            minor = 1;
            path = operand_path.clone();
            (6, 0)
        }
        L::Value { function: id, value } => {
            function = Some(*id);
            minor = 2;
            if let Ok(data) = module.functions.get(*id)
                && let Some(body) = &data.body
                && let Ok(value) = body.values.get(*value) {
                match value.def {
                    Definition::BlockParam { block: bid, index } => {
                        block = bid.as_u32();
                        path = OperandPath(vec![index as usize]);
                    }
                    Definition::InstResult { inst, index } => {
                        (block, position) = instruction_position(*id, inst);
                        path = OperandPath(vec![usize::from(index)]);
                    }
                }
            }
            (6, 0)
        }
        L::Edge { function: id, edge } => {
            function = Some(*id);
            minor = 3;
            block = edge_positions.get(&(*id, *edge)).map_or(u32::MAX, |id| id.as_u32());
            position = usize::MAX;
            path = OperandPath(vec![edge.0 as usize]);
            (6, 0)
        }
    };
    let pass = match rule {
        RuleId::S1 | RuleId::S2 | RuleId::S3 => 0,
        RuleId::T1 | RuleId::T2 | RuleId::T3 | RuleId::T4 | RuleId::T5 | RuleId::T6 => 1,
        RuleId::SSA1 | RuleId::SSA2 | RuleId::B61 => 2,
        RuleId::B62 => 3,
        RuleId::A1 | RuleId::A2 | RuleId::A3 => 4,
        RuleId::B51 => 5,
        _ => 6,
    };
    (pass, class, entity, function.map_or(0, FuncId::as_u32), block, position, minor, path, rule)
}
