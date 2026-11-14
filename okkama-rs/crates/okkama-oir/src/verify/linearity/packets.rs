use super::*;

impl Engine<'_, '_> {
    pub(super) fn partition(&mut self, flow: &Flow, token: &Token, selection: SelectionId) -> (Token, Token) {
        let mut rest = token.clone();
        let mut selected = Token::default();
        for (keys, fresh, labels) in self.selection_terms(flow, selection) {
            let possible = self.possible(flow, &keys);
            let filter = Filter { keys, fresh, possible, labels, stale: false, state: 0 };
            let (next, part) = self.partition_filter(&rest, filter);
            rest = next;
            selected = self.union([selected, part], true);
        }
        (rest, selected)
    }
    pub(super) fn partition_filter(&mut self, token: &Token, base: Filter) -> (Token, Token) {
        let Filter { keys, fresh, possible, labels, .. } = &base;
        let mut rest = Token {
            scope: token.scope, sources: token.sources.clone(), ..Token::default()
        };
        let mut selected = Token::default();
        for piece in &token.pieces {
            let admits_all = piece.provenance & !FRESH == 0;
            let filter = Filter {
                keys: keys.clone(), fresh: *fresh && !admits_all,
                possible: possible.clone(), labels: labels.clone(), stale: false,
                state: if *fresh && !admits_all { piece.provenance } else { 0 },
            };
            let unknown = piece.family || matches!(piece.origin, Origin::Input(_))
                || possible.contains(&piece.origin);
            let provenance = piece.provenance;
            let eligible = !*fresh || provenance & FRESH != 0;
            let related = matches!(piece.origin, Origin::Input(_))
                || keys.contains(&piece.origin) || possible.contains(&piece.origin);
            if !eligible || keys.is_empty() || !related {
                rest.pieces.insert(piece.clone());
            } else if !unknown {
                selected.pieces.insert(piece.clone());
            } else {
                let mut yes = piece.clone();
                let mut no = piece.clone();
                if piece.predicates.get(&filter) != Some(&false) {
                    yes.predicates.insert(filter.clone(), true);
                    if *fresh {
                        yes.provenance &= FRESH;
                    }
                    selected.pieces.insert(yes);
                }
                if piece.predicates.get(&filter) != Some(&true) {
                    no.predicates.insert(filter.clone(), false);
                    rest.pieces.insert(no);
                }
            }
        }
        (rest, selected)
    }
    fn union(&mut self, parts: impl IntoIterator<Item = Token>, disjoint: bool) -> Token {
        let mut result = Token::default();
        for part in parts {
            if !self.state.step(part.pieces.len().saturating_mul(result.pieces.len()).saturating_add(1)) {
                return result;
            }
            if disjoint && part.pieces.iter().any(|piece| result.pieces.iter().any(|old| piece.overlaps(old))) {
                self.error(R::B22, "join inputs have overlapping ownership contents");
            }
            result.scope = result.scope.or(part.scope);
            result.reserve.extend(part.reserve);
            result.sources.extend(part.sources);
            result.pieces.extend(part.pieces);
        }
        result
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn operation(
        &mut self, block: BlockId, op: &Operation, schema: &OutcomeContract, outcome: Outcome,
        origin: Origin, flow: &mut Flow,
    ) -> Vec<Slot> {
        if !self.charge_flow(flow) {
            return Vec::new();
        }
        if outcome == Outcome::Normal {
            if super::super::summaries::call(op).is_none() {
                for slot in 0..schema.results.len() {
                    if !self.charge_flow(flow) {
                        return Vec::new();
                    }
                    flow.retire_origin(view_origin(origin, slot));
                }
            }
            if matches!(op, Operation::Core(CoreOp::ObjNew { .. } | CoreOp::ObjNext { .. })) {
                flow.retire_origin(origin);
            }
            if let Some((call, _, _)) = super::super::summaries::call(op)
                && let Some(summary) = call.summary.and_then(|id| self.module.summaries.get(id).ok()) {
                let sites: BTreeSet<_> = summary.results.iter().flat_map(|relation| &relation.fresh).copied().collect();
                for site in sites {
                    if !self.charge_flow(flow) {
                        return Vec::new();
                    }
                    flow.retire_origin(creation(origin, site));
                }
                for (index, _) in summary.results.iter().enumerate() {
                    let view = match origin {
                        Origin::Instruction(inst, _) => Origin::ViewInstruction(inst, index as u16),
                        Origin::Invoke(block, _) => Origin::ViewInvoke(block, index as u16),
                        _ => origin,
                    };
                    if !self.charge_flow(flow) {
                        return Vec::new();
                    }
                    flow.retire_origin(view);
                }
            }
        }
        let before = flow.clone();
        if let Operation::Ownership(OwnershipOp::ScopeUnwind { failure, .. }) = op {
            flow.active_failure = before.failures[failure.as_u32() as usize];
        }
        if outcome != Outcome::Normal
            && let Ok(contract) = ContractRegistry::new(self.module).construction_schema(op, self.body) {
            flow.active_failure = if outcome == Outcome::Cancel { failure_bit(FailureKind::Cancelled) }
                else { contract.effects.failures.iter().fold(0, |mask, kind| mask | failure_bit(*kind)) };
        }
        let refs = op.operand_refs();
        let mut metadata: Vec<_> = refs.iter().filter_map(|operand| {
            (!matches!(operand.reference, OperandRef::Value(_))).then_some(operand.reference)
        }).collect();
        let mut seen = BTreeSet::new();
        while let Some(reference) = metadata.pop() {
            if !self.state.step(1) {
                return Vec::new();
            }
            if !seen.insert(reference) {
                continue;
            }
            if let OperandRef::Value(value) = reference {
                self.live_use(&before, value);
            }
            else if let Ok(children) = cfg::expression_children(reference, self.body) { metadata.extend(children); }
        }
        for operand in refs {
            if let OperandRef::Value(value) = operand.reference {
                let rule = if matches!(op, Operation::Ownership(OwnershipOp::ScopeEnd { .. }
                    | OwnershipOp::ScopeUnwind { .. })) { R::B41 } else { R::B21 };
                self.use_value(flow, value, operand.mode, rule);
            }
        }
        self.scope_operation(block, op, outcome, &before, flow);
        self.call_entry(op, &before, flow);
        let mut slots = vec![Slot::default(); schema.results.len()];
        let slot_index = |name: SlotName| schema.results.iter().position(|slot| slot.name == name);
        for transfer in &schema.transfers {
            if !self.state.step(1) {
                return slots;
            }
            match transfer {
                TransferRule::Preserve { input, output } => {
                    if let Some(i) = slot_index(*output) {
                        slots[i].token = before.tokens[input.as_u32() as usize].clone();
                        slots[i].journal = before.journals[input.as_u32() as usize].clone();
                        slots[i].refs = before.refs[input.as_u32() as usize].clone();
                        slots[i].identity = before.identities[input.as_u32() as usize];
                        slots[i].aliases = before.aliases[input.as_u32() as usize].clone();
                        slots[i].requirements = before.full_deps[input.as_u32() as usize].clone();
                        slots[i].lifetime = before.lifetimes[input.as_u32() as usize];
                    }
                }
                TransferRule::Move { inputs, outputs } => {
                    let token = self.union(inputs.iter().map(|v| before.tokens[v.as_u32() as usize].clone()),
                        matches!(op, Operation::Ownership(OwnershipOp::Join { .. })));
                    for output in outputs {
                        if let Some(i) = slot_index(*output) {
                            // Buffer uniqueness and the element journal are independent.
                            if *output != SlotName::Buffer && *output != SlotName::Value {
                                slots[i].token = token.clone();
                            }
                        }
                    }
                    if let Some(i) = slot_index(SlotName::Packet) {
                        slots[i].token = token;
                    }
                }
                TransferRule::Partition { input, selection, rest, selected } => {
                    let (left, right) = self.partition(&before, &before.tokens[input.as_u32() as usize], *selection);
                    if matches!(op, Operation::Ownership(OwnershipOp::Split { .. })) {
                        let original = &before.tokens[input.as_u32() as usize];
                        let proved = left.pieces.iter().chain(&right.pieces).all(|piece| {
                            original.pieces.contains(piece)
                        });
                        if !proved {
                            self.error(R::B22, "unchecked partition requires a static proof; use own.split.checked");
                        }
                    }
                    if let Some(i) = slot_index(*rest) {
                        slots[i].token = left;
                    }
                    if let Some(i) = slot_index(*selected) {
                        slots[i].token = right;
                    }
                    if let Some(i) = slot_index(SlotName::ExternalDeps) {
                        let (keys, _) = self.selection(&before, *selection);
                        let owned: BTreeSet<_> = slots.iter().flat_map(|s| s.token.pieces.iter().map(|p| p.origin))
                            .collect();
                        slots[i].refs = keys.difference(&owned).copied().collect();
                    }
                }
                TransferRule::Admit { state, packet, output, provenance } => {
                    let owner = scope(self.module, self.body, *state);
                    let mut incoming = before.tokens[packet.as_u32() as usize].clone();
                    incoming.pieces = incoming.pieces.into_iter().map(|mut piece| {
                        if piece.owner.is_some() && piece.owner != owner && piece.provenance & !FRESH != 0 {
                            self.error(R::B43, "admission cannot silently change an existing lexical owner");
                        }
                        piece.owner = owner;
                        piece.journal = true;
                        if let ProvenanceTransition::Set(provenance) = provenance {
                            piece.provenance = provenance_bit(*provenance);
                            if !matches!(piece.origin, Origin::Input(_)) {
                                flow.provenance.insert(piece.origin, piece.provenance);
                            }
                        }
                        piece
                    }).collect();
                    let mut token = self.union([before.tokens[state.as_u32() as usize].clone(), incoming], false);
                    token.scope = owner;
                    if let Some(i) = slot_index(*output) {
                        slots[i].token = token;
                    }
                }
                TransferRule::ScopeTransfer { states, selection } => match states {
                    TransferStates::Same(value) => {
                        if let Some(i) = slot_index(SlotName::State) {
                            slots[i].token = before.tokens[value.as_u32() as usize].clone();
                        }
                    }
                    TransferStates::Between { from, to } => {
                        let (rest, mut part) = self.partition(&before, &before.tokens[from.as_u32() as usize],
                            *selection);
                        let destination = scope(self.module, self.body, *to);
                        part.pieces = part.pieces.into_iter().map(|mut piece| {
                            piece.owner = destination;
                            piece
                        }).collect();
                        let mut target = self.union([before.tokens[to.as_u32() as usize].clone(), part], false);
                        target.scope = destination;
                        if let Some(i) = slot_index(SlotName::From) {
                            slots[i].token = rest;
                        }
                        if let Some(i) = slot_index(SlotName::To) {
                            slots[i].token = target;
                        }
                    }
                },
                TransferRule::Close { input } => {
                    let reserve = match op {
                        Operation::Ownership(OwnershipOp::ScopeEnd { reserve, .. }
                            | OwnershipOp::ScopeUnwind { reserve, .. }) => *reserve,
                        _ => None,
                    };
                    self.close(&before, flow, *input, reserve);
                }
                TransferRule::Settle { inputs } => {
                    for input in inputs {
                        let pieces: Vec<_> = before.tokens[input.as_u32() as usize].pieces.iter().cloned().collect();
                        self.invalidate(&before, flow, &pieces);
                        for piece in &pieces {
                            flow.dead.insert(piece.origin);
                            if matches!(piece.origin, Origin::Input(_)) {
                                flow.closed_inputs.insert(piece.clone());
                            }
                        }
                    }
                }
                TransferRule::Reserve { packet, boundary } => {
                    let mut token = before.tokens[packet.as_u32() as usize].clone();
                    if let Ok(boundary) = self.body.boundaries.get(*boundary) {
                        token.reserve.insert(boundary.scope);
                        token.pieces = token.pieces.into_iter().map(|mut piece| {
                            piece.owner = piece.owner.or(Some(boundary.scope));
                            piece
                        }).collect();
                    }
                    if let Some(i) = schema.results.iter().position(|slot| {
                        slot.ty == ResultType::Service(ServiceType::Pending)
                    }) { slots[i].token = token; }
                }
                TransferRule::CloseReserve { scope: state, reserve, output, merge_scope_part } => {
                    let mut token = before.tokens[reserve.as_u32() as usize].clone();
                    let closing = scope(self.module, self.body, *state);
                    if let Some(scope) = closing {
                        token.reserve.remove(&scope);
                    }
                    if *merge_scope_part {
                        let merged: Vec<_> = token.pieces.iter().filter(|p| p.owner == closing).cloned().collect();
                        self.invalidate(&before, flow, &merged);
                        flow.closed_inputs.extend(merged);
                        token.pieces.retain(|p| {
                            if p.owner == closing {
                                flow.dead.insert(p.origin);
                                false
                            } else { true }
                        });
                    }
                    // Normal close protects the reserve from the closing plan.
                    for piece in &token.pieces {
                        flow.dead.remove(&piece.origin);
                    }
                    if let Some(i) = slot_index(*output) {
                        let index = reserve.as_u32() as usize;
                        slots[i].token = token;
                        slots[i].refs = before.refs[index].clone();
                        slots[i].identity = before.identities[index];
                        slots[i].aliases = before.aliases[index].clone();
                        slots[i].requirements = before.full_deps[index].clone();
                        slots[i].lifetime = flow.lifetimes[index];
                    }
                }
                TransferRule::CompleteIdentity { identity, state, output } => {
                    let mut token = before.tokens[state.as_u32() as usize].clone();
                    let origin = self.identity(&before, *identity);
                    let mut piece = Piece::new(origin, AdmissionProvenance::UnboundResult);
                    piece.owner = token.scope;
                    if token.pieces.iter().any(|p| p.origin == origin) {
                        self.error(R::B23, "identity has already been completed");
                    }
                    token.pieces.insert(piece);
                    flow.provenance.insert(origin, FRESH & 1);
                    if let Some(i) = slot_index(*output) {
                        slots[i].token = token;
                    }
                }
                // Async protocols and the lowered cleanup automaton are checked
                // by the reserved commit-4 pass, not by claimed postconditions.
                _ => {}
            }
        }
        self.buffer(op, outcome, &before, schema, &mut slots);
        self.stage(op, outcome, &before, flow, schema, &mut slots);
        self.results(op, schema, outcome, origin, &before, flow, &mut slots);
        for postcondition in &schema.postconditions {
            if let Postcondition::DischargeMustUse(value) = postcondition {
                flow.must_use[value.as_u32() as usize] = 2;
            }
        }
        if matches!(op, Operation::Call(_)) {
            for operand in op.operand_refs() {
                if let OperandRef::Value(value) = operand.reference
                    && successor(self.module, self.body, value) {
                    flow.must_use[value.as_u32() as usize] = 2;
                }
            }
        }
        slots
    }
    fn buffer(
        &mut self, op: &Operation, outcome: Outcome, before: &Flow,
        schema: &OutcomeContract, slots: &mut [Slot],
    ) {
        let Operation::Collection(op) = op else {
            return;
        };
        let (buffer, packet, inserted) = match op {
            CollectionOp::BuilderPush { buffer, packet, value } => (*buffer, packet, vec![*value]),
            CollectionOp::BuilderAppend { buffer, packet, array } => (*buffer, packet, vec![*array]),
            CollectionOp::BuilderAppendRange { buffer, packet, .. } => (*buffer, packet, vec![]),
            CollectionOp::MapBuilderPut { buffer, packet, key, value } => (*buffer, packet, vec![*key, *value]),
            CollectionOp::MapBuilderAppend { buffer, packet, map } => (*buffer, packet, vec![*map]),
            CollectionOp::BuilderFinish { buffer, packet } | CollectionOp::BuilderAbandon { buffer, packet }
            | CollectionOp::MapBuilderFinish { buffer, packet }
            | CollectionOp::MapBuilderAbandon { buffer, packet } => (*buffer, packet, vec![]),
            _ => return,
        };
        if self.module.profile != Profile::High && packet.is_none() {
            self.error(R::B81, "elaborated builder transition needs its explicit element journal");
        }
        let journal = packet.map(|value| before.tokens[value.as_u32() as usize].clone()).unwrap_or_default();
        if !before.journals[buffer.as_u32() as usize].pieces.is_subset(&journal.pieces) {
            self.error(R::B81, "builder transition loses the element journal of its current buffer version");
        }
        if outcome != Outcome::Normal {
            return;
        }
        let mut deps = before.refs[buffer.as_u32() as usize].clone();
        for value in inserted {
            deps.extend(&before.refs[value.as_u32() as usize]);
        }
        for (slot, info) in schema.results.iter().zip(slots) {
            if matches!(slot.name, SlotName::Buffer | SlotName::Value) {
                info.refs = deps.clone();
            }
            if slot.name == SlotName::Buffer {
                info.journal = journal.clone();
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn results(
        &mut self, op: &Operation, schema: &OutcomeContract, outcome: Outcome, origin: Origin,
        before: &Flow, flow: &mut Flow, slots: &mut [Slot],
    ) {
        let normal = outcome == Outcome::Normal;
        let mut retained = BTreeSet::new();
        for retention in &schema.retention {
            let deps = self.retention(before, &retention.selection);
            if let Some(i) = schema.results.iter().position(|s| s.name == retention.result) {
                slots[i].refs.extend(&deps);
                for value in retention_values(&retention.selection) {
                    slots[i].requirements.extend(before.full_deps[value.as_u32() as usize]
                        .difference(&before.refs[value.as_u32() as usize]).copied());
                    slots[i].lifetime |= before.lifetimes[value.as_u32() as usize] & 2;
                }
                for key in &deps {
                    if let Origin::Projection(value, _) = key {
                        flow.views.insert(*key, before.refs[value.as_u32() as usize].clone());
                        slots[i].bases.insert(*value);
                    }
                }
                if let Retention::Value(value) = retention.selection {
                    slots[i].identity = before.identities[value.as_u32() as usize];
                    slots[i].aliases = before.aliases[value.as_u32() as usize].clone();
                    slots[i].requirements = before.full_deps[value.as_u32() as usize].clone();
                }
            }
            retained.extend(deps);
        }
        let call = super::super::summaries::call(op);
        if normal && let Some((call, receiver, _)) = call {
            self.call_results(call, receiver, origin, before, flow, schema, slots);
        }
        let managed_result = schema.results.iter().find(|slot| slot.name == SlotName::Value)
            .is_some_and(|slot| match &slot.ty {
                ResultType::Exact(ty) => self.managed.get(ty.as_u32() as usize).copied().unwrap_or(true),
                ResultType::Object { decl, .. } => self.module.types.iter().any(|(id, ty)| {
                    matches!(ty, Type::Object { decl: actual, .. } if actual == decl)
                        && self.managed.get(id.as_u32() as usize).copied().unwrap_or(true)
                }),
                ResultType::SuccessorOf(value) => self.is_managed(*value),
                _ => true,
            });
        let creates = normal && managed_result && schema.postconditions.iter().any(|p| {
            matches!(p, Postcondition::FreshAdmission(AdmissionProvenance::UnboundResult))
        }) && !matches!(op, Operation::Exit(_) | Operation::Ownership(_)
            | Operation::Core(CoreOp::ObjInit { .. }));
        let incomplete = matches!(op, Operation::Core(CoreOp::ObjNew {
            construction: ConstructionStatus::Incomplete, ..
        }));
        if incomplete && normal && let Some(i) = schema.results.iter().position(|s| s.name == SlotName::Value) {
            slots[i].refs.insert(origin);
            slots[i].identity = Some(origin);
        }
        if creates {
            let piece = Piece::new(origin, AdmissionProvenance::UnboundResult);
            if let Some(i) = schema.results.iter().position(|s| s.name == SlotName::Value) {
                slots[i].refs.insert(origin);
                slots[i].identity = Some(origin);
            }
            if !incomplete {
                if let Some(i) = schema.results.iter().position(|s| s.name == SlotName::Packet) {
                    slots[i].token.pieces.insert(piece);
                } else if self.module.profile != Profile::High {
                    self.error(R::B23, "creation needs an explicit ownership result");
                }
                flow.provenance.insert(origin, 1);
                flow.dead.remove(&origin);
            }
        }
        if normal {
            self.object_fields(op, origin, before, flow);
            if let Operation::Ownership(OwnershipOp::ScopeBegin { scope }) = op
                && let Some(slot) = slots.first_mut() {
                slot.token.scope = Some(*scope);
            }
            if let Operation::Exit(ExitOp::Commit { pending, .. }) = op {
                for (schema, slot) in schema.results.iter().zip(slots.iter_mut()) {
                    slot.refs.extend(&before.full_deps[pending.as_u32() as usize]);
                    if schema.name == SlotName::Value {
                        slot.aliases = before.aliases[pending.as_u32() as usize].clone();
                        slot.identity = before.identities[pending.as_u32() as usize];
                        slot.requirements = before.full_deps[pending.as_u32() as usize].clone();
                        slot.lifetime = before.lifetimes[pending.as_u32() as usize];
                    }
                    slot.token.pieces = slot.token.pieces.iter().cloned().map(|mut piece| {
                        piece.provenance = 1;
                        piece.owner = None;
                        piece.journal = false;
                        if !matches!(piece.origin, Origin::Input(_)) {
                            flow.provenance.insert(piece.origin, 1);
                        }
                        piece
                    }).collect();
                }
            }
            if let Operation::Exit(ExitOp::Prepare { exit }) = op
                && let Some(value) = exit.value
                && let Some(slot) = slots.first_mut() {
                slot.refs = before.refs[value.as_u32() as usize].clone();
                slot.identity = before.identities[value.as_u32() as usize];
                slot.aliases = before.aliases[value.as_u32() as usize].clone();
                slot.requirements = before.full_deps[value.as_u32() as usize].clone();
                slot.lifetime = before.lifetimes[value.as_u32() as usize];
            }
        }
        let mut subset = BTreeSet::new();
        for retention in &schema.retention {
            let opaque = match &retention.selection {
                Retention::Difference { removed, .. } => !removed.is_empty(),
                Retention::MapMerge(items) => items.len() > 1,
                Retention::Entries(items) => items.len() > 1,
                _ => false,
            };
            if opaque && let Some(index) = schema.results.iter().position(|slot| slot.name == retention.result) {
                subset.insert(index);
            }
        }
        if matches!(op, Operation::Collection(CollectionOp::MapBuilderPut {
            ..
        }
            | CollectionOp::MapBuilderAppend { .. })) && normal {
            for (index, slot) in schema.results.iter().enumerate() {
                if slot.name == SlotName::Buffer {
                    subset.insert(index);
                }
            }
        }
        for index in subset {
            if slots[index].refs.is_empty() {
                continue;
            }
            let view = view_origin(origin, index);
            flow.views.insert(view, slots[index].refs.clone());
            slots[index].refs = BTreeSet::from([view]);
            slots[index].identity = None;
            slots[index].aliases.clear();
        }
        if super::super::summaries::call(op).is_none() {
            retained = schema.results.iter().zip(slots.iter()).filter(|(slot, _)| {
                matches!(slot.name, SlotName::Value | SlotName::Buffer)
            }).flat_map(|(_, slot)| slot.refs.iter().copied()).collect();
        }
        for (i, slot) in slots.iter_mut().enumerate() {
            if schema.results[i].ty == ResultType::Service(ServiceType::Failure) {
                slot.failures = match op {
                    Operation::Ownership(OwnershipOp::ScopeUnwind { failure, .. }) => {
                        before.failures[failure.as_u32() as usize]
                    }
                    Operation::Core(CoreOp::FailSecondary { primary, .. }) => {
                        before.failures[primary.as_u32() as usize]
                    }
                    _ => flow.active_failure,
                };
            }
            if matches!(schema.results[i].ty, ResultType::SuccessorOf(_)) {
                slot.must_use = 1;
            }
            if slot.refs.is_empty() && schema.results[i].name == SlotName::Deps {
                slot.refs.extend(&retained);
            }
        }
    }
    pub(super) fn facts(
        &mut self, flow: &Flow, op: &Operation, facts: &InstructionFacts, values: &[ValueId],
    ) {
        for (index, dep) in &facts.result_deps {
            if let Some(value) = values.get(usize::from(*index)) {
                let stated = self.dependencies(flow, *dep);
                if !flow.full_deps[value.as_u32() as usize].is_subset(&stated) {
                    self.error(R::B32, "instruction result_deps omits a required dependency");
                }
            } else {
                self.error(R::B32, "instruction result_deps names an absent result");
            }
        }
        for fact in &facts.provenance {
            let allowed = provenance_bit(fact.state);
            let mut found = false;
            for (value, _) in self.body.values.iter() {
                if flow.availability[value.as_u32() as usize] != AVAILABLE {
                    continue;
                }
                let (_, selected) = self.partition(flow, &flow.tokens[value.as_u32() as usize], fact.obligations);
                if !selected.pieces.is_empty() {
                    found = true;
                }
                if selected.pieces.iter().any(|piece| piece.provenance != allowed) {
                    self.error(R::B23, "provenance annotation is not justified by an executable transition");
                }
            }
            if !found {
                self.error(R::B23, "provenance annotation has no executable ownership witness");
            }
        }
        if matches!(op, Operation::Ownership(OwnershipOp::Move { .. } | OwnershipOp::Join { .. }))
            && facts.provenance.iter().any(|f| f.state == AdmissionProvenance::UnboundResult) {
            let (keys, _) = self.selection(flow, facts.provenance[0].obligations);
            if keys.is_empty() {
                self.error(R::B23, "a metadata fact cannot create an obligation");
            }
        }
    }
    fn object_fields(&mut self, op: &Operation, origin: Origin, before: &Flow, flow: &mut Flow) {
        let mut entries = Vec::new();
        match op {
            Operation::Core(CoreOp::ObjNew { declaration, fields, .. }) => {
                for (index, value) in fields.iter().enumerate() {
                    entries.push((origin, *declaration, index as u32, *value));
                }
            }
            Operation::Core(CoreOp::ObjInit { object, field, value, .. }) => {
                entries.push((self.identity(before, *object), field.declaration, field.index, *value));
            }
            Operation::Core(CoreOp::ObjNext { object, updates, .. }) => {
                let identity = self.identity(before, *object);
                for ((key, declaration, index), field) in &before.fields {
                    if *key == identity {
                        flow.fields.insert((origin, *declaration, *index), field.clone());
                    }
                }
                for update in updates {
                    entries.push((origin, update.field.declaration, update.field.index, update.value));
                }
            }
            _ => {}
        }
        for (key, declaration, index, value) in entries {
            flow.fields.insert((key, declaration, index),
                (before.refs[value.as_u32() as usize].clone(), before.identities[value.as_u32() as usize]));
        }
    }
    fn retention(&mut self, flow: &Flow, retention: &Retention) -> BTreeSet<Origin> {
        let projection = match retention {
            Retention::Field { base, field } => return self.field(flow, *base, *field),
            Retention::Index { base, index } => Some((*base, ProjectionKind::Index(*index))),
            Retention::Slice { base, start, end } => Some((*base, ProjectionKind::Slice(*start, *end))),
            Retention::MapEntry { map, key } => Some((*map, ProjectionKind::Map(*key))),
            Retention::MapPosition { map, index, key } => Some((*map, ProjectionKind::MapIndex(*index, *key))),
            Retention::Keys(value) => Some((*value, ProjectionKind::Keys)),
            Retention::Values(value) => Some((*value, ProjectionKind::Values)),
            Retention::ArrayRemove { array, index } => Some((*array, ProjectionKind::Remove(*index))),
            _ => None,
        };
        if let Some((value, kind)) = projection {
            if flow.refs[value.as_u32() as usize].is_empty() {
                return BTreeSet::new();
            }
            return BTreeSet::from([Origin::Projection(value, kind)]);
        }
        if let Retention::ArrayReplace { array, index, value } = retention {
            let mut refs = flow.refs[value.as_u32() as usize].clone();
            if !flow.refs[array.as_u32() as usize].is_empty() {
                refs.insert(Origin::Projection(*array, ProjectionKind::Remove(*index)));
            }
            return refs;
        }
        let values = retention_values(retention);
        let mut refs: BTreeSet<_> = values.iter().flat_map(|v| flow.refs[v.as_u32() as usize].iter().copied())
            .collect();
        if let Retention::ObjectFields { object, updates } = retention {
            refs.clear();
            let identity = self.identity(flow, *object);
            let declaration = self.body.values.get(*object).ok().and_then(|v| self.module.types.get(v.ty).ok())
                .and_then(|ty| match ty {
                    Type::Object { decl, .. } => Some(*decl),
                    _ => None,
                });
            if let Some(declaration) = declaration
                && let Ok(Declaration { kind: DeclarationKind::Object { fields, .. }, .. }) =
                    self.module.declarations.get(declaration) {
                for index in 0..fields.len() {
                    if let Some(update) = updates.iter().find(|u| u.field.index as usize == index) {
                        refs.extend(&flow.refs[update.value.as_u32() as usize]);
                    } else {
                        refs.extend(self.field(flow, *object, FieldRef { declaration, index: index as u32 }));
                    }
                }
            }
            // The new identity is inserted by the creation transition below.
            refs.remove(&identity);
        }
        refs
    }
}

fn retention_values(retention: &Retention) -> Vec<ValueId> {
    match retention {
        Retention::Value(v) | Retention::OwnerGraph(v) | Retention::ViewOwner(v) | Retention::Cache(v)
        | Retention::Keys(v) | Retention::Values(v) => vec![*v],
        Retention::Field { base, .. } | Retention::Index { base, .. } | Retention::Slice { base, .. }
        | Retention::Difference { base, .. } => vec![*base],
        Retention::MapEntry { map, .. } | Retention::MapPosition { map, .. } => vec![*map],
        Retention::ArrayReplace { array, value, .. } => vec![*array, *value],
        Retention::ArrayRemove { array, .. } => vec![*array],
        Retention::MapMerge(items) | Retention::Union(items) | Retention::CarrierGraph(items) => items.clone(),
        Retention::ObjectFields { object, updates } => {
            std::iter::once(*object).chain(updates.iter().map(|f| f.value)).collect()
        }
        Retention::Entries(items) => items.iter().flat_map(|(k, v)| [*k, *v]).collect(),
        Retention::Descriptor { value, .. } => vec![*value],
        Retention::TakenOutcome { future, .. } | Retention::AwaitFailure { future, .. } => vec![*future],
        Retention::Global(_) => vec![],
    }
}
