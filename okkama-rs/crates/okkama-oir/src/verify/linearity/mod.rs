//! Bounded relational worklist. Diagnostics are emitted only by replaying the
//! fixed point; solver visits never turn a temporary absence into a rejection.
mod domain;
mod metadata;
mod packets;
mod scopes;
mod summaries;
mod cleanup;
mod async_rules;
mod defer;
mod storage;
mod descriptors;

use super::{EntityLocation as L, RuleId as R, State};
use crate::*;
use domain::*;
use metadata::*;
use std::collections::{BTreeSet, VecDeque};

struct Engine<'m, 's> {
    module: &'m Module,
    function: FuncId,
    declaration: &'m Function,
    body: &'m Body,
    state: &'s mut State<'m>,
    emit: bool,
    reported: BTreeSet<(L, R)>,
    managed: &'s [bool],
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Slot {
    token: Token,
    journal: Token,
    refs: BTreeSet<Origin>,
    identity: Option<Origin>,
    aliases: BTreeSet<ValueId>,
    contained: BTreeSet<ValueId>,
    bases: BTreeSet<ValueId>,
    requirements: BTreeSet<Origin>,
    lifetime: u8,
    must_use: u8,
    failures: u8,
    redefine: bool,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            token: Token::default(), journal: Token::default(), refs: BTreeSet::new(),
            identity: None, aliases: BTreeSet::new(),
            requirements: BTreeSet::new(), lifetime: 1, must_use: 0, failures: 0,
            contained: BTreeSet::new(), bases: BTreeSet::new(), redefine: true,
        }
    }
}

impl Slot {
    fn rename(&mut self, old: Origin, new: Origin) {
        let rename = |key| if key == old { new } else { key };
        self.refs = self.refs.iter().copied().map(rename).collect();
        self.requirements = self.requirements.iter().copied().map(rename).collect();
        self.identity = self.identity.map(rename);
        for token in [&mut self.token, &mut self.journal] {
            token.pieces = token.pieces.iter().cloned().map(|mut piece| {
                piece.origin = rename(piece.origin);
                piece.predicates = piece.predicates.into_iter().map(|(mut filter, selected)| {
                    filter.keys = filter.keys.into_iter().map(rename).collect();
                    filter.possible = filter.possible.into_iter().map(rename).collect();
                    (filter, selected)
                }).collect();
                piece
            }).collect();
        }
    }
}

pub(super) fn check<'m>(module: &'m Module, selected: Option<FuncId>, state: &mut State<'m>) {
    let managed = managed_types(module, state);
    for (id, declaration) in module.declarations.iter() {
        if let DeclarationKind::Object { fields, .. } = &declaration.kind {
            for field in fields {
                if field.once && managed.get(field.ty.as_u32() as usize).copied().unwrap_or(true) {
                    state.at(L::Declaration(id), declaration.loc);
                    state.error(R::B111, "once storage must be plain over its complete target/type graph");
                }
            }
        }
    }
    for (function, declaration) in module.functions.iter() {
        if selected.is_some_and(|id| id != function) {
            continue;
        }
        let Some(body) = &declaration.body else {
            continue;
        };
        if !super::graph::eligible(function, body, state) {
            continue;
        }
        if module.profile != Profile::High {
            value_facts(module, function, body, &managed, state);
        }
        if !state.array(body.values.len(), 64) {
            return;
        }
        let mut engine = Engine {
            module, function, declaration, body, state, emit: false,
            reported: BTreeSet::new(),
            managed: &managed,
        };
        engine.solve();
    }
}

