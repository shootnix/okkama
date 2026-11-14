use super::*;

impl Universe {
    pub fn annotate(
        &self,
        module: &mut Module,
        scopes: &BTreeMap<ScopeId, ScopeFacts>,
        budget: &mut Budget<'_>,
    ) -> Result<(), OwnError> {
        let mut ids = vec![];
        for node in &self.nodes {
            budget.memory(normal::size(&node.summary))?;
            ids.push(module.summaries.alloc(node.summary.clone()).map_err(arena)?);
        }
        for (function, data) in module.functions.iter_mut() {
            data.summary = Some(ids[function.as_u32() as usize]);
            let Some(body) = &mut data.body else {
                continue;
            };
            let mut records = vec![];
            for (instruction, data) in body.insts.iter_mut() {
                annotate_op(function, Site::Instruction(instruction), &mut data.op, &self.calls,
                    &ids, &self.nodes, &body.values, &module.types, scopes, &mut records);
            }
            for (block, data) in body.blocks.iter_mut() {
                if let TermKind::InvokeOwn { op, .. } = &mut data.term.kind {
                    annotate_op(function, Site::Terminator(block), op, &self.calls,
                        &ids, &self.nodes, &body.values, &module.types, scopes, &mut records);
                }
                if let TermKind::InvokeDefer { call, .. } = &mut data.term.kind
                    && let Some(node) = self.calls.get(&(function, Site::Terminator(block))) {
                    call.summary = Some(ids[*node]);
                    for (argument, effect) in call.args.iter_mut().zip(&self.nodes[*node].summary.parameters) {
                        argument.convention = convention(effect);
                    }
                }
            }
            if module.profile == Profile::High {
                for record in records {
                    if let Some(old) = body.high_effects.iter_mut().find(|old| old.site == record.site) {
                        *old = record;
                    } else {
                        body.high_effects.push(record);
                    }
                }
            }
        }
        Ok(())
    }
}

fn convention(effect: &ParameterEffect) -> OperandConvention {
    match effect.convention {
        FreshConvention::Borrow => OperandConvention::Borrow,
        FreshConvention::Take => OperandConvention::Consume,
    }
}

#[allow(clippy::too_many_arguments)]
fn annotate_op(
    function: FuncId,
    site: Site,
    op: &mut Operation,
    calls: &BTreeMap<(FuncId, Site), usize>,
    ids: &[SummaryId],
    nodes: &[Node],
    values: &Arena<ValueId, ValueData>,
    types: &TypeTable,
    scopes: &BTreeMap<ScopeId, ScopeFacts>,
    records: &mut Vec<HighEffectRecord>,
) {
    if let Some((call, receiver)) = call_mut(op)
        && let Some(node) = calls.get(&(function, site)) {
        let summary = &nodes[*node].summary;
        call.summary = Some(ids[*node]);
        for (argument, effect) in receiver.into_iter().chain(&mut call.args).zip(&summary.parameters) {
            argument.convention = convention(effect);
        }
        records.push(HighEffectRecord {
            site: site.high(),
            effects: HighEffectEnvelope {
                failures: summary.contract.failures.clone(),
                may_suspend: summary.contract.suspends || summary.contract.async_destroy,
            },
        });
    }
    if let Operation::Ownership(OwnershipOp::ScopeEnd { state, effects, .. }
        | OwnershipOp::ScopeUnwind { state, effects, .. }) = op
        && let Ok(value) = values.get(*state)
        && let Ok(Type::Service(ServiceType::Scope(scope))) = types.get(value.ty)
        && let Some(facts) = scopes.get(scope) {
        *effects = facts.effects.clone();
    }
}
