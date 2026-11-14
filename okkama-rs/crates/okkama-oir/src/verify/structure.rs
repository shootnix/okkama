use std::collections::BTreeSet;
use super::{expressions, index::references, read::Read, EntityLocation as L, RuleId as R, State};
use crate::*;

fn unique<'a>(names: impl IntoIterator<Item = &'a str>, state: &mut State<'_>) {
    let mut seen = BTreeSet::new();
    for name in names {
        if !state.step(1) { return; }
        if !state.step(name.len()) || !state.memory(64) { return; }
        if name.is_empty() || !seen.insert(name) { state.error(R::S1, "empty or duplicate symbol/slot name"); }
    }
}
fn refs<'a>(module: &'a Module, item: &impl Read, state: &mut State<'a>) {
    references(module, None, None, item, state);
}
fn signature<'a>(module: &'a Module, sig: &Signature, state: &mut State<'a>) {
    refs(module, &sig.params, state);
    refs(module, &sig.results, state);
    if sig.results.len() > state.options.limits.outcome_slots {
        state.incomplete = true;
        return;
    }
    if let OwnershipAbi::Explicit { input_packet, output_packet } = sig.ownership_abi {
        for (types, index) in [(&sig.params, input_packet), (&sig.results, output_packet)] {
            if !state.step(1) { return; }
            let valid = types.get(usize::from(index)).and_then(|id| module.types.get(*id).ok());
            if valid != Some(&Type::Service(ServiceType::Own)) {
                state.error(R::T5, "explicit ABI packet position must have type own");
            }
        }
    }
}
fn parameters<'a>(module: &'a Module, params: &[TypeParameter], state: &mut State<'a>) {
    unique(params.iter().map(|param| param.name.as_str()), state);
    for param in params { refs(module, &param.bounds, state); }
}

pub(super) fn check<'a>(module: &'a Module, selected: Option<FuncId>, state: &mut State<'a>) {
    for len in [module.declarations.len(), module.types.len(), module.scopes.len(),
        module.summaries.len(), module.protocols.len(), module.functions.len(), module.files.len()] {
        state.entities(len);
    }
    if state.incomplete { return; }
    if let Some(function) = selected {
        if module.functions.get(function).is_err() {
            state.at(L::Function(function), None);
            state.error(R::S1, "selected function does not exist");
            return;
        }
    } else {
        module_structure(module, state);
    }
    for (function, data) in module.functions.iter() {
        if !state.step(1) { return; }
        if selected.is_some_and(|selected| function != selected) { continue; }
        if !state.step(1) { return; }
        state.stats.functions = state.stats.functions.saturating_add(1);
        state.at(L::Function(function), data.loc);
        signature(module, &data.signature, state);
        parameters(module, &data.type_params, state);
        refs(module, &data.loc, state);
        for id in [data.summary, data.async_creation_summary, data.async_body_summary] {
            if !state.step(1) { return; }
            refs(module, &id, state);
        }
        let is_entry = module.entry == Some(function);
        if data.attrs.contains(&FunctionAttr::Entry) != is_entry {
            state.error(R::S1, "entry attribute must mark exactly Module.entry");
        }
        let mut externs = 0;
        let mut intrinsics = 0;
        if !state.array(data.attrs.len(), 64) { return; }
        let mut attributes = std::collections::HashSet::new();
        for attr in &data.attrs {
            if !state.step(1) { return; }
            let strings = match attr {
                FunctionAttr::Extern(name) | FunctionAttr::Intrinsic { name, .. } => name.len(),
                _ => 0,
            };
            if !state.step(strings) { return; }
            if !attributes.insert(attr) {
                state.error(R::S1, "duplicate function attribute");
            }
            match attr {
                FunctionAttr::Extern(name) => {
                    externs += 1;
                    if name.is_empty() { state.error(R::S1, "empty external ABI name"); }
                }
                FunctionAttr::Intrinsic { name, objects, helpers } => {
                    intrinsics += 1;
                    if name.is_empty() { state.error(R::S1, "empty intrinsic ABI name"); }
                    refs(module, objects, state);
                    refs(module, helpers, state);
                }
                FunctionAttr::Method(id) | FunctionAttr::Ctor(id) | FunctionAttr::Successor(id)
                | FunctionAttr::Destroy(id) => refs(module, id, state),
                FunctionAttr::Default { object, .. } => refs(module, object, state),
                FunctionAttr::Lambda { owner, captures } => {
                    refs(module, owner, state);
                    if *captures as usize > data.signature.params.len() {
                        state.error(R::S1, "lambda capture count exceeds its parameters");
                    }
                }
                FunctionAttr::DeferEntry { replacements } => {
                    if data.attrs.iter().filter(|attr| matches!(attr, FunctionAttr::DeferEntry { .. })).count() != 1 {
                        state.error(R::S1, "function must have exactly one defer entry declaration");
                    }
                    if replacements.len() > state.options.limits.outcome_slots {
                        state.incomplete = true;
                        return;
                    }
                    for label in replacements {
                        if label.result.is_some_and(|ty| module.types.get(ty).is_ok_and(|ty| {
                            matches!(ty, Type::Service(_) | Type::Frame(_) | Type::LowCell(_) | Type::Never)
                        })) {
                            state.error(R::T5, "defer replacement label requires a language result type");
                        }
                    }
                    unique(replacements.iter().map(|label| label.label.as_str()), state);
                    refs(module, replacements, state);
                }
                FunctionAttr::Cleanup | FunctionAttr::Entry | FunctionAttr::Hidden | FunctionAttr::Async => {}
            }
        }
        if externs > 1 || intrinsics > 1 || externs + intrinsics > 1
            || (data.body.is_some() == (externs + intrinsics == 1)) {
            state.error(R::S1, "function must have exactly one body, external ABI, or intrinsic ABI form");
        }
        if is_entry && (!data.signature.params.is_empty() || data.signature.results.len() != 1
            || data.signature.results.first().and_then(|id| module.types.get(*id).ok()) != Some(&Type::Int)) {
            state.error(R::S3, "module entry must have signature () -> int");
        }
        if let Some(body) = &data.body { body_structure(module, function, data, body, state); }
    }
}

