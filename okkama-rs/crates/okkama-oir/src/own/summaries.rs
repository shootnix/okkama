//! The universe is frozen before SCC solving. Implicit cleanup edges are
//! ordinary dependencies, so destroy bodies and their locals solve together.
mod infer;
mod annotate;
mod adapters;
use adapters::adapt;

use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Site {
    Instruction(InstId),
    Terminator(BlockId),
}
impl Site {
    pub fn high(self) -> HighOperationSite {
        match self {
            Self::Instruction(id) => HighOperationSite::Instruction(id),
            Self::Terminator(id) => HighOperationSite::Terminator(id),
        }
    }
    fn origin(self, body: &Body, slot: usize, stride: usize) -> CreationSiteId {
        let value = match self {
            Self::Instruction(id) => body.insts.get(id).ok().and_then(|inst| inst.results.get(slot)).copied(),
            Self::Terminator(id) => body.blocks.get(id).ok().and_then(|block| {
                let TermKind::InvokeOwn { normal, .. } = &block.term.kind else {
                    return None;
                };
                body.blocks.get(normal.block).ok()?.params.iter().zip(&normal.args).find_map(|(value, arg)| {
                    matches!(arg, EdgeArg::Produced(result) if usize::from(result.slot) == slot).then_some(*value)
                })
            }),
        };
        if let Some(value) = value
            && let Some(site) = body.keys.iter().find_map(|(_, key)| match key {
                KeyExpr::Created { site, instance } if *instance == value => Some(*site),
                _ => None,
            }) {
            return site;
        }
        let mut layout = 0usize;
        let mut index = None;
        for (block_id, block) in body.blocks.iter() {
            for instruction in &block.insts {
                if self == Self::Instruction(*instruction) {
                    index = Some(layout);
                }
                layout += 1;
            }
            if self == Self::Terminator(block_id) {
                index = Some(layout);
            }
            layout += 1;
        }
        let index = index.unwrap();
        let base = body.keys.iter().filter_map(|(_, key)| match key {
            KeyExpr::Created { site, .. } => Some(site.as_u32() as usize + 1),
            _ => None,
        }).max().unwrap_or(0);
        // Slots are indexed before solving. The checked universe bound in build
        // guarantees this multiplication fits in the CreationSiteId namespace.
        CreationSiteId::from_u32((base + index * stride + slot) as u32)
    }
}

#[derive(Clone)]
enum Equation {
    Body(FuncId),
    External,
    Dispatch { targets: Vec<usize>, captures: Option<usize> },
}
struct Node {
    equation: Equation,
    summary: Summary,
    dependencies: BTreeSet<usize>,
}

pub(super) struct Universe {
    nodes: Vec<Node>,
    pub calls: BTreeMap<(FuncId, Site), usize>,
    strides: BTreeMap<FuncId, usize>,
    pub inventories: BTreeMap<ScopeId, ScopeFacts>,
    pub services: BTreeMap<TypeId, usize>,
    pub protocol_services: BTreeMap<ProtocolId, usize>,
    pub descriptors: BTreeMap<TypeId, Vec<DescriptorContract>>,
}

pub(super) fn bottom(module: &Module, function: &Function) -> Summary {
    let results = language(module, &function.signature.results);
    Summary {
        entry_kind: if function.signature.ownership_abi == OwnershipAbi::Observation {
            EntryKind::ObservationAdmissionStaysCaller
        } else if function.summary.and_then(|id| module.summaries.get(id).ok())
            .is_some_and(|summary| summary.entry_kind == EntryKind::Internal) {
            EntryKind::Internal
        } else {
            EntryKind::CallBoundary
        },
        parameters: language(module, &function.signature.params).iter().map(|_| ParameterEffect {
            convention: FreshConvention::Borrow,
            retention: None,
            deps: vec![],
            destination: TransferDestination::None,
        }).collect(),
        results: results.iter().map(|_| normal::empty_relation()).collect(),
        contract: CallContract {
            results,
            failures: vec![],
            suspends: false,
            async_destroy: false,
            publication: function.summary.and_then(|id| module.summaries.get(id).ok())
                .map_or(Publication::Atomic, |summary| summary.contract.publication),
            protocol: function.summary.and_then(|id| module.summaries.get(id).ok())
                .and_then(|summary| summary.contract.protocol),
        },
    }
}

