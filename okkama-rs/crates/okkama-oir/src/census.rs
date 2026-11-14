use crate::{DynamicReason, FuncId};
use std::collections::BTreeMap;

/// Counts emitted fallback sites, not runtime executions. Function IDs belong
/// to one module; merge requires the same function ID namespace.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DynamicCensus {
    functions: BTreeMap<FuncId, BTreeMap<DynamicReason, u64>>,
    module: BTreeMap<DynamicReason, u64>,
}
impl DynamicCensus {
    pub fn add(&mut self, function: FuncId, reason: DynamicReason, count: u64) {
        if count == 0 { return; }
        *self.functions.entry(function).or_default().entry(reason).or_default() += count;
        *self.module.entry(reason).or_default() += count;
    }
    pub fn merge(&mut self, other: &Self) {
        for (&function, counts) in &other.functions {
            for (&reason, &count) in counts { self.add(function, reason, count); }
        }
    }
    pub fn function_count(&self, function: FuncId, reason: DynamicReason) -> u64 {
        self.functions.get(&function).and_then(|counts| counts.get(&reason)).copied().unwrap_or(0)
    }
    pub fn module_count(&self, reason: DynamicReason) -> u64 { self.module.get(&reason).copied().unwrap_or(0) }
    pub fn functions(&self) -> impl Iterator<Item = (FuncId, &BTreeMap<DynamicReason, u64>)> { self.functions.iter().map(|(&id, counts)| (id, counts)) }
}