impl Engine<'_, '_> {
    fn error(&mut self, rule: R, message: &str) {
        if self.emit && self.reported.insert((self.state.location.clone(), rule)) {
            self.state.error(rule, message);
        }
    }
    fn charge_flow(&mut self, flow: &Flow) -> bool {
        let tokens = flow.tokens.iter().chain(&flow.journals)
            .chain(flow.frames.values().flat_map(|layout| layout.values().map(|slot| &slot.token)))
            .chain(flow.cells.values().map(|slot| &slot.token));
        let pieces = tokens.flat_map(|token| &token.pieces).chain(&flow.closed_inputs);
        let mut entries = 0usize;
        for piece in pieces {
            entries = entries.saturating_add(1);
            for filter in piece.predicates.keys() {
                entries = entries.saturating_add(1 + filter.keys.len() + filter.possible.len() + filter.labels.len());
            }
        }
        for refs in flow.refs.iter().chain(&flow.full_deps) {
            entries = entries.saturating_add(refs.len());
        }
        for labels in flow.aliases.iter().chain(&flow.contained) {
            entries = entries.saturating_add(labels.len());
        }
        for refs in flow.views.values() {
            entries = entries.saturating_add(1 + refs.len());
        }
        for (refs, _) in flow.fields.values() {
            entries = entries.saturating_add(1 + refs.len());
        }
        for token in &flow.tokens {
            entries = entries.saturating_add(token.unique_region.len() + token.pin_deps.len());
            if let Some(fact) = &token.cleanup {
                entries = entries.saturating_add(fact.reserve.len() + fact.inventory.len()
                    + fact.services.len() + fact.executed.len());
            }
        }
        entries = entries.saturating_add(flow.dead.len() + flow.bindings.len() + flow.provenance.len());
        entries = entries.saturating_add(flow.frames.values().map(|slots| slots.len()).sum::<usize>());
        self.state.step(entries.saturating_add(flow.availability.len()))
            && self.state.array(flow.availability.len(), 1536) && self.state.array(entries, 128)
    }