pub(super) fn operations(body: &Body) -> Vec<(Site, &Operation)> {
    let mut operations = vec![];
    for (block_id, block) in body.blocks.iter() {
        for instruction in &block.insts {
            if let Ok(data) = body.insts.get(*instruction) {
                operations.push((Site::Instruction(*instruction), &data.op));
            }
        }
        if let TermKind::InvokeOwn { op, .. } = &block.term.kind {
            operations.push((Site::Terminator(block_id), op));
        }
    }
    operations
}

pub(super) fn call(op: &Operation) -> Option<(&CallSite, Option<CallArgument>)> {
    match op {
        Operation::Call(CallOp::Direct { call, .. } | CallOp::Native { call, .. }
            | CallOp::Intrinsic { call, .. } | CallOp::Resume { call, .. }) => Some((call, None)),
        Operation::Call(CallOp::Role { call, receiver, .. }) => Some((call, Some(*receiver))),
        Operation::Call(CallOp::Closure { call, callee }) => Some((call, Some(*callee))),
        Operation::Call(CallOp::Cap { call, cap, .. }) => Some((call, Some(*cap))),
        _ => None,
    }
}

fn call_mut(op: &mut Operation) -> Option<(&mut CallSite, Option<&mut CallArgument>)> {
    match op {
        Operation::Call(CallOp::Direct { call, .. } | CallOp::Native { call, .. }
            | CallOp::Intrinsic { call, .. } | CallOp::Resume { call, .. }) => Some((call, None)),
        Operation::Call(CallOp::Role { call, receiver, .. }) => Some((call, Some(receiver))),
        Operation::Call(CallOp::Closure { call, callee }) => Some((call, Some(callee))),
        Operation::Call(CallOp::Cap { call, cap, .. }) => Some((call, Some(cap))),
        _ => None,
    }
}

fn targets(module: &Module, body: &Body, op: &Operation, budget: &mut Budget<'_>) -> Result<Vec<usize>, OwnError> {
    let mut targets = BTreeSet::new();
    match op {
        Operation::Call(CallOp::Direct { function, .. } | CallOp::Native { function, .. }
            | CallOp::Intrinsic { function, .. } | CallOp::Resume { function, .. }) => {
            targets.insert(function.as_u32() as usize);
        }
        Operation::Call(CallOp::Cap { field, .. }) => {
            let declaration = module.declarations.get(field.declaration).map_err(arena)?;
            if let DeclarationKind::Cap { fields, .. } = &declaration.kind
                && let Some(field) = fields.get(field.index as usize) {
                targets.insert(field.function.as_u32() as usize);
            }
        }
        Operation::Call(CallOp::Role { role, slot, .. }) => {
            for (_, declaration) in module.declarations.iter() {
                budget.work(1)?;
                let DeclarationKind::Object { witnesses, .. } = &declaration.kind else {
                    continue;
                };
                for witness in witnesses {
                    if matches!(module.types.get(witness.role), Ok(Type::Role { decl, .. }) if decl == role)
                        && let Some(function) = witness.slots.get(*slot as usize) {
                        targets.insert(function.as_u32() as usize);
                    }
                }
            }
        }
        Operation::Call(CallOp::Closure { callee, .. }) => {
            // Known closure/bound origins receive fixed adapter equations.
            // An open environment requires its registered catalog contract.
            let value = body.values.get(callee.value).map_err(arena)?;
            if let Definition::InstResult { inst, .. } = value.def
                && let Operation::Core(CoreOp::FnClosure { function, .. } | CoreOp::FnBind { function, .. })
                    = &body.insts.get(inst).map_err(arena)?.op {
                targets.insert(function.as_u32() as usize);
            }
        }
        _ => {}
    }
    Ok(targets.into_iter().collect())
}

