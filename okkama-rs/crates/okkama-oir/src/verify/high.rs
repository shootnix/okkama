use super::{graph::{Dominators, Edge}, EntityLocation as L, RuleId as R, State};
use crate::*;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn inside(module: &Module, mut scope: ScopeId, ancestor: ScopeId, state: &mut State<'_>) -> bool {
    let mut remaining = module.scopes.len();
    loop {
        if !state.step(1) { return false; }
        if scope == ancestor { return true; }
        if remaining == 0 { return false; }
        remaining -= 1;
        let Some(parent) = module.scopes.get(scope).ok().and_then(|s| s.parent) else { return false; };
        scope = parent;
    }
}
pub(super) fn check(module: &Module, selected: Option<FuncId>, state: &mut State<'_>) {
    if module.profile != Profile::High { return; }
    for (function, data) in module.functions.iter() {
        if selected.is_some_and(|id| id != function) { continue; }
        state.at(L::Function(function), data.loc);
        if data.signature.ownership_abi != OwnershipAbi::Unelaborated {
            state.error(R::H2, "high calls use unelaborated ownership ABI");
        }
        if data.attrs.contains(&FunctionAttr::Async)
            && (data.async_creation_summary.is_none() || data.async_body_summary.is_none()) {
            state.error(R::H6, "async high action requires creation and body summaries");
        }
        if data.attrs.contains(&FunctionAttr::Async) { async_summaries(module, data, state); }
        if data.signature.params.iter().chain(&data.signature.results).any(|ty| forbidden(module, *ty)) {
            state.error(R::H2, "high signatures cannot contain ownership service values");
        }
        let Some(body) = &data.body else { continue; };
        if !super::graph::eligible(function, body, state) { continue; }
        let Some(dom) = Dominators::new(body, state) else { continue; };
        scopes(module, function, body, &dom, state);
        super::high_defer::check(module, function, body, &dom.edges, state);
        binding_definitions(function, body, &dom, state);
        let mut bindings = BTreeSet::new();
        for (value, data) in body.values.iter() {
            if forbidden(module, data.ty) {
                state.at(L::Value { function, value }, None);
                state.error(R::H2, "ownership service values are forbidden in high");
            }
        }
        for (block_id, block) in body.blocks.iter() {
            for &instruction in &block.insts {
                let Ok(inst) = body.insts.get(instruction) else { continue; };
                state.at(L::Instruction { function, instruction }, inst.loc);
                operation(module, body, block, &inst.op, HighOperationSite::Instruction(instruction), &mut bindings,
                    state);
                if !inst.results.is_empty() && matches!(inst.op, Operation::Lexical(_) | Operation::Exit(_)) {
                    state.error(R::H3, "high intents must be statements without SSA results");
                }
            }
            state.at(L::Terminator { function, block: block_id }, block.term.loc);
            if let TermKind::InvokeOwn { op, .. } = &block.term.kind {
                if matches!(**op, Operation::Lexical(_) | Operation::Exit(_)) {
                    state.error(R::H3, "high intents cannot occur in invoke.own");
                }
                state.error(R::H2, "invoke.own belongs to elaborated ownership profiles");
            }
            result_records(module, body, block_id, block, &dom.edges, state);
            if !body.high_results.iter().any(|record| record.block == block_id) {
                exits(module, body, block_id, block, &dom.edges, state);
            }
        }
    }
}
fn forbidden(module: &Module, ty: TypeId) -> bool {
    module.types.get(ty).is_ok_and(|ty| match ty {
        Type::Service(service) => !matches!(service, ServiceType::Failure),
        Type::Frame(_) | Type::LowCell(_) => true,
        _ => false,
    })
}
fn scopes(module: &Module, function: FuncId, body: &Body, dom: &Dominators, state: &mut State<'_>) {
    let root = body.blocks.get(body.entry).map(|b| b.lexical_scope).ok();
    for (scope, decl) in module.scopes.iter().filter(|(_, s)| s.function == function) {
        if (Some(scope) == root && (decl.parent.is_some() || decl.kind != ScopeKind::Action))
            || (Some(scope) != root && decl.parent.is_none()) {
            state.at(L::Scope(scope), decl.loc);
            state.error(R::H1, "function lexical scopes must form one tree rooted at the entry action");
        }
        let mut entries = BTreeSet::new();
        if body.blocks.get(body.entry).is_ok_and(|b| inside(module, b.lexical_scope, scope, state)) {
            entries.insert(body.entry);
        }
        for edge in &dom.edges {
            if !state.step(1) { return; }
            let (Ok(source), Ok(target)) = (body.blocks.get(edge.source), body.blocks.get(edge.target)) else {
                continue;
            };
            let enters = !inside(module, source.lexical_scope, scope, state)
                && inside(module, target.lexical_scope, scope, state);
            if enters {
                entries.insert(edge.target);
            }
        }
        for edge in &dom.edges {
            if entries.contains(&edge.target)
                && body.blocks.get(edge.source).is_ok_and(|block| {
                    inside(module, block.lexical_scope, scope, state)
                }) {
                state.at(L::Scope(scope), decl.loc);
                state.error(R::H1, "scope subtree cannot branch back to its own entry block");
            }
        }
        if entries.len() > 1 {
            state.at(L::Scope(scope), decl.loc);
            state.error(R::H1, "lexical scope has multiple entry blocks");
        }
    }
}
fn operation(
    module: &Module,
    body: &Body,
    block: &Block,
    op: &Operation,
    site: HighOperationSite,
    bindings: &mut BTreeSet<BindingId>,
    state: &mut State<'_>,
) {
    if op.operand_refs().iter().any(|operand| match operand.reference {
        OperandRef::Value(value) => body.values.get(value).is_ok_and(|v| forbidden(module, v.ty)),
        _ => false,
    }) || matches!(op, Operation::Ownership(_) | Operation::Carrier(_) | Operation::Cleanup(_)) {
        state.error(R::H2, "high has no ownership tokens or elaborated ownership operations");
    }
    if let Some((call, _, _)) = super::summaries::call(op)
        && (call.packet.is_some() || !call.deps.is_empty()) {
        state.error(R::H2, "high calls have no ownership packet or deps operands");
    }
    if let Ok(contract) = ContractRegistry::new(module).high_site_schema(op, body, site)
        && (!contract.effects.failures.is_empty() || contract.effects.suspends) && block.unwind.is_none() {
        state.error(R::H5, "may-fail high operation requires an explicit block unwind target");
    }
    match op {
        Operation::Lexical(LexicalOp::Bind { binding, .. }) => {
            if !bindings.insert(*binding) { state.error(R::H7, "binding has multiple initial SSA definitions"); }
            if body.bindings.get(*binding).is_ok_and(|b| !inside(module, block.lexical_scope, b.scope, state)) {
                state.error(R::H7, "binding definition is outside its lexical scope");
            }
        }
        Operation::Lexical(LexicalOp::Rebind { binding, target, .. }) => {
            if body.bindings.get(*binding).is_ok_and(|b| b.scope != *target)
                || !inside(module, block.lexical_scope, *target, state) {
                state.error(R::H8, "rebind must explicitly target the original binding scope");
            }
        }
        Operation::Lexical(LexicalOp::Stage { action, .. }) => {
            if module.scopes.get(*action).is_ok_and(|s| s.kind != ScopeKind::Action)
                || !inside(module, block.lexical_scope, *action, state) {
                state.error(R::H8, "staging must explicitly target the enclosing action scope");
            }
        }
        Operation::Lexical(LexicalOp::DeferRegister { scope, .. }) => {
            if *scope != block.lexical_scope {
                state.error(R::H3, "defer.register must be a statement in its own scope");
            }
        }
        Operation::Lexical(LexicalOp::Temp { scope, .. }) => {
            if *scope != block.lexical_scope { state.error(R::H8, "temporary destination differs from its scope"); }
        }
        Operation::Exit(ExitOp::Prepare { exit }) => {
            if exit.packet.is_some() || exit.deps.is_some() { state.error(R::H3, "high exit intent has packet/deps"); }
        }
        Operation::Exit(_) => state.error(R::H2, "high permits only the exit.prepare intent"),
        Operation::Async(op) => {
            if !matches!(op, AsyncOp::Await { .. } | AsyncOp::WorkerBind { .. }
                | AsyncOp::WorkerSpawn { .. } | AsyncOp::ProcessSpawn { .. }) {
                state.error(R::H6, "high permits shared await; Future ownership/state operations require own");
            }
        }
        Operation::Core(CoreOp::ObjNew { declaration, fields, construction, .. }) => {
            let incomplete = fields.iter().any(|v| is_undef(body, *v));
            let pending = module.declarations.get(*declaration).is_ok_and(|d| match &d.kind {
                DeclarationKind::Object { fields, .. } => fields.last().is_some_and(|f| f.name == "$pending"
                    && module.types.get(f.ty) == Ok(&Type::Bool)),
                _ => false,
            });
            let unbound = fields.last().is_some_and(|value| is_undef(body, *value));
            let completed = fields.last().is_some_and(|value| super::construction::is_false(body, *value));
            if (*construction == ConstructionStatus::Incomplete && (!pending || !unbound))
                || (*construction == ConstructionStatus::Complete && (incomplete || (pending && !completed))) {
                state.error(R::H9, "early self requires Incomplete and a trailing $pending: bool field");
            }
        }
        Operation::Core(CoreOp::ObjInit { completion: Some(_), .. }) => {
            state.error(R::H9, "high completion uses obj.init $pending=false without ownership state");
        }
        _ => {}
    }
}
fn is_undef(body: &Body, value: ValueId) -> bool {
    body.values.get(value).is_ok_and(|v| match v.def {
        Definition::InstResult { inst, .. } => body.insts.get(inst)
            .is_ok_and(|i| matches!(i.op, Operation::Core(CoreOp::Undef { .. }))),
        _ => false,
    })
}
fn exits(module: &Module, body: &Body, id: BlockId, block: &Block, edges: &[Edge], state: &mut State<'_>) {
    let mut prepared = Vec::new();
    let mut wanted = BTreeSet::new();
    let mut discarded = BTreeSet::new();
    for &instruction in &block.insts {
        let Ok(inst) = body.insts.get(instruction) else { continue; };
        match &inst.op {
            Operation::Exit(ExitOp::Prepare { exit }) => {
                if let Some(value) = exit.value {
                    let boundary = body.boundaries.get(exit.boundary).ok();
                    if boundary.is_none_or(|b| !inside(module, block.lexical_scope, b.scope, state)) {
                        state.error(R::H4, "exit.prepare boundary must belong to an exited lexical ancestor");
                    }
                    prepared.push((value, exit.kind, boundary.map(|b| b.scope)));
                }
            }
            Operation::Lexical(LexicalOp::Want { value }) => { wanted.insert(*value); }
            Operation::Lexical(LexicalOp::Discard { value }) => { discarded.insert(*value); }
            _ => {}
        }
    }
    if let TermKind::Ret { values } = &block.term.kind
        && values.iter().any(|v| !prepared.iter().any(|(value, kind, _)| value == v && *kind == ExitKind::Return)) {
        state.error(R::H4, "every valued return needs exit.prepare in its source block");
    }
    if wanted.iter().any(|v| !prepared.iter().any(|(value, _, _)| value == v) || discarded.contains(v)) {
        state.error(R::H4, "wanted result needs exit.prepare and a unique want/discard demand");
    }
    for edge in edges.iter().filter(|edge| edge.source == id && !edge.entry) {
        let Ok(target) = body.blocks.get(edge.target) else { continue; };
        if inside(module, target.lexical_scope, block.lexical_scope, state) { continue; }
        if wanted.iter().any(|v| !prepared.iter().any(|(value, _, scope)| value == v
            && scope.is_some_and(|scope| !inside(module, target.lexical_scope, scope, state)))) {
            state.error(R::H4, "wanted exit.prepare must reserve a scope actually left by the edge");
        }
        let result_scope = module.scopes.get(block.lexical_scope)
            .is_ok_and(|s| matches!(s.kind, ScopeKind::Branch | ScopeKind::Block));
        if result_scope {
            let last = block.insts.last().and_then(|id| body.insts.get(*id).ok());
            if matches!(block.term.kind, TermKind::Br(_)) && last.is_some_and(|inst| {
                !matches!(inst.op, Operation::Lexical(_) | Operation::Exit(_))
                    && inst.results.iter().any(|v| !wanted.contains(v) && !discarded.contains(v))
            }) {
                state.error(R::H4, "discarded branch/block result still requires explicit discard");
            }
            for arg in &edge.args {
                if let EdgeArg::Value(value) = arg
                    && !wanted.contains(value) && !discarded.contains(value) {
                    state.error(R::H4, "branch/block result requires explicit want or discard");
                }
            }
        }
    }
    if matches!(block.term.kind, TermKind::Throw(_) | TermKind::Raise(_) | TermKind::Panic(_) | TermKind::Trap { .. })
        && module.scopes.get(block.lexical_scope).is_ok_and(|s| s.parent.is_some()) && block.unwind.is_none() {
        state.error(R::H5, "failure in a nested lexical scope requires an explicit unwind path");
    }
    if let Some(unwind) = &block.unwind {
        if unwind.target.args.iter().any(|v| !block.params.contains(v)) {
            state.error(R::H5, "high block unwind arguments must be entry parameters");
        }
        if body.blocks.get(unwind.target.block).is_ok_and(|b| !matches!(b.term.kind,
            TermKind::Catch { .. } | TermKind::Raise(_))) {
            state.error(R::H5, "high unwind must target catch dispatch or the function failure exit");
        }
    }
}

