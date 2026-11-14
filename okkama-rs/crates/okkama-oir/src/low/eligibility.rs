use super::*;

fn unsupported(function: FuncId, message: &str) -> LowError {
    LowError::Unsupported { function, message: message.into() }
}
pub(super) fn check(module: &Module, budget: &mut Budget<'_>) -> Result<(), LowError> {
    let mut entities = 0_usize;
    for count in [module.types.len(), module.scopes.len(), module.functions.len(), module.declarations.len(),
        module.summaries.len(), module.protocols.len(), module.files.len()] {
        entities = entities.checked_add(count).ok_or(LowError::LimitExceeded("input entities"))?;
    }
    if entities > budget.options.max_entities {
        return Err(LowError::LimitExceeded("input entities"));
    }
    for (id, function) in module.functions.iter() {
        if function.body.is_some() && function.attrs.contains(&FunctionAttr::Async) {
            return Err(unsupported(id, "async function"));
        }
        if let Some(summary) = function.summary.filter(|_| function.body.is_some()) {
            let contract = &module.summaries.get(summary)?.contract;
            if contract.suspends || contract.async_destroy || contract.failures.contains(&FailureKind::Cancelled) {
                return Err(unsupported(id, "summary does not prove synchronous execution and owned cleanup"));
            }
        } else if function.body.is_some() {
            return Err(unsupported(id, "missing final function summary"));
        }
        let Some(body) = &function.body else {
            continue;
        };
        for count in [body.values.len(), body.blocks.len(), body.insts.len(), body.bindings.len(),
            body.boundaries.len(), body.deps.len(), body.keys.len(), body.selections.len(),
            body.descriptor_bindings.len(), body.service_schedules.len()] {
            entities = entities.checked_add(count).ok_or(LowError::LimitExceeded("input entities"))?;
        }
        if entities > budget.options.max_entities {
            return Err(LowError::LimitExceeded("input entities"));
        }
        if !body.service_schedules.is_empty() {
            return Err(unsupported(id, "nonempty service duty inventory"));
        }
        if body.cleanup != CleanupMetadata::default() || !body.high_results.is_empty() {
            return Err(unsupported(id, "input contains High or Low phase metadata"));
        }
        let entry = body.blocks.get(body.entry)?;
        for &parameter in &entry.params {
            budget.step(1)?;
            let opaque = match module.types.get(body.values.get(parameter)?.ty)? {
                Type::Service(ServiceType::Scope(_) | ServiceType::Pending) => true,
                Type::Service(ServiceType::Own) => {
                    let constructor = function.attrs.iter().any(|attr| matches!(attr, FunctionAttr::Ctor(_)));
                    let language_input = entry.params.iter().any(|value| {
                        body.values.get(*value).ok().and_then(|data| module.types.get(data.ty).ok())
                            .is_some_and(|ty| !matches!(ty, Type::Service(_) | Type::Descriptor))
                    });
                    !constructor && !language_input
                }
                _ => false,
            };
            if opaque {
                return Err(unsupported(id, "entry cleanup inventory has no typed synchronous ownership evidence"));
            }
        }
        for (_, value) in body.values.iter() {
            budget.step(1)?;
            if matches!(module.types.get(value.ty)?, Type::Frame(_)
                | Type::Service(ServiceType::TaskState | ServiceType::CacheState | ServiceType::CarrierState
                    | ServiceType::Pin | ServiceType::Receipt | ServiceType::UniqueCache)) {
                return Err(unsupported(id, "service duty or coroutine state requires a later lowering slice"));
            }
        }
        for binding in &body.descriptor_bindings {
            budget.step(1)?;
            let descriptor = &binding.contract;
            let contract = &module.summaries.get(descriptor.summary)?.contract;
            if descriptor.async_destroy || contract.suspends || contract.async_destroy
                || contract.failures.contains(&FailureKind::Cancelled) {
                return Err(unsupported(id, "descriptor permits asynchronous owned cleanup"));
            }
        }
        for (_, block) in body.blocks.iter() {
            for instruction in &block.insts {
                budget.step(1)?;
                operation(module, id, &body.insts.get(*instruction)?.op)?;
            }
            match &block.term.kind {
                TermKind::InvokeOwn { op, .. } => operation(module, id, op)?,
                TermKind::InvokeDefer { call, .. } => call_summary(module, id, call)?,
                TermKind::CleanupNext { .. } | TermKind::ExitDispatch { .. } | TermKind::FailureDispatch { .. } => {
                    return Err(unsupported(id, "Low terminator in Own"));
                }
                _ => {}
            }
        }
        for (_, block) in body.blocks.iter() {
            for instruction in &block.insts {
                graph::operation(body, &body.insts.get(*instruction)?.op, budget)?;
            }
            graph::terminator(body, &block.term.kind, budget)?;
        }
    }
    Ok(())
}
fn call_summary(module: &Module, function: FuncId, call: &CallSite) -> Result<(), LowError> {
    let summary = call.summary.ok_or_else(|| unsupported(function, "call has no checked summary"))?;
    let contract = &module.summaries.get(summary)?.contract;
    if contract.suspends || contract.async_destroy || contract.failures.contains(&FailureKind::Cancelled) {
        return Err(unsupported(function, "call permits suspension, cancellation or async-owned cleanup"));
    }
    Ok(())
}
fn operation(module: &Module, function: FuncId, op: &Operation) -> Result<(), LowError> {
    match op {
        Operation::Lexical(_) => return Err(unsupported(function, "High-only operation in Own")),
        Operation::Async(_) | Operation::Carrier(_) | Operation::Cleanup(_) => {
            return Err(unsupported(function, "async/service or preexisting Low operation"));
        }
        Operation::Ownership(OwnershipOp::Dynamic { .. } | OwnershipOp::DynamicCheck { .. }
            | OwnershipOp::DynamicRefresh { .. }) => {
            return Err(unsupported(function, "dynamic protocol sources require L3"));
        }
        Operation::Ownership(OwnershipOp::ScopeEnd { effects, .. }
            | OwnershipOp::ScopeUnwind { effects, .. }) => {
            if effects.suspends || effects.cancel || effects.failures.contains(&FailureKind::Cancelled) {
                return Err(unsupported(function, "D18 does not prove synchronous close"));
            }
        }
        Operation::Call(call) => {
            let site = match call {
                CallOp::Direct { call, .. } | CallOp::Role { call, .. } | CallOp::Closure { call, .. }
                | CallOp::Cap { call, .. } | CallOp::Native { call, .. } | CallOp::Intrinsic { call, .. }
                | CallOp::Resume { call, .. } => call,
            };
            call_summary(module, function, site)?;
        }
        _ => {}
    }
    Ok(())
}