impl Universe {
    pub fn summary(&self, function: FuncId) -> &Summary {
        &self.nodes[function.as_u32() as usize].summary
    }
    pub fn call_summary(&self, function: FuncId, site: Site) -> Option<&Summary> {
        self.calls.get(&(function, site)).map(|node| &self.nodes[*node].summary)
    }
    pub fn protocol_summary(&self, protocol: ProtocolId) -> &Summary {
        &self.nodes[self.protocol_services[&protocol]].summary
    }
    pub fn service_summary(&self, ty: TypeId) -> Option<&Summary> {
        self.services.get(&ty).map(|node| &self.nodes[*node].summary)
    }
    pub fn build(
        module: &Module,
        types: &BTreeMap<TypeId, TypeClassification>,
        mut external: BTreeMap<FuncId, Summary>,
        natives: &dyn NativeSummaries,
        budget: &mut Budget<'_>,
    ) -> Result<Self, OwnError> {
        let inventories = scopes::inventory(module, types, &external, budget)?;
        let mut universe = Self {
            nodes: vec![], calls: BTreeMap::new(), strides: BTreeMap::new(), inventories,
            services: BTreeMap::new(), protocol_services: BTreeMap::new(), descriptors: BTreeMap::new(),
        };
        let result_bound = module.functions.iter().map(|(_, function)| function.signature.results.len())
            .max().unwrap_or(0).saturating_add(16);
        for (function, data) in module.functions.iter() {
            budget.work(1)?;
            let summary = external.remove(&function).unwrap_or_else(|| bottom(module, data));
            let equation = if data.body.is_some() { Equation::Body(function) } else { Equation::External };
            budget.memory(normal::size(&summary) + 256)?;
            universe.nodes.push(Node { equation, summary, dependencies: BTreeSet::new() });
            if let Some(body) = &data.body {
                let slots = body.values.len().saturating_add(result_bound);
                universe.strides.insert(function, slots);
                let sites = body.insts.len().saturating_add(body.blocks.len()).saturating_add(1);
                let base = body.keys.iter().filter_map(|(_, key)| match key {
                    KeyExpr::Created { site, .. } => Some(site.as_u32() as usize + 1),
                    _ => None,
                }).max().unwrap_or(0);
                if base.saturating_add(slots.saturating_mul(sites)) > u32::MAX as usize {
                    return Err(OwnError::Incomplete { stats: Box::new(budget.stats.clone()) });
                }
            }
        }
        // Keys include targets, substitution, public ABI, closure kind and
        // dictionary contracts. They never depend on a solver iteration.
        let mut adapters = BTreeMap::new();
        // Freeze all call/role/protocol equations before the first SCC walk.
        for (function, data) in module.functions.iter() {
            let Some(body) = &data.body else {
                continue;
            };
            budget.memory((body.insts.len() + body.blocks.len()) * 32)?;
            for (site, op) in operations(body) {
                budget.work(1)?;
                let Some((call, receiver)) = call(op) else {
                    continue;
                };
                let targets = targets(module, body, op, budget)?;
                let direct = matches!(op, Operation::Call(CallOp::Direct { .. } | CallOp::Native { .. }
                    | CallOp::Intrinsic { .. } | CallOp::Resume { .. } | CallOp::Cap { .. }));
                let captures = match op {
                    Operation::Call(CallOp::Closure { callee, .. }) => {
                        match body.values.get(callee.value).map_err(arena)?.def {
                            Definition::InstResult { inst, .. } => match &body.insts.get(inst).map_err(arena)?.op {
                                Operation::Core(CoreOp::FnClosure { captures, .. }) => Some(captures.len()),
                                Operation::Core(CoreOp::FnBind { .. }) => Some(1),
                                _ => None,
                            },
                            _ => None,
                        }
                    }
                    _ => None,
                };
                let kind = match op {
                    Operation::Call(CallOp::Role { .. }) => 0,
                    Operation::Call(CallOp::Closure { callee, .. }) => {
                        match body.values.get(callee.value).map_err(arena)?.def {
                            Definition::InstResult { inst, .. } => match &body.insts.get(inst).map_err(arena)?.op {
                                Operation::Core(CoreOp::FnClosure { .. }) => 1,
                                Operation::Core(CoreOp::FnBind { .. }) => 2,
                                Operation::Core(CoreOp::FnBindRole { .. }) => 3,
                                _ => 4,
                            },
                            _ => 4,
                        }
                    }
                    _ => 5,
                };
                let public_types: Vec<_> = receiver.iter().chain(&call.args).map(|arg| {
                    body.values.get(arg.value).map(|value| value.ty).map_err(arena)
                }).collect::<Result<_, _>>()?;
                let dictionaries: Vec<_> = call.descriptors.iter().map(|value| {
                    body.descriptor_bindings.iter().find(|binding| binding.dictionary == *value)
                        .map(|binding| format!("{:?}", binding.contract))
                }).collect();
                let key = (kind, targets.clone(), captures, call.type_args.clone(), public_types, dictionaries);
                let node = if direct && targets.len() == 1 {
                    targets[0]
                } else if let Some(node) = adapters.get(&key) {
                    *node
                } else if !targets.is_empty() {
                    let summary = adapt(&universe.nodes[targets[0]].summary, captures, budget)?;
                    let dependencies = targets.iter().copied().collect();
                    let node = universe.nodes.len();
                    budget.memory(256 + normal::size(&summary) + targets.len() * 64)?;
                    let equation = Equation::Dispatch { targets, captures };
                    universe.nodes.push(Node { equation, summary, dependencies });
                    adapters.insert(key, node);
                    budget.stats.adapter_equations += 1;
                    node
                } else {
                    let receiver = receiver.ok_or_else(|| budget.fail(
                        OwnDiagnosticKind::MissingBoundary, format!("no targets at {function:?} {site:?}"),
                    ))?;
                    let ty = body.values.get(receiver.value).map_err(arena)?.ty;
                    let substitution = Substitution {
                        types: call.type_args.clone(),
                        descriptors: body.descriptor_bindings.iter().map(|binding| binding.contract.clone()).collect(),
                    };
                    let mut summary = natives.lookup(ExternalBoundary::Callable(ty), &substitution).map_err(|error| {
                        let message = format!("{function:?} {site:?}: {}", error.message);
                        budget.fail(OwnDiagnosticKind::MissingBoundary, message)
                    })?.summary;
                    normal::normalize(&mut summary, budget)?;
                    let node = universe.nodes.len();
                    budget.memory(256 + normal::size(&summary))?;
                    universe.nodes.push(Node { equation: Equation::External, summary, dependencies: BTreeSet::new() });
                    adapters.insert(key, node);
                    budget.stats.adapter_equations += 1;
                    node
                };
                universe.calls.insert((function, site), node);
                universe.nodes[function.as_u32() as usize].dependencies.insert(node);
            }
            for (block, data) in body.blocks.iter() {
                if let TermKind::InvokeDefer { function: target, .. } = &data.term.kind {
                    let node = target.as_u32() as usize;
                    universe.calls.insert((function, Site::Terminator(block)), node);
                    universe.nodes[function.as_u32() as usize].dependencies.insert(node);
                }
            }
        }
        for facts in universe.inventories.values() {
            for protocol in &facts.service_protocols {
                if universe.protocol_services.contains_key(protocol) {
                    continue;
                }
                let boundary = ExternalBoundary::ServiceProtocol(*protocol);
                let mut summary = natives.lookup(boundary, &Substitution::default())
                    .map_err(|error| budget.fail(OwnDiagnosticKind::MissingBoundary, error.message))?.summary;
                normal::normalize(&mut summary, budget)?;
                let node = universe.nodes.len();
                budget.memory(256 + normal::size(&summary))?;
                universe.nodes.push(Node { equation: Equation::External, summary, dependencies: BTreeSet::new() });
                universe.protocol_services.insert(*protocol, node);
            }
            for ty in &facts.potential_owned_types {
                let classification = &types[ty];
                if !classification.unresolved.is_empty() {
                    return Err(budget.fail(OwnDiagnosticKind::MissingDescriptor, format!(
                        "owned type {ty:?} reaches unbound descriptors {:?}", classification.unresolved,
                    )));
                }
                universe.descriptors.insert(*ty, classification.descriptors.clone());
                for service in &classification.service_types {
                    if universe.services.contains_key(service) {
                        continue;
                    }
                    let mut summary = natives.lookup(ExternalBoundary::Service(*service), &Substitution::default())
                        .map_err(|error| budget.fail(OwnDiagnosticKind::MissingBoundary, error.message))?.summary;
                    normal::normalize(&mut summary, budget)?;
                    let node = universe.nodes.len();
                    budget.memory(256 + normal::size(&summary))?;
                    universe.nodes.push(Node { equation: Equation::External, summary, dependencies: BTreeSet::new() });
                    universe.services.insert(*service, node);
                }
            }
        }
        for (scope, facts) in &universe.inventories {
            let function = module.scopes.get(*scope).map_err(arena)?.function;
            let node = &mut universe.nodes[function.as_u32() as usize];
            for ty in &facts.potential_owned_types {
                for destroy in &types[ty].destroy_entries {
                    budget.work(1)?;
                    budget.memory(64)?;
                    if node.dependencies.insert(destroy.as_u32() as usize) {
                        budget.stats.implicit_destroy_edges += 1;
                    }
                }
                for service in &types[ty].service_types {
                    node.dependencies.insert(universe.services[service]);
                }
            }
            for defer in &facts.defers {
                node.dependencies.insert(defer.as_u32() as usize);
            }
            for protocol in &facts.service_protocols {
                node.dependencies.insert(universe.protocol_services[protocol]);
            }
        }
        budget.stats.functions = module.functions.len();
        budget.stats.universe_nodes = universe.nodes.len();
        universe.check_representations(module, budget)?;
        Ok(universe)
    }