fn module_structure<'a>(module: &'a Module, state: &mut State<'a>) {
    state.at(L::Module, None);
    unique(module.declarations.iter().map(|(_, decl)| decl.name.as_str())
        .chain(module.functions.iter().map(|(_, func)| func.symbol.as_str())), state);
    if let Some(entry) = module.entry { refs(module, &entry, state); }
    let mut features = BTreeSet::new();
    for feature in &module.requires {
        if !state.step(1) { return; }
        if !features.insert(*feature as u8) { state.error(R::S1, "duplicate required feature"); }
    }
    unique(module.imports.iter().filter_map(|import| import.alias.as_deref()), state);
    for import in &module.imports {
        if !state.step(1) { return; }
        if import.path.is_empty() || import.major == Some(0) {
            state.error(R::S1, "invalid import path or major version");
        }
        unique(import.take.iter().map(String::as_str), state);
    }
    if let Some(tests) = &module.tests {
        if !module.requires.contains(&Feature::Test) {
            state.error(R::S1, "test declarations require the test feature");
        }
        unique(tests.iter().map(|test| test.name.as_str()), state);
        for test in tests {
            if !state.step(1) { return; }
            refs(module, &test.function, state);
            if let Ok(function) = module.functions.get(test.function)
                && (!function.signature.params.is_empty() || !function.signature.results.is_empty()) {
                state.error(R::S1, "test function must have signature () -> void");
            }
        }
    }
    for (id, decl) in module.declarations.iter() {
        if !state.step(1) { return; }
        state.at(L::Declaration(id), decl.loc);
        refs(module, &decl.loc, state);
        parameters(module, &decl.type_params, state);
        match &decl.kind {
            DeclarationKind::Object { fields, witnesses, destroy, .. } => {
                unique(fields.iter().map(|field| field.name.as_str()), state);
                refs(module, destroy, state);
                for field in fields {
                    if !state.step(1) { return; }
                    refs(module, &field.ty, state);
                    refs(module, &field.default, state);
                }
                for witness in witnesses {
                    if !state.step(1) { return; }
                    refs(module, &witness.role, state);
                    refs(module, &witness.slots, state);
                }
            }
            DeclarationKind::Role { methods, .. } => {
                unique(methods.iter().map(|method| method.name.as_str()), state);
                for method in methods { signature(module, &method.signature, state); }
            }
            DeclarationKind::Enum { cases, .. } => unique(cases.iter().map(String::as_str), state),
            DeclarationKind::Variant { cases, .. } => {
                unique(cases.iter().map(|case| case.name.as_str()), state);
                for case in cases { refs(module, &case.layout, state); }
            }
            DeclarationKind::Cap { owner, fields } => {
                refs(module, owner, state);
                unique(fields.iter().map(|field| field.name.as_str()), state);
                for field in fields { refs(module, &field.function, state); }
            }
            DeclarationKind::Global { ty, init } => {
                refs(module, ty, state);
                refs(module, init, state);
            }
            DeclarationKind::Handle => {}
        }
    }
    for (id, ty) in module.types.iter() {
        if !state.step(1) { return; }
        state.at(L::Type(id), None);
        for child in super::types::children(ty) { refs(module, &child, state); }
        match ty {
            Type::Object { decl, .. } | Type::Role { decl, .. } | Type::Cap { decl, .. }
            | Type::Enum(decl) | Type::Variant(decl) | Type::Handle(decl) => refs(module, decl, state),
            Type::Frame(function) => refs(module, function, state),
            Type::Service(ServiceType::Scope(scope)) => refs(module, scope, state),
            Type::Parameter(parameter) => match parameter.owner {
                GenericOwner::Declaration(decl) => refs(module, &decl, state),
                GenericOwner::Function(function) => refs(module, &function, state),
            },
            _ => {}
        }
    }
    expressions::types(module, state);
    // Scope parent traversal is iterative and memoised even for unused scopes.
    if !state.array(module.scopes.len(), 32) { return; }
    let mut colours = vec![0_u8; module.scopes.len()];
    for (id, scope) in module.scopes.iter() {
        if !state.step(1) { return; }
        state.at(L::Scope(id), scope.loc);
        refs(module, &scope.function, state);
        refs(module, &scope.loc, state);
        if let Some(parent) = scope.parent
            && !module.scopes.get(parent).is_ok_and(|parent| parent.function == scope.function) {
            state.error(R::S2, "scope parent must belong to the same function");
        }
        let mut cursor = Some(id);
        let mut path = Vec::new();
        while let Some(next) = cursor {
            if !state.step(1) { return; }
            let Some(colour) = colours.get_mut(next.as_u32() as usize) else { break; };
            if *colour == 2 { break; }
            if *colour == 1 {
                state.error(R::S2, "cyclic scope parents");
                break;
            }
            *colour = 1;
            if !state.memory(4) { return; }
            path.push(next);
            cursor = module.scopes.get(next).ok().and_then(|scope| scope.parent);
        }
        for id in path { colours[id.as_u32() as usize] = 2; }
    }
    for (id, summary) in module.summaries.iter() {
        if !state.step(1) { return; }
        state.at(L::Summary(id), None);
        if summary.contract.results.len() > state.options.limits.outcome_slots {
            state.incomplete = true;
            return;
        }
        if !state.array(summary.parameters.len(), 64) { return; }
        refs(module, &summary.contract.results, state);
        refs(module, &summary.contract.protocol, state);
        for param in &summary.parameters {
            if !state.step(1) { return; }
            refs(module, &param.retention, state);
            if let TransferDestination::Scope(id) = param.destination { refs(module, &id, state); }
            if let TransferDestination::Protocol(id) = param.destination { refs(module, &id, state); }
            if param.deps.iter().any(|&id| usize::from(id) >= summary.parameters.len()) {
                state.error(R::S1, "summary dependency parameter is out of range");
            }
        }
        for result in &summary.results { refs(module, &result.aliases, state); }
    }
    for (id, protocol) in module.protocols.iter() {
        if !state.step(1) { return; }
        state.at(L::Protocol(id), None);
        let mut holders = BTreeSet::new();
        for holder in &protocol.holders {
            if !state.step(1) { return; }
            if !holders.insert(*holder as u8) { state.error(R::S1, "duplicate protocol holder"); }
        }
        for rule in &protocol.rules {
            if !state.step(1) { return; }
            for holder in [rule.from, rule.to, rule.normal.holder]
                .into_iter().chain(rule.unwind.map(|s| s.holder)).chain(rule.cancel.map(|s| s.holder)) {
                if !protocol.holders.contains(&holder) {
                    state.error(R::S1, "protocol rule references an undeclared holder");
                }
            }
        }
    }
}

