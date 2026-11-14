#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifyLimits {
    pub entities: usize,
    pub blocks_per_function: usize,
    pub instructions_per_function: usize,
    pub values_per_function: usize,
    pub expression_nodes_per_function: usize,
    pub expression_depth: usize,
    pub outcome_slots: usize,
    pub memory_bytes: usize,
    pub steps: usize,
    pub diagnostics: usize,
}
impl Default for VerifyLimits {
    fn default() -> Self {
        Self {
            entities: 1 << 22,
            blocks_per_function: 1 << 18,
            instructions_per_function: 1 << 20,
            values_per_function: 1 << 20,
            expression_nodes_per_function: 1 << 20,
            expression_depth: 128,
            outcome_slots: 65536,
            memory_bytes: 256 << 20,
            steps: 50_000_000,
            diagnostics: 256,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VerifyStats {
    pub entities: usize,
    pub steps: usize,
    /// Conservative charged allocation bound; never a measurement of process RSS.
    pub memory_bytes: usize,
    pub functions: usize,
}

pub(super) fn add(current: &mut usize, amount: usize, limit: usize) -> bool {
    match current.checked_add(amount) {
        Some(next) if next <= limit => {
            *current = next;
            true
        }
        _ => false,
    }
}