    fn check_representations(&self, module: &Module, budget: &mut Budget<'_>) -> Result<(), OwnError> {
        let mut diagnostics = vec![];
        for ((function, site), index) in &self.calls {
            let Equation::Dispatch { targets, .. } = &self.nodes[*index].equation else {
                continue;
            };
            let first = &self.nodes[targets[0]].summary.contract;
            budget.work(targets.len())?;
            let publication = targets.iter().any(|target| {
                self.nodes[*target].summary.contract.publication != first.publication
            });
            let protocol = targets.iter().any(|target| self.nodes[*target].summary.contract.protocol != first.protocol);
            let contracts: Vec<_> = targets.iter().map(|target| {
                let name = module.functions.get(FuncId::from_u32(*target as u32))
                    .map_or("adapter", |function| function.symbol.as_str());
                format!("{name}: {:?}", self.nodes[*target].summary.contract)
            }).collect();
            for (conflict, kind) in [(publication, OwnDiagnosticKind::IncompatiblePublicationJoin),
                (protocol, OwnDiagnosticKind::IncompatibleProtocolJoin)] {
                if !conflict {
                    continue;
                }
                if diagnostics.len() >= budget.options.verify.limits.diagnostics {
                    return Err(OwnError::Incomplete { stats: Box::new(budget.stats.clone()) });
                }
                match kind {
                    OwnDiagnosticKind::IncompatiblePublicationJoin => budget.stats.incompatible_publication_joins += 1,
                    _ => budget.stats.incompatible_protocol_joins += 1,
                }
                budget.memory(512 + contracts.iter().map(String::len).sum::<usize>())?;
                diagnostics.push(OwnDiagnostic {
                    kind,
                    function: Some(*function),
                    site: Some(site.high()),
                    message: format!("call in {} at {site:?} has conflicting contracts: {}",
                        module.functions.get(*function).map_err(arena)?.symbol, contracts.join("; ")),
                });
            }
        }
        if !diagnostics.is_empty() {
            return Err(OwnError::Diagnostics { diagnostics, stats: Box::new(budget.stats.clone()) });
        }
        Ok(())
    }