fn async_summaries(module: &Module, function: &Function, state: &mut State<'_>) {
    let language = |ty: &&TypeId| module.types.get(**ty)
        .is_ok_and(|ty| !matches!(ty, Type::Service(_) | Type::Descriptor));
    let params = function.signature.params.iter().filter(language).count();
    let results: Vec<_> = function.signature.results.iter().filter(language).copied().collect();
    if let Some(id) = function.async_body_summary
        && let Ok(summary) = module.summaries.get(id)
        && (summary.parameters.len() != params || summary.contract.results != results
            || summary.results.len() != results.len()) {
        state.error(R::H6, "async body summary must describe the action parameters and body results");
    }
    if let Some(id) = function.async_creation_summary && let Ok(summary) = module.summaries.get(id) {
        let future = summary.contract.results.first().and_then(|ty| module.types.get(*ty).ok());
        let correct_future = match future {
            Some(Type::Future(result)) => result.as_slice() == results.as_slice(),
            _ => false,
        };
        if summary.parameters.len() != params || summary.results.len() != 1
            || summary.contract.results.len() != 1 || !correct_future {
            state.error(R::H6, "async creation summary must return the Future of the body result");
        }
    }
}

fn binding_definitions(function: FuncId, body: &Body, dom: &Dominators, state: &mut State<'_>) {
    let mut definitions = BTreeMap::new();
    for (block_id, block) in body.blocks.iter() {
        for (position, &instruction) in block.insts.iter().enumerate() {
            if let Ok(Instruction { op: Operation::Lexical(LexicalOp::Bind { binding, .. }), .. })
                = body.insts.get(instruction) {
                definitions.insert(*binding, (block_id, position));
            }
        }
    }
    for (block_id, block) in body.blocks.iter() {
        for (position, &instruction) in block.insts.iter().enumerate() {
            let Ok(inst) = body.insts.get(instruction) else { continue; };
            let bindings = match &inst.op {
                Operation::Lexical(LexicalOp::Rebind { binding, .. }) => std::slice::from_ref(binding),
                Operation::Lexical(LexicalOp::DeferRegister { action, .. }) => &action.bindings,
                _ => continue,
            };
            for binding in bindings {
                let valid = definitions.get(binding).is_some_and(|&(definition, index)| {
                    if definition == block_id { index < position }
                    else { dom.dominates(Dominators::entry(definition) + 1, Dominators::entry(block_id), state) }
                });
                if !valid {
                    state.at(L::Instruction { function, instruction }, inst.loc);
                    state.error(R::H7, "binding's initial SSA definition must dominate rebind/defer registration");
                }
            }
        }
    }
}