fn body_structure<'a>(module: &'a Module, function: FuncId, data: &Function, body: &'a Body, state: &mut State<'a>) {
    let limits = &state.options.limits;
    if body.blocks.len() > limits.blocks_per_function || body.insts.len() > limits.instructions_per_function
        || body.values.len() > limits.values_per_function {
        state.incomplete = true;
        return;
    }
    for len in [body.blocks.len(), body.insts.len(), body.values.len(), body.bindings.len(),
        body.boundaries.len(), body.keys.len(), body.deps.len(), body.selections.len()] {
        state.entities(len);
    }
    if state.incomplete { return; }
    references(module, Some(body), Some(function), &body.entry, state);
    if let Ok(entry) = body.blocks.get(body.entry) {
        if entry.params.len() != data.signature.params.len() {
            state.error(R::S3, "entry parameter count differs from the function signature");
        }
        for (&value, expected) in entry.params.iter().zip(&data.signature.params) {
            if !state.step(1) { return; }
            if body.values.get(value).is_ok_and(|value| value.ty != *expected) {
                state.error(R::S3, "entry parameter type differs from the function signature");
            }
        }
    }
    for (_, binding) in body.bindings.iter() {
        if !state.step(1) { return; }
        references(module, Some(body), Some(function), binding, state);
    }
    for (_, boundary) in body.boundaries.iter() {
        if !state.step(1) { return; }
        references(module, Some(body), Some(function), boundary, state);
    }
    for (_, key) in body.keys.iter() { references(module, Some(body), Some(function), key, state); }
    for (_, dep) in body.deps.iter() { references(module, Some(body), Some(function), dep, state); }
    for (_, selection) in body.selections.iter() {
        if !state.step(1) { return; }
        references(module, Some(body), Some(function), selection, state);
    }
    references(module, Some(body), Some(function), &body.cleanup, state);
    references(module, Some(body), Some(function), &body.high_results, state);
    references(module, Some(body), Some(function), &body.high_effects, state);
    references(module, Some(body), Some(function), &body.descriptor_bindings, state);
    references(module, Some(body), Some(function), &body.service_schedules, state);
    expressions::check(body, state);
    if !state.array(body.insts.len(), 8) || !state.array(body.values.len(), 8) { return; }
    let mut inst_count = vec![0_u8; body.insts.len()];
    let mut value_count = vec![0_u8; body.values.len()];
    for (block_id, block) in body.blocks.iter() {
        if !state.step(1) { return; }
        state.at(L::Block { function, block: block_id }, block.term.loc);
        references(module, Some(body), Some(function), &block.lexical_scope, state);
        for (index, &value) in block.params.iter().enumerate() {
            if !state.step(1) { return; }
            if let Some(count) = value_count.get_mut(value.as_u32() as usize) { *count = count.saturating_add(1); }
            if !body.values.get(value).is_ok_and(|value| value.def == Definition::BlockParam {
                block: block_id, index: index as u32,
            }) { state.error(R::S3, "block parameter and ValueData.def disagree"); }
        }
        for (position, &inst_id) in block.insts.iter().enumerate() {
            if !state.step(1) { return; }
            if !state.memory(64) { return; }
            state.positions.entry((function, inst_id)).or_insert((block_id, position));
            if !state.step(1) { return; }
            state.at(L::Instruction { function, instruction: inst_id }, None);
            if let Some(count) = inst_count.get_mut(inst_id.as_u32() as usize) { *count = count.saturating_add(1); }
            let Ok(inst) = body.insts.get(inst_id) else {
                state.error(R::S3, "block references a missing instruction");
                continue;
            };
            state.source_loc = inst.loc;
            references(module, Some(body), Some(function), inst, state);
            if inst.results.len() > 65536 {
                state.error(R::S3, "instruction has more results than the index space");
            }
            for (index, &value) in inst.results.iter().enumerate() {
                if !state.step(1) { return; }
                if let Some(count) = value_count.get_mut(value.as_u32() as usize) { *count = count.saturating_add(1); }
                if !body.values.get(value).is_ok_and(|value| value.def == Definition::InstResult {
                    inst: inst_id, index: index as u16,
                }) { state.error(R::S3, "instruction result and ValueData.def disagree"); }
            }
            let mut slots = BTreeSet::new();
            for (slot, _) in &inst.facts.result_deps {
                if !state.step(1) { return; }
                if usize::from(*slot) >= inst.results.len() || !slots.insert(*slot) {
                    state.error(R::S3, "invalid or duplicate result-dependency slot");
                }
            }
        }
        state.at(L::Terminator { function, block: block_id }, block.term.loc);
        references(module, Some(body), Some(function), &block.term, state);
        references(module, Some(body), Some(function), &block.unwind, state);
    }
    for (id, _) in body.insts.iter() {
        if !state.step(1) { return; }
        if inst_count.get(id.as_u32() as usize) != Some(&1) {
            state.at(L::Instruction { function, instruction: id }, None);
            state.error(R::S3, "instruction must occur in exactly one block");
        }
    }
    for (id, value) in body.values.iter() {
        if !state.step(1) { return; }
        state.at(L::Value { function, value: id }, None);
        references(module, Some(body), Some(function), value, state);
        if value_count.get(id.as_u32() as usize) != Some(&1) {
            state.error(R::S3, "value must occur in exactly one parameter/result list");
        }
        if let Ok(Type::Service(ServiceType::Scope(scope))) = module.types.get(value.ty) {
            references(module, Some(body), Some(function), scope, state);
        }
    }
    reachability(body, function, state);
}