    pub fn solve(
        &mut self,
        module: &Module,
        types: &BTreeMap<TypeId, TypeClassification>,
        budget: &mut Budget<'_>,
    ) -> Result<(), OwnError> {
        budget.memory(self.nodes.len() * 64
            + self.nodes.iter().map(|node| node.dependencies.len() * 16).sum::<usize>())?;
        let graph: Vec<_> = self.nodes.iter().map(|node| node.dependencies.iter().copied().collect()).collect();
        let components = components(&graph, budget)?;
        budget.stats.sccs = components.len();
        // With A canonical selections, C creation sites, P parameters and R
        // results, each node has at most P*(A+P+2) + R*(C+A+P+1) + 7
        // strict lattice increases. A is finite (field paths of depth <= 4).
        // One nonfinal sweep makes an increase, so sum(height)+1 bounds SCC
        // sweeps. Work/memory limits may stop earlier, always without output.
        for component in components.into_iter().rev() {
            if component.len() > 1 || graph[component[0]].contains(&component[0]) {
                budget.stats.cyclic_sccs += 1;
            }
            loop {
                let mut changed = false;
                for &index in &component {
                    budget.work(1)?;
                    budget.stats.equation_evaluations += 1;
                    let inferred = match &self.nodes[index].equation {
                        Equation::External => continue,
                        Equation::Body(function) => self.infer_body(module, *function, types, budget)?,
                        Equation::Dispatch { targets, captures } => {
                            let mut summary = adapt(&self.nodes[targets[0]].summary, *captures, budget)?;
                            for target in &targets[1..] {
                                let source = adapt(&self.nodes[*target].summary, *captures, budget)?;
                                if let Err(mut error) = normal::join(&mut summary, &source, budget) {
                                    self.locate(index, &mut error, module);
                                    return Err(error);
                                }
                            }
                            summary
                        }
                    };
                    let mut summary = self.nodes[index].summary.clone();
                    normal::join(&mut summary, &inferred, budget)?;
                    normal::normalize(&mut summary, budget)?;
                    if summary != self.nodes[index].summary {
                        budget.memory(normal::size(&summary))?;
                        self.nodes[index].summary = summary;
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
        }
        debug_assert_eq!(budget.stats.universe_nodes, self.nodes.len());
        Ok(())
    }

    fn locate(&self, node: usize, error: &mut OwnError, module: &Module) {
        if let OwnError::Diagnostics { diagnostics, .. } = error
            && let Some(((function, site), _)) = self.calls.iter().find(|(_, target)| **target == node) {
            for diagnostic in diagnostics {
                diagnostic.function = Some(*function);
                diagnostic.site = Some(site.high());
                diagnostic.message = format!("call in {} at {site:?}: {}",
                    module.functions.get(*function).map_or("?", |function| function.symbol.as_str()),
                    diagnostic.message);
            }
        }
    }

    pub fn cleanup(&self, module: &Module, facts: &TypeClassification) -> ScopeEffectEnvelope {
        let mut effects = ScopeEffectEnvelope::default();
        for destroy in &facts.destroy_entries {
            project(&mut effects, self.summary(*destroy));
        }
        for descriptor in &facts.descriptors {
            if descriptor.managed != DescriptorManaged::Plain {
                if let Ok(summary) = module.summaries.get(descriptor.summary) {
                    project(&mut effects, summary);
                }
                effects.suspends |= descriptor.async_destroy;
            }
        }
        for service in &facts.service_types {
            if let Some(summary) = self.service_summary(*service) {
                project(&mut effects, summary);
            }
        }
        effects
    }
}

pub(super) fn project(effects: &mut ScopeEffectEnvelope, summary: &Summary) {
    let failures: Vec<_> = summary.contract.failures.iter().copied()
        .filter(|failure| *failure != FailureKind::Cancelled).collect();
    normal::failures(&mut effects.failures, &failures);
    effects.suspends |= summary.contract.suspends || summary.contract.async_destroy;
}

fn components(graph: &[Vec<usize>], budget: &mut Budget<'_>) -> Result<Vec<Vec<usize>>, OwnError> {
    budget.memory(graph.len() * 128 + graph.iter().map(|edges| edges.len() * 32).sum::<usize>())?;
    let mut reverse = vec![vec![]; graph.len()];
    for (source, edges) in graph.iter().enumerate() {
        for &target in edges {
            reverse[target].push(source);
        }
    }
    let mut visited = vec![false; graph.len()];
    let mut order = vec![];
    for root in 0..graph.len() {
        if visited[root] {
            continue;
        }
        let mut pending = vec![(root, false)];
        while let Some((node, leaving)) = pending.pop() {
            budget.work(1)?;
            if leaving {
                order.push(node);
            } else if !visited[node] {
                visited[node] = true;
                pending.push((node, true));
                pending.extend(graph[node].iter().rev().map(|target| (*target, false)));
            }
        }
    }
    visited.fill(false);
    let mut result = vec![];
    for root in order.into_iter().rev() {
        if visited[root] {
            continue;
        }
        let mut component = vec![];
        let mut pending = vec![root];
        visited[root] = true;
        while let Some(node) = pending.pop() {
            budget.work(1)?;
            component.push(node);
            for &target in &reverse[node] {
                if !visited[target] {
                    visited[target] = true;
                    pending.push(target);
                }
            }
        }
        component.sort();
        result.push(component);
    }
    Ok(result)
}