    fn initial(&mut self) -> Flow {
        let mut flow = Flow::new(self.body.values.len());
        for (value, data) in self.body.values.iter() {
            flow.witnesses[value.as_u32() as usize] = self.module.types.get(data.ty).is_ok_and(|ty| {
                !matches!(ty, Type::Service(_) | Type::Descriptor | Type::Frame(_)
                    | Type::Builder(_) | Type::MapBuilder(..))
            });
        }
        let defer_entry = self.declaration.attrs.iter().any(|attr| matches!(attr, FunctionAttr::DeferEntry { .. }));
        let call_boundary = defer_entry || self.declaration.summary.and_then(|id| self.module.summaries.get(id).ok())
            .is_some_and(|s| s.entry_kind == EntryKind::CallBoundary);
        let input = self.declaration.signature.params.iter().any(|ty| {
            self.module.types.get(*ty) == Ok(&Type::Service(ServiceType::Own))
        });
        flow.entry_accepted = !matches!(self.declaration.signature.ownership_abi, OwnershipAbi::Explicit { .. })
            && !(call_boundary && input);
        let Ok(entry) = self.body.blocks.get(self.body.entry) else {
            return flow;
        };
        for &value in &entry.params {
            let i = value.as_u32() as usize;
            flow.availability[i] = AVAILABLE;
            if self.body.descriptor_bindings.iter().any(|binding| binding.dictionary == value) {
                flow.tokens[i].dictionary = Some(DictionarySource::Parameter(value));
            }
            flow.aliases[i].insert(value);
            flow.contained[i].insert(value);
            if linear(self.module, self.body, value) {
                let mut token = Token { scope: scope(self.module, self.body, value), ..Token::default() };
                token.sources.insert(value);
                if matches!(super::later::value_type(self.module, self.body, value),
                    Some(Type::Frame(_) | Type::Service(ServiceType::TaskState | ServiceType::Receipt
                        | ServiceType::CacheState | ServiceType::Pin))) {
                    token.service = Some(Origin::Input(value));
                }
                let permission = matches!(super::later::value_type(self.module, self.body, value),
                    Some(Type::Service(ServiceType::UniqueCache | ServiceType::Pin | ServiceType::Receipt
                        | ServiceType::TaskState | ServiceType::CleanupState | ServiceType::ExitState)));
                if !builder(self.module, self.body, value) && !permission {
                    let mut piece = Piece::new(Origin::Input(value), AdmissionProvenance::UnboundResult);
                    if !call_boundary || token.scope.is_some() {
                        piece.provenance = 63;
                    }
                    piece.owner = token.scope;
                    piece.family = true;
                    token.pieces.insert(piece);
                }
                if let Some(scope) = token.scope {
                    flow.scopes.insert(scope, 1);
                }
                flow.tokens[i] = token;
            } else if self.is_managed(value) || matches!(super::later::value_type(self.module, self.body, value),
                Some(Type::Service(ServiceType::StableKey))) {
                flow.refs[i].insert(Origin::External(value));
                flow.identities[i] = Some(Origin::External(value));
                flow.full_deps[i].insert(Origin::External(value));
                flow.provenance.insert(Origin::External(value), provenance_bit(AdmissionProvenance::Bound));
            }
            if matches!(super::later::value_type(self.module, self.body, value), Some(Type::Future(_))) {
                flow.cache_keys.insert(Origin::External(value));
            }
            if successor(self.module, self.body, value) {
                flow.must_use[i] = 1;
            }
            if self.body.values.get(value).ok().and_then(|v| self.module.types.get(v.ty).ok())
                == Some(&Type::Service(ServiceType::Failure)) {
                flow.failures[i] = 31;
                flow.tokens[i].failure_origin = Some(Origin::Input(value));
            }
        }
        // A paired deps parameter is an ABI precondition, checked at direct
        // calls below. Its symbolic entry graph is the argument's full graph.
        for &value in &entry.params {
            let Ok(ValueData { facts: ValueFacts::Managed { deps }, .. }) = self.body.values.get(value) else {
                continue;
            };
            if let Ok(DepExpr::FromSsa(parameter)) = self.body.deps.get(*deps)
                && entry.params.contains(parameter) {
                let required = flow.full_deps[value.as_u32() as usize].clone();
                flow.refs[parameter.as_u32() as usize].extend(&required);
                flow.full_deps[parameter.as_u32() as usize].extend(required);
            }
        }
        if call_boundary {
            let keys: BTreeSet<_> = entry.params.iter().flat_map(|v| {
                flow.refs[v.as_u32() as usize].iter().copied()
            }).collect();
            if keys.is_empty() {
                flow.entry_accepted = true;
            }
            let input_labels: BTreeSet<_> = entry.params.iter().copied().filter(|value| {
                flow.witnesses[value.as_u32() as usize] && !flow.refs[value.as_u32() as usize].is_empty()
            }).collect();
            for &value in &entry.params {
                let token = &mut flow.tokens[value.as_u32() as usize];
                if self.body.values.get(value).ok().and_then(|v| self.module.types.get(v.ty).ok())
                    != Some(&Type::Service(ServiceType::Own)) { continue; }
                if keys.is_empty() {
                    token.pieces.clear();
                }
                else {
                    token.pieces = token.pieces.iter().cloned().map(|mut piece| {
                        piece.predicates.insert(Filter {
                            keys: keys.clone(), fresh: false, possible: BTreeSet::new(),
                            labels: input_labels.clone(), stale: false, whole_future: false, state: 0,
                        }, true);
                        piece
                    }).collect();
                }
            }
        }
        for &value in &entry.params {
            self.state.at(L::Value { function: self.function, value }, None);
            let expected = flow.full_deps[value.as_u32() as usize].clone();
            self.check_deps(&mut flow, value, &expected);
        }
        flow
    }
    fn is_managed(&self, value: ValueId) -> bool {
        self.body.values.get(value).is_ok_and(|v| !matches!(v.facts, ValueFacts::Plain {
            certificate: PlainCertificate::Descriptor(_),
        }) && (matches!(v.facts, ValueFacts::Managed { .. })
            || self.managed.get(v.ty.as_u32() as usize).copied().unwrap_or(true)))
    }
    fn solve(&mut self) {
        if !self.state.array(self.body.blocks.len(), 64) {
            return;
        }
        let mut incoming: Vec<Vec<Flow>> = vec![Vec::new(); self.body.blocks.len()];
        if !self.state.array(self.body.values.len(), 320) {
            return;
        }
        let initial = self.initial();
        if !self.charge_flow(&initial) {
            return;
        }
        incoming[self.body.entry.as_u32() as usize].push(initial);
        let mut queue = VecDeque::from([self.body.entry]);
        let mut queued = vec![false; self.body.blocks.len()];
        queued[self.body.entry.as_u32() as usize] = true;
        while let Some(block) = queue.pop_front() {
            let index = block.as_u32() as usize;
            queued[index] = false;
            if !self.state.step(1) {
                return;
            }
            let inputs = incoming[index].clone();
            for flow in inputs {
                if !self.charge_flow(&flow) {
                    return;
                }
                for (target, flow) in self.block(block, flow) {
                    if !self.charge_flow(&flow) {
                        return;
                    }
                    let index = target.as_u32() as usize;
                    let Some(states) = incoming.get_mut(index) else {
                        continue;
                    };
                    if !self.state.step(states.len().saturating_mul(self.body.values.len())) {
                        return;
                    }
                    let changed = if let Some(old) = states.iter_mut().find(|old| old.same_relations(&flow)) {
                        old.join_masks(&flow)
                    } else {
                        states.push(flow);
                        true
                    };
                    if changed && !queued[index] {
                        queued[index] = true;
                        queue.push_back(target);
                    }
                }
            }
        }
        self.emit = true;
        self.initial();
        for (block, _) in self.body.blocks.iter() {
            for flow in &incoming[block.as_u32() as usize] {
                if !self.charge_flow(flow) {
                    return;
                }
                self.block(block, flow.clone());
            }
        }
    }
    fn use_value(&mut self, flow: &mut Flow, value: ValueId, mode: OperandMode, rule: R) {
        let index = value.as_u32() as usize;
        if flow.tokens[index].pin_deps.iter().any(|key| flow.dead.contains(key)) {
            self.error(R::B96, "retained view pin outlives its stable owner");
        }
        if flow.tokens[index].unique.is_some_and(|key| flow.dead.contains(&key)) {
            self.error(R::B92, "unique-cache capability outlives the cache owner");
        }
        let reservation = self.body.values.get(value).ok().and_then(|v| self.module.types.get(v.ty).ok())
            == Some(&Type::Service(ServiceType::Pending));
        if !reservation {
            self.live_use(flow, value);
        }
        if linear(self.module, self.body, value) {
            if flow.availability[index] != AVAILABLE {
                let pending = self.body.values.get(value).ok().and_then(|v| self.module.types.get(v.ty).ok())
                    == Some(&Type::Service(ServiceType::Pending));
                let rule = if pending { R::B101 }
                    else if builder(self.module, self.body, value) { R::B81 } else { rule };
                self.error(rule, "linear carrier is not available on every reaching path");
            }
            if matches!(mode, OperandMode::Consume | OperandMode::Forward) {
                flow.retire(value);
            }
        }
    }
    fn define(&mut self, flow: &mut Flow, value: ValueId, mut slot: Slot) {
        let i = value.as_u32() as usize;
        if slot.redefine && flow.availability[i] != ABSENT {
            for key in flow.projection_keys() {
                let Origin::Projection(base, kind) = key else {
                    continue;
                };
                let changes = base == value || match kind {
                    ProjectionKind::Index(index) | ProjectionKind::Map(index) | ProjectionKind::Remove(index)
                    | ProjectionKind::MapIndex(index, _) => {
                        index == value
                    }
                    ProjectionKind::Slice(start, end) => start == value || end == value,
                    _ => false,
                };
                if changes {
                    flow.retire_origin(key);
                    slot.rename(key, Origin::RetiredProjection(base, kind));
                }
            }
        }
        let core = linear(self.module, self.body, value);
            if core && flow.availability[i] & AVAILABLE != 0 {
            self.error(if builder(self.module, self.body, value) { R::B81 } else { R::B22 },
                "new incarnation overwrites an available linear carrier");
        }
        if successor(self.module, self.body, value) && flow.must_use[i] == 1 {
            self.error(R::B82, "successor incarnation overwritten before a permitted use");
        }
        if slot.aliases != flow.aliases[i] || !slot.aliases.contains(&value) {
            let old = flow.aliases[i].clone();
            let retained: BTreeSet<_> = old.iter().copied().filter(|v| *v != value).collect();
            flow.replace_aliases(&old, &retained);
        }
        let source = slot.aliases.clone();
        let aliases: BTreeSet<_> = source.iter().copied().chain(std::iter::once(value)).collect();
        flow.replace_aliases(&source, &aliases);
        flow.aliases[i] = aliases.clone();
        flow.contained[i] = slot.contained.union(&aliases).copied().collect();
        for base in &slot.bases {
            let roots = flow.aliases[base.as_u32() as usize].clone();
            flow.add_contained(&roots, &aliases);
        }
        flow.availability[i] = AVAILABLE;
        flow.tokens[i] = flow.normalize_token(slot.token);
        flow.journals[i] = flow.normalize_token(slot.journal);
        let service_deps = self.body.values.get(value).ok().and_then(|v| self.module.types.get(v.ty).ok())
            .is_some_and(|ty| matches!(ty, Type::Service(ServiceType::Deps | ServiceType::Pending
                | ServiceType::StableKey
                | ServiceType::CleanupState | ServiceType::ExitState | ServiceType::OwnerSnapshot)));
        flow.refs[i] = if self.is_managed(value) || service_deps { slot.refs }
            else { BTreeSet::new() };
        flow.full_deps[i] = flow.refs[i].union(&slot.requirements).copied().collect();
        flow.lifetimes[i] = slot.lifetime;
        flow.failures[i] = slot.failures;
        flow.identities[i] = slot.identity;
        flow.must_use[i] = if successor(self.module, self.body, value) && slot.must_use == 0 { 1 }
            else { slot.must_use };
    }
    fn block(&mut self, id: BlockId, mut flow: Flow) -> Vec<(BlockId, Flow)> {
        let Ok(block) = self.body.blocks.get(id) else {
            return Vec::new();
        };
        let mut outgoing = Vec::new();
        for &instruction in &block.insts {
            let Ok(inst) = self.body.insts.get(instruction) else {
                continue;
            };
            self.state.at(L::Instruction { function: self.function, instruction }, inst.loc);
            let Ok(contract) = ContractRegistry::new(self.module).construction_schema(&inst.op, self.body) else {
                continue;
            };
            for outcome in [Outcome::Unwind, Outcome::Cancel] {
                if let Some(schema) = contract.outcome(outcome) {
                    let mut failed = flow.clone();
                    if self.module.profile != Profile::High && schema.results.iter().any(|slot| {
                        matches!(slot.ty, ResultType::Service(service) if service.is_linear())
                            || matches!(slot.ty, ResultType::SameAs(v) if linear(self.module, self.body, v))
                    }) {
                        self.error(R::B63, "exceptional linear results require an invoke.own edge");
                    }
                    self.operation(id, &inst.op, schema, outcome,
                        Origin::Instruction(instruction, 0), &mut failed);
                    if let Some(unwind) = &block.unwind {
                        outgoing.push(self.edge(&failed, unwind.target.block,
                            &unwind.target.args.iter().copied().map(EdgeArg::Value).collect::<Vec<_>>(), &[], R::B63));
                    } else {
                        self.finish(&mut failed, false, &[]);
                    }
                }
            }
            let slots = self.operation(id, &inst.op, &contract.normal, Outcome::Normal,
                Origin::Instruction(instruction, 0), &mut flow);
            for (value, slot) in inst.results.iter().copied().zip(slots) {
                self.define(&mut flow, value, slot);
                let expected = flow.full_deps[value.as_u32() as usize].clone();
                self.check_deps(&mut flow, value, &expected);
            }
            self.facts(&flow, &inst.op, &inst.facts, &inst.results);
        }
        self.state.at(L::Terminator { function: self.function, block: id }, block.term.loc);
        match &block.term.kind {
            TermKind::InvokeOwn { op, normal, unwind } => {
                if let Ok(contract) = ContractRegistry::new(self.module).construction_schema(op, self.body) {
                    for (outcome, edge) in [(Outcome::Normal, normal), (Outcome::Unwind, unwind),
                        (Outcome::Cancel, unwind)] {
                        if let Some(schema) = contract.outcome(outcome) {
                            let mut branch = flow.clone();
                            let slots = self.operation(id, op, schema, outcome, Origin::Invoke(id, 0), &mut branch);
                            let rule = if outcome != Outcome::Normal && matches!(**op, Operation::Call(_)) {
                                R::B54
                            } else { R::B21 };
                            outgoing.push(self.edge(&branch, edge.block, &edge.args, &slots, rule));
                        }
                    }
                }
            }
            TermKind::Cond { condition, yes, no } if flow.tokens[condition.as_u32() as usize]
                .cleanup_test.is_some() => {
                let test = flow.tokens[condition.as_u32() as usize].cleanup_test;
                for (present, target) in [(true, yes), (false, no)] {
                    let mut branch = flow.clone();
                    let mut possible = true;
                    for (index, token) in branch.tokens.iter_mut().enumerate() {
                        if branch.availability[index] != AVAILABLE { continue; }
                        if let Some(fact) = &mut token.cleanup {
                            if fact.test != test { continue; }
                            if present { possible &= !token.pieces.is_empty(); }
                            else {
                                possible &= token.pieces.is_empty() || token.pieces.iter().any(|piece| piece.family);
                                token.pieces.clear();
                                fact.selected = None;
                            }
                        }
                    }
                    if possible {
                        let args = target.args.iter().copied().map(EdgeArg::Value).collect::<Vec<_>>();
                        outgoing.push(self.edge(&branch, target.block, &args, &[], R::B21));
                    }
                }
            }
            TermKind::StateDispatch { closed, failure, exit, arms } => {
                self.dispatch(&mut flow, *closed, *failure, *exit, arms, &mut outgoing);
            }
            TermKind::InvokeDefer { function, call, continue_edge, replacements, unwind } => {
                self.defer_invoke(id, *function, call, continue_edge, replacements, unwind.as_ref(),
                    &flow, &mut outgoing);
            }
            TermKind::RetDefer { tag, values } => {
                self.defer_return(tag, values, &flow);
                let normal = !matches!(tag, DeferReturnTag::Continue) || values.is_empty();
                self.finish(&mut flow, normal, values);
            }
            TermKind::Ret { values } => self.finish(&mut flow, true, values),
            TermKind::Throw(v) | TermKind::Raise(v) | TermKind::Panic(v) => {
                self.live_use(&flow, *v);
                if flow.active_failure != 0 && !matches!(block.term.kind, TermKind::Raise(_)) {
                    self.error(R::B104, "cleanup cannot reclassify an active failure as panic or typed throw");
                }
                if matches!(block.term.kind, TermKind::Raise(_))
                    && flow.active_failure_origin.is_some()
                    && flow.active_failure_origin != flow.tokens[v.as_u32() as usize].failure_origin {
                    self.error(R::B104, "cleanup cannot substitute a different primary Error/failure view");
                }
                flow.active_failure_origin = if matches!(block.term.kind, TermKind::Raise(_)) {
                    flow.tokens[v.as_u32() as usize].failure_origin
                } else { Some(Origin::External(*v)) };
                flow.active_failure = match &block.term.kind {
                    TermKind::Throw(_) => failure_bit(FailureKind::Thrown),
                    TermKind::Panic(_) => failure_bit(FailureKind::Panic),
                    _ => flow.failures[v.as_u32() as usize],
                };
                flow.latest_failure = flow.active_failure;
                flow.latest_failure_origin = flow.active_failure_origin;
                if let Some(unwind) = &block.unwind {
                    outgoing.push(self.edge(&flow, unwind.target.block,
                        &unwind.target.args.iter().copied().map(EdgeArg::Value).collect::<Vec<_>>(), &[], R::B63));
                } else { self.finish(&mut flow, false, &[]); }
            }
            TermKind::Trap { .. } | TermKind::Unreachable => {
                flow.active_failure = failure_bit(FailureKind::Program);
                self.finish(&mut flow, false, &[]);
            }
            TermKind::Lookup { kind, collection, key, found, missing } => {
                self.live_use(&flow, *collection);
                self.live_use(&flow, *key);
                let projection = match kind {
                    LookupKind::Array => ProjectionKind::Index(*key),
                    LookupKind::Map => ProjectionKind::Map(*key),
                };
                let view = Origin::Projection(*collection, projection);
                let mut found_flow = flow.clone();
                found_flow.views.insert(view, flow.refs[collection.as_u32() as usize].clone());
                let slot = Slot {
                    refs: BTreeSet::from([view]), bases: BTreeSet::from([*collection]),
                    lifetime: flow.lifetimes[collection.as_u32() as usize], ..Slot::default()
                };
                outgoing.push(self.edge(&found_flow, found.block, &found.args, &[slot], R::B21));
                let args = missing.args.iter().copied().map(EdgeArg::Value).collect::<Vec<_>>();
                outgoing.push(self.edge(&flow, missing.block, &args, &[], R::B21));
            }
            TermKind::Catch { failure, handler, next, .. } => {
                self.live_use(&flow, *failure);
                let mut caught = flow.clone();
                if flow.tokens[failure.as_u32() as usize].failure_origin == flow.active_failure_origin {
                    caught.active_failure = 0;
                    caught.active_failure_origin = None;
                }
                outgoing.push(self.edge(&caught, handler.block, &handler.args, &[Slot::default()], R::B21));
                let args = next.args.iter().copied().map(EdgeArg::Value).collect::<Vec<_>>();
                outgoing.push(self.edge(&flow, next.block, &args, &[], R::B21));
            }
            _ => {
                match &block.term.kind {
                    TermKind::Cond { condition, .. } | TermKind::Switch { value: condition, .. } => {
                        self.live_use(&flow, *condition);
                    }
                    _ => {}
                }
                for (target, args) in targets(&block.term.kind) {
                    outgoing.push(self.edge(&flow, target, &args, &[], R::B21));
                }
            }
        }
        outgoing
    }
    fn edge(&mut self, flow: &Flow, target: BlockId, args: &[EdgeArg], slots: &[Slot], rule: R) -> (BlockId, Flow) {
        if !self.charge_flow(flow) {
            return (target, flow.clone());
        }
        let mut next = flow.clone();
        for (index, token) in flow.tokens.iter().enumerate() {
            if flow.availability[index] == AVAILABLE && token.unique.is_some()
                && !token.unique_region.contains(&target) {
                self.error(R::B92, "unique capability leaves its proved CFG region");
            }
        }
        let mut saved = Vec::new();
        for arg in args {
            let slot = match arg {
                EdgeArg::Value(value) => {
                    let i = value.as_u32() as usize;
                    let slot = Slot {
                        token: flow.tokens[i].clone(), journal: flow.journals[i].clone(),
                        refs: flow.refs[i].clone(), must_use: flow.must_use[i],
                        failures: flow.failures[i], identity: flow.identities[i], aliases: flow.aliases[i].clone(),
                        requirements: flow.full_deps[i].clone(), lifetime: flow.lifetimes[i],
                        contained: flow.contained[i].clone(), bases: BTreeSet::new(), redefine: true,
                    };
                    self.use_value(&mut next, *value, OperandMode::Forward, rule);
                    next.must_use[i] = 0;
                    slot
                }
                EdgeArg::Produced(result) => slots.get(usize::from(result.slot)).cloned().unwrap_or_default(),
            };
            saved.push(slot);
        }
        if let Ok(block) = self.body.blocks.get(target) {
            if block.params.len() == saved.len() + 1 && self.body.values.get(block.params[saved.len()]).ok()
                .and_then(|v| self.module.types.get(v.ty).ok()) == Some(&Type::Service(ServiceType::Failure)) {
                let token = Token { failure_origin: next.latest_failure_origin, ..Token::default() };
                saved.push(Slot { token, failures: next.latest_failure, ..Slot::default() });
            }
            for (index, slot) in saved.iter_mut().enumerate() {
                slot.redefine = args.get(index) != block.params.get(index).map(|value| EdgeArg::Value(*value)).as_ref();
            }
            let replaced: BTreeSet<_> = block.params.iter().copied().collect();
            let originals: Vec<_> = saved.iter().map(|slot| slot.aliases.clone()).collect();
            let classes: BTreeSet<_> = next.aliases.iter().filter(|aliases| !aliases.is_empty()).cloned().collect();
            let mut remapped = std::collections::BTreeMap::new();
            for class in classes {
                let mut members: BTreeSet<_> = class.difference(&replaced).copied().collect();
                for (index, aliases) in originals.iter().enumerate() {
                    if aliases == &class {
                        members.insert(block.params[index]);
                    }
                }
                remapped.insert(class, members);
            }
            next.remap_aliases(&remapped);
            for (index, slot) in saved.iter_mut().enumerate() {
                if let Some(members) = remapped.get(&originals[index]) {
                    slot.aliases = members.clone();
                }
                slot.contained = slot.contained.iter().flat_map(|label| {
                    remapped.get(&flow.aliases[label.as_u32() as usize]).into_iter().flatten().copied()
                }).collect();
                slot.token.pieces = slot.token.pieces.iter().cloned().map(|mut piece| {
                    piece.predicates = piece.predicates.into_iter().map(|(mut filter, selected)| {
                        let mut labels = BTreeSet::new();
                        for label in &filter.labels {
                            let class = &flow.aliases[label.as_u32() as usize];
                            if let Some(members) = remapped.get(class) {
                                labels.extend(members);
                                filter.stale |= members.is_empty();
                            }
                        }
                        filter.labels = labels;
                        (filter, selected)
                    }).collect();
                    piece
                }).collect();
            }
            for (&param, slot) in block.params.iter().zip(saved) {
                self.define(&mut next, param, slot);
            }
            for index in 0..next.tokens.len() {
                next.tokens[index] = next.normalize_token(next.tokens[index].clone());
            }
            for &param in &block.params {
                let expected = next.full_deps[param.as_u32() as usize].clone();
                self.check_deps(&mut next, param, &expected);
            }
        }
        (target, next)
    }
    fn finish(&mut self, flow: &mut Flow, normal: bool, values: &[ValueId]) {
        if !self.declaration.attrs.iter().any(|attr| matches!(attr, FunctionAttr::DeferEntry { .. })) {
            self.body_summary(flow, normal, values);
        }
        if !flow.entry_accepted && self.module.profile != Profile::High {
            self.error(R::B54, "explicit input packet was never accepted into callee cleanup");
        }
        let exported: BTreeSet<_> = values.iter().flat_map(|v| {
            flow.tokens[v.as_u32() as usize].pieces.iter().map(|p| p.origin)
        }).collect();
        for &value in values {
            if super::later::value_type(self.module, self.body, value)
                == Some(&Type::Service(ServiceType::ExitState)) {
                self.error(R::B101, "ExitState must be eliminated by typed dispatch, not returned");
            }
            if flow.tokens[value.as_u32() as usize].unique.is_some() {
                self.error(R::B92, "unique-cache capability cannot escape its function/region");
            }
            if flow.refs[value.as_u32() as usize].iter().any(|origin| {
                matches!(origin, Origin::Instruction(..) | Origin::Invoke(..)
                    | Origin::RetiredInstruction(..) | Origin::RetiredInvoke(..)) && !exported.contains(origin)
            }) && self.module.profile != Profile::High {
                self.error(R::B32, "escaped value lacks ownership or a sufficient external lifetime boundary");
            }
            self.use_value(flow, value, OperandMode::Forward, R::B21);
            flow.must_use[value.as_u32() as usize] = 0;
        }
        for (value, _) in self.body.values.iter() {
            let i = value.as_u32() as usize;
            let core = linear(self.module, self.body, value);
            if core && flow.availability[i] & AVAILABLE != 0 {
                let rule = if scope(self.module, self.body, value).is_some() { R::B41 }
                    else if builder(self.module, self.body, value) { R::B81 }
                    else if self.body.values.get(value).ok().and_then(|v| self.module.types.get(v.ty).ok())
                        == Some(&Type::Service(ServiceType::Pending)) { R::B101 } else { R::B22 };
                self.error(rule, "function exit loses an available linear carrier");
            }
            if normal && flow.must_use[i] == 1 {
                self.error(R::B82, "normal exit leaves an unused successor");
            }
        }
        if flow.scopes.values().any(|status| *status == 1) {
            self.error(R::B41, "function exits with an open lexical scope");
        }
    }
}

fn targets(term: &TermKind) -> Vec<(BlockId, Vec<EdgeArg>)> {
    let target = |t: &Target| (t.block, t.args.iter().copied().map(EdgeArg::Value).collect());
    match term {
        TermKind::Br(t) => vec![target(t)],
        TermKind::Cond { yes, no, .. } => vec![target(yes), target(no)],
        TermKind::Switch { cases, default, .. } => cases.iter().map(|c| target(&c.target))
            .chain(std::iter::once(target(default))).collect(),
        TermKind::Catch { handler, next, .. } => vec![(handler.block, handler.args.clone()), target(next)],
        TermKind::Lookup { found, missing, .. } => vec![(found.block, found.args.clone()), target(missing)],
        _ => Vec::new(),
    }
}
