use super::*;

impl Universe {
    pub(super) fn infer_body(
        &self,
        module: &Module,
        function: FuncId,
        types: &BTreeMap<TypeId, TypeClassification>,
        budget: &mut Budget<'_>,
    ) -> Result<Summary, OwnError> {
        let declaration = module.functions.get(function).map_err(arena)?;
        let body = declaration.body.as_ref().unwrap();
        let mut summary = bottom(module, declaration);
        budget.memory(body.values.len() * 256 + (body.insts.len() + body.blocks.len()) * 64)?;
        let mut values = vec![normal::empty_relation(); body.values.len()];
        let entry = body.blocks.get(body.entry).map_err(arena)?;
        let mut parameter = 0u16;
        for value in &entry.params {
            let ty = body.values.get(*value).map_err(arena)?.ty;
            if !language(module, &[ty]).is_empty() {
                if types[&ty].may_own() {
                    let relation = &mut values[value.as_u32() as usize];
                    relation.aliases.push(SummarySelection::Parameter(parameter));
                    relation.external_deps.push(parameter);
                }
                parameter = parameter.checked_add(1).ok_or_else(|| {
                    budget.fail(OwnDiagnosticKind::IncompatibleSignature, "too many language parameters")
                })?;
            }
        }
        let operations = operations(body);
        loop {
            let mut changed = false;
            for (site, op) in &operations {
                budget.work(1)?;
                let output = self.operation_relations(module, function, body, *site, op, &values, types, budget)?;
                if let Site::Instruction(instruction) = site {
                    for (result, relation) in body.insts.get(*instruction).map_err(arena)?.results.iter().zip(&output) {
                        changed |= join_value(&mut values, *result, relation, budget)?;
                    }
                }
            }
            for (block_id, block) in body.blocks.iter() {
                budget.work(1)?;
                let mut edge = |target: &Target| -> Result<(), OwnError> {
                    for (parameter, argument) in body.blocks.get(target.block).map_err(arena)?.params.iter()
                        .zip(&target.args) {
                        let relation = values[argument.as_u32() as usize].clone();
                        changed |= join_value(&mut values, *parameter, &relation, budget)?;
                    }
                    Ok(())
                };
                match &block.term.kind {
                    TermKind::Br(target) => edge(target)?,
                    TermKind::Cond { yes, no, .. } => {
                        edge(yes)?;
                        edge(no)?;
                    }
                    TermKind::Switch { cases, default, .. } => {
                        for case in cases {
                            edge(&case.target)?;
                        }
                        edge(default)?;
                    }
                    TermKind::Catch { next, .. } => edge(next)?,
                    TermKind::Lookup { missing, .. } => edge(missing)?,
                    _ => {}
                }
                let mut produced = vec![];
                if let TermKind::InvokeOwn { op, .. } = &block.term.kind {
                    produced = self.operation_relations(module, function, body, Site::Terminator(block_id),
                        op, &values, types, budget)?;
                }
                let mut invoke = |target: &InvokeEdge, normal: bool| -> Result<(), OwnError> {
                    for (parameter, argument) in body.blocks.get(target.block).map_err(arena)?.params.iter()
                        .zip(&target.args) {
                        let relation = match argument {
                            EdgeArg::Value(value) => values[value.as_u32() as usize].clone(),
                            EdgeArg::Produced(result) if normal => {
                                produced.get(usize::from(result.slot)).cloned().unwrap_or_else(normal::empty_relation)
                            }
                            _ => normal::empty_relation(),
                        };
                        changed |= join_value(&mut values, *parameter, &relation, budget)?;
                    }
                    Ok(())
                };
                match &block.term.kind {
                    TermKind::InvokeOwn { normal, unwind, .. } => {
                        invoke(normal, true)?;
                        invoke(unwind, false)?;
                    }
                    TermKind::InvokeDefer { continue_edge, replacements, unwind, .. } => {
                        invoke(continue_edge, false)?;
                        for replacement in replacements {
                            invoke(&replacement.edge, false)?;
                        }
                        if let Some(unwind) = unwind {
                            invoke(unwind, false)?;
                        }
                    }
                    TermKind::Catch { handler, .. } => invoke(handler, false)?,
                    TermKind::Lookup { .. } => {
                        // Lookup projections select elements, preserving full
                        // actual deps. Its one produced slot is handled below.
                    }
                    _ => {}
                }
                if let TermKind::Lookup { collection, found, .. } = &block.term.kind {
                    for (parameter, argument) in body.blocks.get(found.block).map_err(arena)?.params.iter()
                        .zip(&found.args) {
                        let relation = match argument {
                            EdgeArg::Value(value) => values[value.as_u32() as usize].clone(),
                            EdgeArg::Produced(_) => values[collection.as_u32() as usize].clone(),
                        };
                        changed |= join_value(&mut values, *parameter, &relation, budget)?;
                    }
                }
                if let Some(unwind) = &block.unwind {
                    for (parameter, argument) in body.blocks.get(unwind.target.block).map_err(arena)?.params.iter()
                        .zip(&unwind.target.args) {
                        let relation = values[argument.as_u32() as usize].clone();
                        changed |= join_value(&mut values, *parameter, &relation, budget)?;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        for (site, op) in operations {
            budget.work(1)?;
            if let Some(callee) = self.call_summary(function, site) {
                normal::failures(&mut summary.contract.failures, &callee.contract.failures);
                summary.contract.suspends |= callee.contract.suspends;
                summary.contract.async_destroy |= callee.contract.async_destroy;
                if let Some((call, receiver)) = call(op) {
                    let arguments: Vec<_> = receiver.iter().chain(&call.args).map(|arg| arg.value).collect();
                    for effect in &callee.parameters {
                        if effect.destination == TransferDestination::None
                            || effect.destination == TransferDestination::Result {
                            continue;
                        }
                        if let Some(selection) = &effect.retention {
                            let relation = instantiate(selection, &arguments, &values, budget)?;
                            retain_inputs(&mut summary, &relation, effect.destination.clone(), budget)?;
                        }
                    }
                }
            } else if matches!(op, Operation::Ownership(OwnershipOp::End { .. })) {
                // The legacy packet close has no D18 payload. Preserve its
                // declared failure envelope until cleanup specialization gives
                // it a scope contract; sync eligibility still comes from types.
                let contract = ContractRegistry::new(module).construction_schema(op, body)
                    .map_err(|error| budget.fail(OwnDiagnosticKind::InvalidEquation, format!("{error:?}")))?;
                normal::failures(&mut summary.contract.failures, &contract.effects.failures);
            } else if !is_scope_close(op) {
                let contract = ContractRegistry::new(module).high_site_schema(op, body, site.high())
                    .map_err(|error| budget.fail(OwnDiagnosticKind::InvalidEquation, format!("{error:?}")))?;
                normal::failures(&mut summary.contract.failures, &contract.effects.failures);
                summary.contract.suspends |= contract.effects.suspends;
            }
        }
        for (block_id, block) in body.blocks.iter() {
            match &block.term.kind {
                TermKind::Ret { values: returned } | TermKind::RetDefer { values: returned, .. } => {
                    let returned = returned.iter().filter(|value| body.values.get(**value).is_ok_and(|value| {
                        !language(module, &[value.ty]).is_empty()
                    }));
                    for (index, value) in returned.enumerate() {
                        if let Some(result) = summary.results.get_mut(index) {
                            normal::relation(result, &values[value.as_u32() as usize], budget)?;
                        }
                    }
                }
                TermKind::Throw(_) => normal::failures(&mut summary.contract.failures, &[FailureKind::Thrown]),
                TermKind::Panic(_) => normal::failures(&mut summary.contract.failures, &[FailureKind::Panic]),
                TermKind::Trap { .. } => normal::failures(&mut summary.contract.failures, &[FailureKind::Program]),
                // Raise forwards an existing failure. Its sources (operations,
                // throw/panic and accepted failure views) supply the kinds.
                TermKind::Raise(_) => {},
                TermKind::InvokeDefer { .. } => {
                    if let Some(callee) = self.call_summary(function, Site::Terminator(block_id)) {
                        normal::failures(&mut summary.contract.failures, &callee.contract.failures);
                        summary.contract.suspends |= callee.contract.suspends;
                    }
                }
                _ => {}
            }
        }
        if summary.entry_kind == EntryKind::CallBoundary {
            for result in summary.results.clone() {
                retain_inputs(&mut summary, &result, TransferDestination::Result, budget)?;
            }
        }
        if entry.params.iter().any(|value| body.values.get(*value).is_ok_and(|value| {
            module.types.get(value.ty) == Ok(&Type::Service(ServiceType::Failure))
        })) {
            normal::failures(&mut summary.contract.failures, &[FailureKind::Thrown, FailureKind::Program,
                FailureKind::Panic, FailureKind::Native, FailureKind::Cancelled]);
        }
        for (scope, facts) in &self.inventories {
            if module.scopes.get(*scope).map_err(arena)?.function != function {
                continue;
            }
            for ty in &facts.potential_owned_types {
                let effects = self.cleanup(module, &types[ty]);
                normal::failures(&mut summary.contract.failures, &effects.failures);
                summary.contract.suspends |= effects.suspends;
                summary.contract.async_destroy |= effects.suspends;
            }
            for defer in &facts.defers {
                normal::failures(&mut summary.contract.failures, &self.summary(*defer).contract.failures);
                summary.contract.suspends |= self.summary(*defer).contract.suspends;
            }
            for protocol in &facts.service_protocols {
                let mut effects = ScopeEffectEnvelope::default();
                project(&mut effects, self.protocol_summary(*protocol));
                normal::failures(&mut summary.contract.failures, &effects.failures);
                summary.contract.suspends |= effects.suspends;
                summary.contract.async_destroy |= effects.suspends;
            }
        }
        normal::normalize(&mut summary, budget)?;
        Ok(summary)
    }

    #[allow(clippy::too_many_arguments)]
    fn operation_relations(
        &self,
        module: &Module,
        function: FuncId,
        body: &Body,
        site: Site,
        op: &Operation,
        values: &[ResultRelation],
        types: &BTreeMap<TypeId, TypeClassification>,
        budget: &mut Budget<'_>,
    ) -> Result<Vec<ResultRelation>, OwnError> {
        if let Some((call, receiver)) = call(op) {
            let callee = self.call_summary(function, site).unwrap();
            let arguments: Vec<_> = receiver.iter().chain(&call.args).map(|arg| arg.value).collect();
            if arguments.len() != callee.parameters.len() {
                return Err(budget.fail(OwnDiagnosticKind::IncompatibleSignature, format!(
                    "call {function:?} {site:?}: {} actuals, {} parameters",
                    arguments.len(), callee.parameters.len(),
                )));
            }
            let mut results = vec![];
            for (index, relation) in callee.results.iter().enumerate() {
                let mut result = normal::empty_relation();
                for selection in &relation.aliases {
                    let selected = instantiate(selection, &arguments, values, budget)?;
                    normal::relation(&mut result, &selected, budget)?;
                }
                for dep in &relation.external_deps {
                    if let Some(argument) = arguments.get(usize::from(*dep)) {
                        let actual = &values[argument.as_u32() as usize];
                        result.external_deps.extend_from_slice(&actual.external_deps);
                        // Keep the full dependency positions without turning a
                        // selected field alias into retention of its container.
                    }
                }
                if !relation.fresh.is_empty() {
                    budget.work(body.keys.len() + body.insts.len() + body.blocks.len())?;
                    result.fresh.push(site.origin(body, index, self.strides[&function]));
                    result.provenance = relation.provenance;
                }
                results.push(result);
            }
            if call.packet.is_some() {
                results.extend([normal::empty_relation(), normal::empty_relation()]);
            }
            return Ok(results);
        }
        let contract = ContractRegistry::new(module).construction_schema(op, body)
            .map_err(|error| budget.fail(OwnDiagnosticKind::InvalidEquation, format!("{error:?}")))?;
        let mut results = vec![normal::empty_relation(); contract.normal.results.len()];
        for rule in &contract.normal.retention {
            if let Some(index) = contract.normal.results.iter().position(|slot| slot.name == rule.result) {
                let retained = retention(&rule.selection, values, budget)?;
                normal::relation(&mut results[index], &retained, budget)?;
            }
        }
        match op {
            Operation::Exit(ExitOp::Prepare { exit }) if !results.is_empty() => {
                if let Some(value) = exit.value {
                    results[0] = values[value.as_u32() as usize].clone();
                }
            }
            Operation::Exit(ExitOp::Replace { pending, replacement }) => {
                if let Some(value) = replacement.value {
                    results[0] = values[value.as_u32() as usize].clone();
                }
                if results.len() > 1 {
                    results[1] = values[pending.as_u32() as usize].clone();
                }
            }
            Operation::Exit(ExitOp::Extend { pending, .. }) => {
                results[0] = values[pending.as_u32() as usize].clone();
            }
            Operation::Exit(ExitOp::Commit { pending, result: Some(_), .. }) => {
                results[0] = values[pending.as_u32() as usize].clone();
                results[0].provenance = AdmissionProvenance::UnboundResult;
            }
            Operation::Ownership(OwnershipOp::ScopeEnd { reserve: Some(reserve), .. }) => {
                if let Some(index) = contract.normal.results.iter().position(|slot| slot.name == SlotName::Pending) {
                    results[index] = values[reserve.as_u32() as usize].clone();
                }
            }
            _ => {}
        }
        let created = matches!(op, Operation::Core(CoreOp::ObjNew { construction: ConstructionStatus::Complete, .. }
            | CoreOp::ObjNext { .. } | CoreOp::VariantNew { .. } | CoreOp::FakeNew { .. }));
        let result_types = match site {
            Site::Instruction(inst) => body.insts.get(inst).map_err(arena)?.results.iter().map(|value| {
                body.values.get(*value).map(|value| value.ty).map_err(arena)
            }).collect::<Result<Vec<_>, _>>()?,
            Site::Terminator(block) => {
                let mut result_types = vec![None; results.len()];
                if let TermKind::InvokeOwn { normal, .. } = &body.blocks.get(block).map_err(arena)?.term.kind {
                    for (parameter, argument) in body.blocks.get(normal.block).map_err(arena)?.params.iter()
                        .zip(&normal.args) {
                        if let EdgeArg::Produced(result) = argument {
                            let ty = body.values.get(*parameter).map_err(arena)?.ty;
                            result_types[usize::from(result.slot)] = Some(ty);
                        }
                    }
                }
                // Unused output slots need no SSA relation.
                result_types.into_iter().map(|ty| ty.unwrap_or(TypeId::from_u32(u32::MAX))).collect()
            }
        };
        for (index, (result, ty)) in results.iter_mut().zip(result_types).enumerate() {
            if let Some(facts) = types.get(&ty) {
                if !facts.may_own() {
                    *result = normal::empty_relation();
                } else if created {
                    budget.work(body.keys.len() + body.insts.len() + body.blocks.len())?;
                    result.fresh.push(site.origin(body, index, self.strides[&function]));
                }
            }
        }
        Ok(results)
    }

}

fn is_scope_close(op: &Operation) -> bool {
    matches!(op, Operation::Ownership(OwnershipOp::ScopeEnd { .. } | OwnershipOp::ScopeUnwind { .. }
        | OwnershipOp::End { .. }) | Operation::Cleanup(_))
}

fn join_value(
    values: &mut [ResultRelation],
    value: ValueId,
    relation: &ResultRelation,
    budget: &mut Budget<'_>,
) -> Result<bool, OwnError> {
    budget.work(1)?;
    let target = &mut values[value.as_u32() as usize];
    let before = target.clone();
    normal::relation(target, relation, budget)?;
    if before != *target {
        budget.memory(128 + target.aliases.len() * 128 + target.fresh.len() * 4)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn retain_inputs(
    summary: &mut Summary,
    relation: &ResultRelation,
    destination: TransferDestination,
    budget: &mut Budget<'_>,
) -> Result<(), OwnError> {
    let mut selections = normal::selections(&relation.aliases, budget)?;
    for parameter in &relation.external_deps {
        if !selections.iter().any(|selection| normal::parameter(selection) == *parameter) {
            selections.push(SummarySelection::OwnerGraph(*parameter));
        }
    }
    for selection in selections {
        let parameter = normal::parameter(&selection);
        let effect = summary.parameters.get_mut(usize::from(parameter)).ok_or_else(|| {
            budget.fail(OwnDiagnosticKind::IncompatibleSignature, "retention index outside boundary")
        })?;
        effect.convention = FreshConvention::Take;
        let mut items: Vec<_> = effect.retention.take().into_iter().collect();
        items.push(selection);
        effect.retention = Some(SummarySelection::Union(normal::selections(&items, budget)?));
        effect.deps.extend_from_slice(&relation.external_deps);
        if effect.destination != TransferDestination::None && effect.destination != destination {
            return Err(budget.fail(OwnDiagnosticKind::IncompatibleDestinationJoin, format!(
                "parameter {parameter}: {:?} conflicts with {destination:?}", effect.destination,
            )));
        }
        effect.destination = destination.clone();
    }
    Ok(())
}

fn project_relation(
    relation: &ResultRelation,
    field: Option<FieldRef>,
    owner_graph: bool,
    budget: &mut Budget<'_>,
) -> Result<ResultRelation, OwnError> {
    let mut result = relation.clone();
    result.aliases.clear();
    for selection in normal::selections(&relation.aliases, budget)? {
        let parameter = normal::parameter(&selection);
        let mut fields = match selection {
            SummarySelection::Field { field, .. } => vec![field],
            SummarySelection::Path { fields, .. } => fields,
            SummarySelection::OwnerGraph(_) | SummarySelection::Descriptor(_) => {
                result.aliases.push(SummarySelection::OwnerGraph(parameter));
                continue;
            }
            _ => vec![],
        };
        if let Some(field) = field {
            fields.push(field);
        }
        result.aliases.push(if owner_graph || fields.len() > 4 {
            SummarySelection::OwnerGraph(parameter)
        } else if fields.is_empty() {
            SummarySelection::Parameter(parameter)
        } else {
            SummarySelection::Path { parameter, fields }
        });
    }
    Ok(result)
}

fn instantiate(
    selection: &SummarySelection,
    arguments: &[ValueId],
    values: &[ResultRelation],
    budget: &mut Budget<'_>,
) -> Result<ResultRelation, OwnError> {
    let mut result = normal::empty_relation();
    for selection in normal::selections(std::slice::from_ref(selection), budget)? {
        let parameter = normal::parameter(&selection);
        let value = arguments.get(usize::from(parameter)).ok_or_else(|| {
            budget.fail(OwnDiagnosticKind::IncompatibleSignature, "selection index outside call arguments")
        })?;
        let mut actual = values[value.as_u32() as usize].clone();
        match selection {
            SummarySelection::Field { field, .. } => {
                actual = project_relation(&actual, Some(field), false, budget)?;
            }
            SummarySelection::Path { fields, .. } => {
                for field in fields {
                    actual = project_relation(&actual, Some(field), false, budget)?;
                }
            }
            SummarySelection::OwnerGraph(_) | SummarySelection::Descriptor(_) => {
                actual = project_relation(&actual, None, true, budget)?;
            }
            _ => {}
        }
        normal::relation(&mut result, &actual, budget)?;
    }
    Ok(result)
}

fn retention(
    retention: &Retention,
    values: &[ResultRelation],
    budget: &mut Budget<'_>,
) -> Result<ResultRelation, OwnError> {
    let mut field = None;
    let mut owner = false;
    let inputs: Vec<_> = match retention {
        Retention::Global(_) => vec![],
        Retention::Value(value) | Retention::Index { base: value, .. }
        | Retention::Slice { base: value, .. } | Retention::Keys(value) | Retention::Values(value)
        | Retention::MapEntry { map: value, .. } | Retention::MapPosition { map: value, .. }
        | Retention::ArrayRemove { array: value, .. } | Retention::Difference { base: value, .. }
        | Retention::Cache(value) | Retention::TakenOutcome { future: value, .. }
        | Retention::AwaitFailure { future: value, .. } => vec![*value],
        Retention::Field { base, field: selected } => {
            field = Some(*selected);
            vec![*base]
        }
        Retention::OwnerGraph(value) | Retention::ViewOwner(value)
        | Retention::Descriptor { value, .. } => {
            owner = true;
            vec![*value]
        }
        Retention::ArrayReplace { array, value, .. } => vec![*array, *value],
        Retention::MapMerge(inputs) | Retention::Union(inputs) | Retention::CarrierGraph(inputs) => inputs.clone(),
        Retention::ObjectFields { object, updates } => {
            let mut inputs = vec![*object];
            inputs.extend(updates.iter().map(|update| update.value));
            inputs
        }
        Retention::Entries(entries) => entries.iter().flat_map(|(key, value)| [*key, *value]).collect(),
    };
    let mut result = normal::empty_relation();
    for input in inputs {
        let actual = project_relation(&values[input.as_u32() as usize], field, owner, budget)?;
        normal::relation(&mut result, &actual, budget)?;
    }
    Ok(result)
}