fn result_records(
    module: &Module, body: &Body, id: BlockId, block: &Block, edges: &[Edge], state: &mut State<'_>,
) {
    let outgoing: Vec<_> = edges.iter().filter(|edge| edge.source == id && !edge.entry).collect();
    let mut records = BTreeSet::new();
    for record in body.high_results.iter().filter(|record| record.block == id) {
        let index = match record.edge {
            HighEdgeSelector::Normal(index) => Some(usize::from(index)),
            HighEdgeSelector::Unwind => None,
        };
        let target = index.and_then(|index| outgoing.get(index)).map(|edge| edge.target);
        let terminal = matches!(block.term.kind, TermKind::Ret { .. }) && index == Some(0);
        if !terminal && target.is_none() {
            state.error(R::H4, "result.exit selector does not name an outgoing normal edge");
        }
        if !records.insert((index, record.value)) {
            state.error(R::H4, "duplicate High result record");
        }
        match record.demand {
            ResultDemand::Discard => {
                if record.prepared.is_some() {
                    state.error(R::H4, "discarded result must retain its cleanup boundary without preparation");
                }
            }
            ResultDemand::Want => {
                let exit = record.prepared.and_then(|id| body.insts.get(id).ok()).and_then(|inst| {
                    if let Operation::Exit(ExitOp::Prepare { exit }) = &inst.op { Some(exit) } else { None }
                });
                let carried = terminal && matches!(&block.term.kind, TermKind::Ret { values }
                    if values.contains(&record.value))
                    || index.and_then(|index| outgoing.get(index)).is_some_and(|edge| {
                        edge.args.contains(&EdgeArg::Value(record.value))
                    });
                let valid = carried && exit.is_some_and(|exit| exit.value == Some(record.value)
                    && (!terminal || exit.kind == ExitKind::Return)
                    && body.boundaries.get(exit.boundary).is_ok_and(|boundary| {
                        inside(module, block.lexical_scope, boundary.scope, state)
                            && target.is_none_or(|target| body.blocks.get(target).is_ok_and(|block| {
                                !inside(module, block.lexical_scope, boundary.scope, state)
                            }))
                    }));
                if !valid { state.error(R::H4,
                    "wanted result record requires matching preparation and exited boundary"); }
            }
        }
    }
    for instruction in &block.insts {
        let demand = body.insts.get(*instruction).ok().and_then(|inst| match inst.op {
            Operation::Lexical(LexicalOp::Want { value }) => Some((value, ResultDemand::Want)),
            Operation::Lexical(LexicalOp::Discard { value }) => Some((value, ResultDemand::Discard)),
            _ => None,
        });
        if let Some((value, demand)) = demand
            && !body.high_results.iter().any(|record| record.block == id
                && record.value == value && record.demand == demand) {
            state.error(R::H4, "High result intent requires an explicit result.exit record");
        }
    }
    if let TermKind::Ret { values } = &block.term.kind {
        for value in values {
            if !body.high_results.iter().any(|record| record.block == id && record.value == *value
                && record.demand == ResultDemand::Want && record.edge == HighEdgeSelector::Normal(0)) {
                state.error(R::H4, "valued return requires an explicit wanted result record");
            }
        }
    }
}