pub(super) fn targets(block: &Block) -> Vec<BlockId> {
    let mut targets = Vec::new();
    if let Some(unwind) = &block.unwind { targets.push(unwind.target.block); }
    match &block.term.kind {
        TermKind::CleanupNext { .. } | TermKind::ExitDispatch { .. } | TermKind::FailureDispatch { .. } => {
            if let TermKind::FailureDispatch { none, .. } = &block.term.kind { targets.push(none.block); }
            targets.extend(dispatch_edges(&block.term.kind).iter().map(|(_, edge)| edge.block));
        }
        TermKind::Br(target) => targets.push(target.block),
        TermKind::Cond { yes, no, .. } => targets.extend([yes.block, no.block]),
        TermKind::Switch { cases, default, .. } => {
            targets.extend(cases.iter().map(|case| case.target.block));
            targets.push(default.block);
        }
        TermKind::InvokeOwn { normal, unwind, .. } => targets.extend([normal.block, unwind.block]),
        TermKind::InvokeDefer { continue_edge, replacements, unwind, .. } => {
            targets.push(continue_edge.block);
            targets.extend(replacements.iter().map(|replacement| replacement.edge.block));
            targets.extend(unwind.iter().map(|edge| edge.block));
        }
        TermKind::Catch { handler, next, .. } => targets.extend([handler.block, next.block]),
        TermKind::Lookup { found, missing, .. } => targets.extend([found.block, missing.block]),
        TermKind::RetDefer { .. } | TermKind::Ret { .. } | TermKind::Throw(_) | TermKind::Raise(_) | TermKind::Panic(_)
        | TermKind::Trap { .. } | TermKind::Unreachable => {}
    }
    targets
}
fn reachability(body: &Body, function: FuncId, state: &mut State<'_>) {
    if !state.array(body.blocks.len(), 32) { return; }
    let mut seen = vec![false; body.blocks.len()];
    let mut pending = vec![body.entry];
    while let Some(id) = pending.pop() {
        if !state.step(1) { return; }
        let Some(visited) = seen.get_mut(id.as_u32() as usize) else { continue; };
        if *visited { continue; }
        *visited = true;
        let Ok(block) = body.blocks.get(id) else { continue; };
        let count = match &block.term.kind { TermKind::Switch { cases, .. } => cases.len(), _ => 2 };
        if !state.array(count, 8) { return; }
        for target in targets(block) {
            if !state.step(1) { return; }
            if target == body.entry {
                state.at(L::Block { function, block: id }, None);
                state.error(R::S3, "function entry cannot be a branch target");
            }
            pending.push(target);
        }
    }
    for (id, block) in body.blocks.iter() {
        if !state.step(1) { return; }
        if !seen[id.as_u32() as usize] && (!block.insts.is_empty() || !block.params.is_empty()
            || block.unwind.is_some() || block.term.kind != TermKind::Unreachable) {
            state.at(L::Block { function, block: id }, block.term.loc);
            state.error(R::S3, "unreachable block must be an empty unreachable tombstone");
        }
    }
}
