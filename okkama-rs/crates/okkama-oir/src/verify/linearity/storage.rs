use super::*;

impl Engine<'_, '_> {
    pub(super) fn storage_operation(
        &mut self, op: &Operation, outcome: Outcome, origin: Origin, before: &Flow,
        flow: &mut Flow, slots: &mut [Slot],
    ) {
        if outcome != Outcome::Normal {
            return;
        }
        let Operation::Cleanup(op) = op else {
            return;
        };
        match op {
            CleanupOp::CellNew { value, .. } => {
                let index = value.as_u32() as usize;
                let stored = Slot {
                    refs: before.refs[index].clone(), requirements: before.full_deps[index].clone(),
                    aliases: before.aliases[index].clone(), identity: before.identities[index],
                    lifetime: before.lifetimes[index], token: before.tokens[index].clone(),
                    failures: before.failures[index], ..Slot::default()
                };
                flow.cells.insert(origin, stored.clone());
                if let Some(result) = slots.first_mut() {
                    result.token.cell = Some(origin);
                    result.refs = stored.refs;
                    result.requirements = stored.requirements;
                    result.lifetime = stored.lifetime;
                }
            }
            CleanupOp::CellLoad { cell } => {
                let key = before.tokens[cell.as_u32() as usize].cell;
                if let Some(stored) = key.and_then(|key| before.cells.get(&key))
                    && let Some(result) = slots.first_mut() {
                    *result = stored.clone();
                }
            }
            CleanupOp::CellStore { cell, value } => {
                let Some(key) = before.tokens[cell.as_u32() as usize].cell else {
                    self.error(R::B73, "raw Cell storage needs its checked borrowed-content state");
                    return;
                };
                let index = value.as_u32() as usize;
                let mut stored = Slot {
                    refs: before.refs[index].clone(), requirements: before.full_deps[index].clone(),
                    aliases: before.aliases[index].clone(), identity: before.identities[index],
                    lifetime: before.lifetimes[index], token: before.tokens[index].clone(),
                    failures: before.failures[index], ..Slot::default()
                };
                if matches!(key, Origin::RetiredInstruction(..) | Origin::RetiredInvoke(..))
                    && let Some(old) = before.cells.get(&key) {
                    stored.refs.extend(&old.refs);
                    stored.requirements.extend(&old.requirements);
                    stored.lifetime |= old.lifetime;
                    stored.failures |= old.failures;
                    if stored.identity != old.identity {
                        stored.identity = None;
                    }
                }
                for (value, token) in flow.tokens.iter().enumerate() {
                    if token.cell == Some(key) {
                        flow.refs[value] = stored.refs.clone();
                        flow.full_deps[value] = stored.requirements.union(&stored.refs).copied().collect();
                        flow.lifetimes[value] = stored.lifetime;
                    }
                }
                flow.cells.insert(key, stored);
            }
            _ => {}
        }
    }
}
