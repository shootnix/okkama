use super::*;

impl Engine<'_, '_> {
    pub(super) fn graph_filter(
        &mut self, flow: &Flow, keys: BTreeSet<Origin>, labels: BTreeSet<ValueId>, fresh: bool,
    ) -> Filter {
        let possible = self.possible(flow, &keys);
        Filter { keys, possible, labels, fresh, state: 0, stale: false }
    }
    pub(super) fn call_entry(&mut self, op: &Operation, before: &Flow, _: &mut Flow) {
        if self.module.profile == Profile::High {
            return;
        }
        let Some((call, receiver, target)) = super::super::summaries::call(op) else {
            return;
        };
        let Some(summary) = call.summary.and_then(|id| self.module.summaries.get(id).ok()) else {
            return;
        };
        let target = target.or_else(|| super::super::summaries::cap_target(self.module, op));
        self.call_deps(call, receiver, target, before);
        if let Some(function) = target.and_then(|id| self.module.functions.get(id).ok()) {
            let own = |ty: &&TypeId| self.module.types.get(**ty) == Ok(&Type::Service(ServiceType::Own));
            let inputs = function.signature.params.iter().filter(own).count();
            let outputs = function.signature.results.iter().filter(own).count();
            if inputs > 1 || outputs > 1 {
                self.error(R::B51, "ordinary call cannot transport multiple packet ABI slots");
            }
            if function.body.is_some() && inputs == 0 && call.packet.is_some_and(|packet| {
                !before.tokens[packet.as_u32() as usize].pieces.is_empty()
            }) {
                self.error(R::B54, "fresh inputs need an explicit callee packet and cleanup entry");
            }
        }
        if summary.entry_kind == EntryKind::ObservationAdmissionStaysCaller {
            if call.packet.is_some() {
                self.error(R::B52, "observation must keep admission in its caller");
            }
            return;
        }
        if summary.entry_kind == EntryKind::Internal {
            return;
        }
        let arguments: Vec<_> = receiver.iter().chain(&call.args).map(|arg| arg.value).collect();
        let graph: BTreeSet<_> = arguments.iter().flat_map(|v| before.refs[v.as_u32() as usize].iter().copied())
            .collect();
        let labels: BTreeSet<_> = arguments.iter().flat_map(|v| before.contained[v.as_u32() as usize].iter().copied())
            .collect();
        if let Some(packet) = call.packet {
            for piece in &before.tokens[packet.as_u32() as usize].pieces {
                if piece.provenance & !FRESH != 0 {
                    self.error(R::B53, "call entry can accept only the offered fresh portion");
                }
                if !self.subset(piece, &graph, &labels) {
                    self.error(R::B51, "offered packet is not a subset of the call argument retention graph");
                }
            }
        }
        for (value, _) in self.body.values.iter() {
            if Some(value) == call.packet || before.availability[value.as_u32() as usize] != AVAILABLE {
                continue;
            }
            let filter = self.graph_filter(before, graph.clone(), labels.clone(), true);
            let (_, selected) = self.partition_filter(&before.tokens[value.as_u32() as usize], filter);
            if !selected.pieces.is_empty() {
                self.error(R::B51, "fresh call inputs must enter callee cleanup, including borrow parameters");
            }
        }
    }
    fn call_deps(&mut self, call: &CallSite, receiver: Option<CallArgument>, target: Option<FuncId>, flow: &Flow) {
        let arguments: Vec<_> = receiver.iter().chain(&call.args).map(|argument| argument.value).collect();
        let formal = target.and_then(|id| self.module.functions.get(id).ok());
        let count = formal.map_or(0, |function| function.signature.params.iter().filter(|ty| {
            self.module.types.get(**ty) == Ok(&Type::Service(ServiceType::Deps))
        }).count());
        if count != 0 && call.deps.len() != count {
            self.error(R::B51, "call does not provide its declared deps ABI parameters");
        }
        if count == 0 && call.deps.is_empty() {
            return;
        }
        let supplied: BTreeSet<_> = call.deps.iter().flat_map(|value| {
            flow.refs[value.as_u32() as usize].iter().copied()
        }).collect();
        let required: BTreeSet<_> = arguments.iter().flat_map(|value| {
            flow.full_deps[value.as_u32() as usize].iter().copied()
        }).collect();
        if !required.is_subset(&supplied) {
            self.error(R::B32, "call deps ABI omits part of an argument's full dependencies");
        }
        // A native/indirect ABI without bodies uses the merged full graph. For
        // a checked body, each paired parameter additionally has its own check.
        let Some(body) = formal.and_then(|function| function.body.as_ref()) else {
            return;
        };
        let Ok(entry) = body.blocks.get(body.entry) else {
            return;
        };
        let parameters: Vec<_> = entry.params.iter().copied().filter(|value| {
            body.values.get(*value).ok().and_then(|value| self.module.types.get(value.ty).ok())
                == Some(&Type::Service(ServiceType::Deps))
        }).collect();
        let language: Vec<_> = entry.params.iter().copied().filter(|value| {
            body.values.get(*value).ok().and_then(|value| self.module.types.get(value.ty).ok())
                .is_some_and(|ty| !matches!(ty, Type::Service(_) | Type::Descriptor))
        }).collect();
        for (parameter, argument) in language.iter().zip(arguments) {
            let Ok(ValueData { facts: ValueFacts::Managed { deps }, .. }) = body.values.get(*parameter) else {
                continue;
            };
            let Ok(DepExpr::FromSsa(dep)) = body.deps.get(*deps) else {
                continue;
            };
            let supplied = parameters.iter().position(|parameter| parameter == dep)
                .and_then(|index| call.deps.get(index));
            if supplied.is_none_or(|value| {
                !flow.full_deps[argument.as_u32() as usize].is_subset(&flow.refs[value.as_u32() as usize])
            }) {
                self.error(R::B32, "paired call deps parameter does not carry its value's full graph");
            }
        }
    }
    fn subset(&self, piece: &Piece, graph: &BTreeSet<Origin>, labels: &BTreeSet<ValueId>) -> bool {
        if !piece.family && graph.contains(&piece.origin) {
            return true;
        }
        piece.predicates.iter().any(|(filter, selected)| {
            *selected && !filter.stale && filter.keys.is_subset(graph)
                && (filter.labels.is_empty() || filter.labels.is_subset(labels))
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn call_results(
        &mut self, call: &CallSite, receiver: Option<CallArgument>, origin: Origin, before: &Flow,
        flow: &mut Flow, schema: &OutcomeContract, slots: &mut [Slot],
    ) {
        let Some(summary) = call.summary.and_then(|id| self.module.summaries.get(id).ok()) else {
            return;
        };
        let arguments: Vec<_> = receiver.iter().chain(&call.args).map(|a| a.value).collect();
        let language: Vec<_> = schema.results.iter().enumerate().filter(|(_, slot)| {
            !matches!(slot.ty, ResultType::Service(_))
        }).map(|(i, _)| i).collect();
        let internal = summary.entry_kind == EntryKind::Internal;
        let fresh = summary.results.iter().any(|relation| !relation.fresh.is_empty());
        let state = summary.results.first().map_or(AdmissionProvenance::Bound, |relation| relation.provenance);
        let mixed = summary.results.iter().any(|relation| relation.provenance != state);
        let mut fresh_states = std::collections::BTreeMap::new();
        let mut all_refs = BTreeSet::new();
        let mut all_labels = BTreeSet::new();
        let mut fresh_refs: std::collections::BTreeMap<CreationSiteId, BTreeSet<Origin>> =
            std::collections::BTreeMap::new();
        for (index, relation) in summary.results.iter().enumerate() {
            let Some(&slot) = language.get(index) else {
                continue;
            };
            let mut aliases = BTreeSet::new();
            let mut labels = BTreeSet::new();
            for alias in &relation.aliases {
                aliases.extend(self.summary_selection(before, &arguments, alias));
                labels.extend(self.summary_labels(before, &arguments, alias));
            }
            let opaque = relation.aliases.len() > 1 || summary.results.len() > 1 || relation.fresh.len() > 1;
            if opaque {
                let view = match origin {
                    Origin::Instruction(inst, _) => Origin::ViewInstruction(inst, slot as u16),
                    Origin::Invoke(block, _) => Origin::ViewInvoke(block, slot as u16),
                    _ => origin,
                };
                let mut candidates = aliases.clone();
                candidates.extend(relation.fresh.iter().map(|site| creation(origin, *site)));
                flow.views.insert(view, candidates);
                slots[slot].refs.insert(view);
            } else {
                slots[slot].refs = aliases;
                slots[slot].contained = labels.clone();
                if relation.fresh.is_empty() && let [SummarySelection::Parameter(parameter)] = &relation.aliases[..]
                    && let Some(value) = arguments.get(usize::from(*parameter)) {
                    slots[slot].aliases = before.aliases[value.as_u32() as usize].clone();
                    slots[slot].identity = before.identities[value.as_u32() as usize];
                }
                if !relation.fresh.is_empty() {
                    let key = creation(origin, relation.fresh[0]);
                    slots[slot].refs.insert(key);
                    slots[slot].identity = Some(key);
                }
            }
            // The external dependency positions classify dependencies from the
            // selected aliases. They do not retain an entire input container.
            for parameter in &relation.external_deps {
                let mentioned = relation.aliases.iter().any(|selection| summary_mentions(selection, *parameter));
                if !mentioned && let Some(value) = arguments.get(usize::from(*parameter)) {
                    slots[slot].requirements.extend(&before.full_deps[value.as_u32() as usize]);
                }
            }
            for site in &relation.fresh {
                if let Some(old) = fresh_states.insert(*site, relation.provenance) && old != relation.provenance {
                    self.error(R::B51, "one fresh creation site cannot have two result provenance states");
                }
                fresh_refs.entry(*site).or_default().extend(&slots[slot].refs);
            }
            all_refs.extend(&slots[slot].refs);
            all_labels.extend(labels);
        }
        let mut output = if internal {
            call.packet.map(|p| before.tokens[p.as_u32() as usize].clone()).unwrap_or_default()
        } else { Token::default() };
        if !internal && let Some(packet) = call.packet {
            let mut permitted = BTreeSet::new();
            let mut labels = BTreeSet::new();
            for parameter in &summary.parameters {
                if parameter.convention == FreshConvention::Take
                    && parameter.destination == TransferDestination::Result
                    && let Some(selection) = &parameter.retention {
                    permitted.extend(self.summary_selection(before, &arguments, selection));
                    labels.extend(self.summary_labels(before, &arguments, selection));
                }
            }
            let filter = self.graph_filter(flow, permitted, labels, false);
            let (denied, allowed) = self.partition_filter(&before.tokens[packet.as_u32() as usize], filter);
            let filter = self.graph_filter(flow, all_refs.clone(), all_labels.clone(), false);
            let (unused, selected) = self.partition_filter(&allowed, filter);
            let settled: Vec<_> = denied.pieces.union(&unused.pieces).cloned().collect();
            let live = flow.clone();
            self.invalidate(&live, flow, &settled);
            for &slot in &language {
                let refs = slots[slot].refs.union(&slots[slot].requirements).copied().collect();
                if self.keys_invalidated(flow, &refs, &slots[slot].aliases, &settled) {
                    slots[slot].lifetime = 2;
                    self.error(R::B51, "call summary returns a borrow of an input settled by callee cleanup");
                }
            }
            for piece in &settled {
                flow.dead.insert(piece.origin);
            }
            flow.closed_inputs.extend(settled);
            output = selected;
            output.pieces = output.pieces.into_iter().map(|mut piece| {
                let provenance = if mixed {
                    let mut states = 0u8;
                    let token = Token { pieces: BTreeSet::from([piece.clone()]), ..Token::default() };
                    for (index, relation) in summary.results.iter().enumerate() {
                        let Some(&slot) = language.get(index) else {
                            continue;
                        };
                        let filter = self.graph_filter(flow, slots[slot].refs.clone(),
                            slots[slot].contained.clone(), false);
                        let (_, selected) = self.partition_filter(&token, filter);
                        if !selected.pieces.is_empty() {
                            states |= provenance_bit(relation.provenance);
                        }
                    }
                    if states.count_ones() != 1 {
                        self.error(R::B51, "transferred input has ambiguous result provenance");
                    }
                    states
                } else { provenance_bit(state) };
                piece.provenance = provenance;
                if provenance & !FRESH == 0 {
                    // Publication is a new caller admission boundary. Moving,
                    // aliasing or rehoming inside a boundary cannot do this.
                    piece.owner = None;
                    piece.journal = false;
                }
                piece
            }).collect();
        }
        for (site, state) in fresh_states {
            let key = creation(origin, site);
            let mut piece = Piece::new(key, state);
            piece.family = summary.results.len() > 1;
            if piece.family {
                let filter = self.graph_filter(flow, fresh_refs.remove(&site).unwrap_or_default(),
                    BTreeSet::new(), false);
                piece.predicates.insert(filter, true);
            }
            output.pieces.insert(piece);
            flow.provenance.insert(key, provenance_bit(state));
            flow.dead.remove(&key);
        }
        if let Some(slot) = schema.results.iter().position(|s| s.name == SlotName::Packet) {
            slots[slot].token = output;
        } else if fresh && self.module.profile != Profile::High {
            self.error(R::B23, "fresh call result has no explicit ownership packet");
        }
        if let Some(slot) = schema.results.iter().position(|s| s.name == SlotName::Deps) {
            slots[slot].refs = all_refs;
            let required: BTreeSet<_> = slots.iter().flat_map(|slot| slot.requirements.iter().copied()).collect();
            slots[slot].refs.extend(required);
        }
    }
    fn summary_labels(&mut self, flow: &Flow, args: &[ValueId], root: &SummarySelection) -> BTreeSet<ValueId> {
        let mut labels = BTreeSet::new();
        let mut pending = vec![root];
        while let Some(selection) = pending.pop() {
            if !self.state.step(1) {
                break;
            }
            match selection {
                SummarySelection::Union(items) => pending.extend(items),
                SummarySelection::Parameter(i) | SummarySelection::OwnerGraph(i) | SummarySelection::Descriptor(i) => {
                    if let Some(value) = args.get(usize::from(*i)) {
                        labels.extend(&flow.contained[value.as_u32() as usize]);
                    }
                }
                SummarySelection::Field { .. } | SummarySelection::Path { .. } => {}
            }
        }
        labels
    }
    fn summary_selection(&mut self, flow: &Flow, args: &[ValueId], root: &SummarySelection) -> BTreeSet<Origin> {
        let mut selected = BTreeSet::new();
        let mut pending = vec![root];
        while let Some(selection) = pending.pop() {
            if !self.state.step(1) {
                break;
            }
            let index = match selection {
                SummarySelection::Parameter(i) | SummarySelection::OwnerGraph(i) | SummarySelection::Descriptor(i)
                => *i,
                SummarySelection::Path { parameter, fields } => {
                    if let Some(value) = args.get(usize::from(*parameter)) {
                        if fields.is_empty() {
                            selected.extend(&flow.refs[value.as_u32() as usize]);
                        } else if fields.len() == 1 {
                            selected.extend(self.field(flow, *value, fields[0]));
                        } else if fields.len() <= 4 {
                            let mut keys = BTreeSet::from([self.identity(flow, *value)]);
                            let mut known = true;
                            for (index, field) in fields.iter().enumerate() {
                                let mut next = BTreeSet::new();
                                for key in keys {
                                    if let Some((refs, identity)) = flow.fields.get(&(
                                        key, field.declaration, field.index,
                                    )) {
                                        if index + 1 < fields.len() {
                                            next.extend(identity);
                                        } else {
                                            next.extend(refs);
                                        }
                                    } else {
                                        known = false;
                                    }
                                }
                                keys = next;
                            }
                            if known {
                                selected.extend(keys);
                            } else {
                                let mut path = [(DeclId::from_u32(0), 0); 4];
                                for (slot, field) in path.iter_mut().zip(fields) {
                                    *slot = (field.declaration, field.index);
                                }
                                selected.insert(Origin::Projection(*value, ProjectionKind::Path {
                                    fields: path, len: fields.len() as u8,
                                }));
                            }
                        }
                    }
                    continue;
                }
                SummarySelection::Field { parameter, field } => {
                    if let Some(value) = args.get(usize::from(*parameter)) {
                        selected.extend(self.field(flow, *value, *field));
                    }
                    continue;
                }
                SummarySelection::Union(items) => {
                    pending.extend(items);
                    continue;
                }
            };
            if let Some(value) = args.get(usize::from(index)) {
                selected.extend(&flow.refs[value.as_u32() as usize]);
            }
        }
        selected
    }
    pub(super) fn body_summary(&mut self, flow: &Flow, normal: bool, values: &[ValueId]) {
        if self.module.profile == Profile::High {
            return;
        }
        let Some(summary) = self.declaration.summary.and_then(|id| self.module.summaries.get(id).ok()) else {
            return;
        };
        let Ok(entry) = self.body.blocks.get(self.body.entry) else {
            return;
        };
        let arguments: Vec<_> = entry.params.iter().copied().filter(|v| {
            self.body.values.get(*v).ok().and_then(|v| self.module.types.get(v.ty).ok())
                .is_some_and(|ty| !matches!(ty, Type::Service(_) | Type::Descriptor))
        }).collect();
        if !normal {
            let allowed = summary.contract.failures.iter().fold(0, |mask, kind| mask | failure_bit(*kind));
            if flow.active_failure & !allowed != 0 {
                self.error(R::B51, "body exposes a failure kind absent from its declared summary");
            }
            if entry.params.iter().any(|v| linear(self.module, self.body, *v)
                && flow.availability[v.as_u32() as usize] == AVAILABLE) {
                self.error(R::B54, "failure/cancel exit has not settled accepted inputs");
            }
            return;
        }
        if summary.entry_kind == EntryKind::Internal
            && self.declaration.signature.results.iter().any(|ty| {
                self.module.types.get(*ty) == Ok(&Type::Service(ServiceType::Own))
            }) {
            for &parameter in &entry.params {
                if self.body.values.get(parameter).ok().and_then(|v| self.module.types.get(v.ty).ok())
                    != Some(&Type::Service(ServiceType::Own)) { continue; }
                let origin = Origin::Input(parameter);
                let exported = values.iter().any(|value| {
                    flow.tokens[value.as_u32() as usize].pieces.iter().any(|p| p.origin == origin)
                });
                if !exported || flow.dead.contains(&origin) {
                    self.error(R::B51, "internal packet ABI must preserve input contents on normal return");
                }
            }
        }
        let language: Vec<_> = values.iter().copied().filter(|v| self.body.values.get(*v).ok()
            .and_then(|v| self.module.types.get(v.ty).ok()).is_some_and(|ty| !matches!(ty, Type::Service(_))))
            .collect();
        if summary.entry_kind == EntryKind::CallBoundary {
            let graph = language.iter().flat_map(|value| flow.refs[value.as_u32() as usize].iter().copied())
                .collect();
            let output_labels = language.iter().flat_map(|value| {
                flow.contained[value.as_u32() as usize].iter().copied()
            }).collect();
            for value in values {
                if self.body.values.get(*value).ok().and_then(|value| self.module.types.get(value.ty).ok())
                    != Some(&Type::Service(ServiceType::Own)) { continue; }
                for piece in &flow.tokens[value.as_u32() as usize].pieces {
                    if !self.subset(piece, &graph, &output_labels) {
                        self.error(R::B51, "body exports obligations outside its result retention graph");
                    }
                }
            }
            let mut permitted = BTreeSet::new();
            let mut labels = BTreeSet::new();
            for parameter in &summary.parameters {
                if parameter.convention == FreshConvention::Take
                    && parameter.destination == TransferDestination::Result
                    && let Some(selection) = &parameter.retention {
                    permitted.extend(self.summary_selection(flow, &arguments, selection));
                    labels.extend(self.summary_labels(flow, &arguments, selection));
                }
            }
            for &value in values {
                for piece in &flow.tokens[value.as_u32() as usize].pieces {
                    let accepted = match piece.origin {
                        Origin::Input(input) => self.body.values.get(input).ok()
                            .and_then(|v| self.module.types.get(v.ty).ok())
                            == Some(&Type::Service(ServiceType::Own)),
                        _ => false,
                    };
                    if accepted && !self.subset(piece, &permitted, &labels) {
                        self.error(R::B51, "body exports accepted inputs beyond its declared take retention");
                    }
                }
            }
        }
        for (value, relation) in language.iter().zip(&summary.results) {
            if summary.entry_kind == EntryKind::CallBoundary {
                let keys = flow.refs[value.as_u32() as usize].clone();
                let labels = flow.contained[value.as_u32() as usize].clone();
                for output in values {
                    let filter = self.graph_filter(flow, keys.clone(), labels.clone(), false);
                    let (_, selected) = self.partition_filter(&flow.tokens[output.as_u32() as usize], filter);
                    if selected.pieces.iter().any(|piece| piece.provenance != provenance_bit(relation.provenance)) {
                        self.error(R::B51, "body result provenance differs from its declared summary");
                    }
                }
            }
            let mut aliases = BTreeSet::new();
            for alias in &relation.aliases {
                aliases.extend(self.summary_selection(flow, &arguments, alias));
            }
            let mut allowed = aliases.clone();
            for param in &relation.external_deps {
                if let Some(value) = arguments.get(usize::from(*param)) {
                    allowed.extend(&flow.full_deps[value.as_u32() as usize]);
                }
            }
            let refs = &flow.full_deps[value.as_u32() as usize];
            let mut keys = refs.clone();
            keys.extend(self.possible(flow, refs));
            for key in keys {
                if allowed.contains(&key) {
                    continue;
                }
                match key {
                    Origin::External(_) => {
                        self.error(R::B51, "body returns an alias/dependency absent from its declared summary");
                    }
                    Origin::Instruction(..) | Origin::Invoke(..) | Origin::RetiredInstruction(..)
                    | Origin::RetiredInvoke(..) if relation.fresh.is_empty() => {
                        self.error(R::B51, "body returns a fresh identity absent from its declared summary");
                    }
                    Origin::Projection(..) | Origin::RetiredProjection(..) => {
                        let possible = self.possible(flow, &BTreeSet::from([key]));
                        if possible.is_empty() || !possible.is_subset(&allowed) {
                            self.error(R::B51, "body returns an undeclared projection of an input");
                        }
                    }
                    _ => {}
                }
            }
            if let Some(identity @ Origin::External(_)) = flow.identities[value.as_u32() as usize]
                && !aliases.contains(&identity) {
                self.error(R::B51, "body aliases an input without a declared result alias");
            }
        }
    }
}

fn summary_mentions(root: &SummarySelection, parameter: u16) -> bool {
    let mut pending = vec![root];
    while let Some(selection) = pending.pop() {
        match selection {
            SummarySelection::Union(items) => pending.extend(items),
            SummarySelection::Parameter(i) | SummarySelection::OwnerGraph(i) | SummarySelection::Descriptor(i)
            | SummarySelection::Path { parameter: i, .. }
            | SummarySelection::Field { parameter: i, .. } if *i == parameter => return true,
            _ => {}
        }
    }
    false
}
