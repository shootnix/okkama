use super::*;

impl Engine<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn async_operation(
        &mut self, block: BlockId, op: &Operation, outcome: Outcome, origin: Origin, before: &Flow,
        flow: &mut Flow, schema: &OutcomeContract, slots: &mut [Slot],
    ) {
        if let Operation::Ownership(OwnershipOp::DynamicCheck { key, snapshot }) = op {
            let keys = &before.refs[key.as_u32() as usize];
            if before.tokens[snapshot.as_u32() as usize].snapshot.as_ref()
                .is_none_or(|(saved, stale)| *stale || saved != keys || saved.is_empty()) {
                self.error(R::B72, "dynamic.check requires a current snapshot for this exact stable key");
            }
        }
        if let Operation::Async(AsyncOp::Await { future, .. } | AsyncOp::AwaitShielded { future, .. }
            | AsyncOp::AwaitTake { future, .. }) = op
            && before.taken_caches.contains(&self.identity(before, *future)) {
            self.error(R::B91, "a taken cache cannot publish another view or ownership packet");
        }
        if outcome != Outcome::Normal {
            if let Operation::Async(AsyncOp::Await { future, .. } | AsyncOp::AwaitShielded { future, .. }
                | AsyncOp::AwaitTake { future, .. }) = op {
                let key = self.identity(before, *future);
                for (info, slot) in schema.results.iter().zip(slots.iter_mut()) {
                    if matches!(info.name, SlotName::Task | SlotName::Receipt) {
                        slot.token.cache = Some(key);
                        slot.token.private = true;
                    }
                }
            }
            return;
        }
        if let Operation::Ownership(OwnershipOp::DynamicRefresh { key, access }) = op {
            let keys = before.refs[key.as_u32() as usize].clone();
            if keys.is_empty() || keys.iter().any(|key| before.dead.contains(key))
                || before.availability[access.owner.as_u32() as usize] != AVAILABLE
                || before.availability[access.holder.as_u32() as usize] != AVAILABLE {
                self.error(R::B72, "snapshot refresh needs live access to the current owner and holder");
            }
            if let Some(slot) = slots.first_mut() {
                slot.token.snapshot = Some((keys, false));
            }
        }
        if let Operation::Ownership(OwnershipOp::ScopeTransfer {
            states: TransferStates::Between { from, to }, ..
        }) = op
            && before.tokens[from.as_u32() as usize].scope != before.tokens[to.as_u32() as usize].scope {
                let moved: BTreeSet<_> = schema.results.iter().zip(slots.iter()).filter(|(info, _)| {
                    info.name == SlotName::To
                }).flat_map(|(_, slot)| slot.token.pieces.iter().filter(|piece| {
                    before.tokens[from.as_u32() as usize].pieces.iter().any(|old| {
                        old.origin == piece.origin && old.owner != piece.owner
                    })
                }).map(|piece| piece.origin)).collect();
                for token in &mut flow.tokens {
                    if let Some((keys, stale)) = &mut token.snapshot {
                        *stale |= !keys.is_disjoint(&moved);
                    }
                }
        }
        let Operation::Async(op) = op else {
            return;
        };
        if before.frame_conflict && matches!(op, AsyncOp::FrameSave { .. } | AsyncOp::FrameResume { .. }
            | AsyncOp::FrameCancel { .. } | AsyncOp::FutureNew { .. }) {
            self.error(R::B95, "occupied frame families need producer layout/continuation metadata");
        }
        match op {
            AsyncOp::Pin { deps, owner, .. } => {
                for (info, slot) in schema.results.iter().zip(slots.iter_mut()) {
                    if info.name == SlotName::State {
                        slot.token = before.tokens[owner.as_u32() as usize].clone();
                    }
                    if info.name == SlotName::Pin {
                        slot.token.scope = before.tokens[owner.as_u32() as usize].scope;
                        slot.token.pin_deps = before.refs[deps.as_u32() as usize].clone();
                    }
                }
            }
            AsyncOp::PinTransfer { pin, owner, .. } | AsyncOp::Unpin { pin, owner, .. } => {
                for (info, slot) in schema.results.iter().zip(slots.iter_mut()) {
                    if info.name == SlotName::To {
                        slot.token = before.tokens[owner.as_u32() as usize].clone();
                    }
                    if info.name == SlotName::State {
                        slot.token = before.tokens[pin.as_u32() as usize].clone();
                        slot.token.scope = before.tokens[owner.as_u32() as usize].scope;
                    }
                }
            }
            AsyncOp::CacheRehome { state, target, .. } => {
                let mut token = before.tokens[state.as_u32() as usize].clone();
                token.scope = Some(*target);
                let changed = before.tokens[state.as_u32() as usize].scope != Some(*target);
                if changed && before.scopes.get(target) != Some(&1) {
                    self.error(R::B94, "cache rehome requires a live sufficient target boundary");
                }
                if let Some(key) = token.cache.filter(|_| changed) {
                    for (index, holder) in flow.tokens.iter_mut().enumerate() {
                        if flow.availability[index] != AVAILABLE {
                            continue;
                        }
                        let moved: Vec<_> = holder.pieces.iter().filter(|piece| piece.holder == Some(key))
                            .cloned().collect();
                        for mut piece in moved {
                            holder.pieces.remove(&piece);
                            piece.owner = Some(*target);
                            token.pieces.insert(piece);
                        }
                        if let Some((keys, stale)) = &mut holder.snapshot
                            && keys.contains(&key) {
                            *stale = true;
                        }
                    }
                }
                if let Some(slot) = slots.first_mut() {
                    slot.token = token;
                }
            }
            AsyncOp::FutureSettle { task, .. } => {
                let mut token = before.tokens[task.as_u32() as usize].clone();
                if token.settled {
                    self.error(R::B91, "waiter task cannot be settled twice");
                }
                token.settled = true;
                if let Some(slot) = slots.first_mut() {
                    slot.token = token;
                }
            }
            AsyncOp::UniqueCacheProof { proof } => {
                self.unique_proof(block, proof, before);
                if let Some(slot) = slots.first_mut() {
                    slot.token.unique = Some(self.identity(before, proof.future));
                    slot.token.unique_region.extend(&proof.region);
                }
            }
            AsyncOp::AwaitTake { future, unique, boundary, .. } => {
                let key = self.identity(before, *future);
                let capability = &before.tokens[unique.as_u32() as usize];
                if capability.unique != Some(key) || !capability.unique_region.contains(&block) {
                    self.error(R::B92, "await.take capability must prove this exact Future/cache and region");
                }
                let mut extracted = Token::default();
                for (index, token) in flow.tokens.iter_mut().enumerate() {
                    if flow.availability[index] != AVAILABLE {
                        continue;
                    }
                    let owned: Vec<_> = token.pieces.iter().filter(|piece| {
                        piece.holder == Some(key) && piece.origin != key
                    }).cloned().collect();
                    for piece in owned {
                        token.pieces.remove(&piece);
                        let mut piece = piece;
                        piece.holder = None;
                        piece.provenance = provenance_bit(match boundary {
                            TakeBoundary::Temporary { .. } => AdmissionProvenance::TemporaryAwaitResult,
                            _ => AdmissionProvenance::ExtractedAtExistingBoundary,
                        });
                        extracted.pieces.insert(piece);
                    }
                }
                flow.taken_caches.insert(key);
                for (info, slot) in schema.results.iter().zip(slots.iter_mut()) {
                    if info.name == SlotName::Packet {
                        slot.token = extracted.clone();
                    }
                }
                if matches!(boundary, TakeBoundary::Temporary { .. })
                    && before.provenance.get(&key).is_none_or(|provenance| provenance & !FRESH != 0) {
                    self.error(R::B93, "temporary take cannot change a bound Future's cleanup boundary");
                }
                if let TakeBoundary::Existing { owner } = boundary {
                    let key = self.identity(before, *future);
                    let actual = before.tokens.iter().flat_map(|token| &token.pieces)
                        .find(|piece| piece.origin == key).and_then(|piece| piece.owner);
                    if actual != Some(*owner) {
                        self.error(R::B93, "bound extraction must preserve its original owner boundary");
                    }
                }
            }
            AsyncOp::FrameAlloc { .. } => {
                flow.frames.insert(origin, std::collections::BTreeMap::new());
                if let Some(slot) = slots.first_mut() {
                    slot.token.frame = Some(origin);
                }
            }
            AsyncOp::FrameLoad { frame, slot, ty } => {
                let content = before.tokens[frame.as_u32() as usize].frame
                    .and_then(|key| before.frames.get(&key)).and_then(|layout| layout.get(slot));
                if let Some(content) = content {
                    if content.token.saved_type != Some(*ty)
                        || super::super::contracts::linear(self.module, *ty) {
                        self.error(R::B95, "raw frame load cannot reinterpret or copy a saved linear slot");
                    }
                    if let Some(result) = slots.first_mut() {
                        *result = content.clone();
                    }
                }
            }
            AsyncOp::FrameStore { frame, slot, value } => {
                let frame_key = before.tokens[frame.as_u32() as usize].frame;
                let mut layout = frame_key.and_then(|key| before.frames.get(&key).cloned()).unwrap_or_default();
                if layout.get(slot).is_some_and(|content| content.token.saved_type.is_some_and(|ty| {
                    super::super::contracts::linear(self.module, ty)
                })) { self.error(R::B95, "raw frame store cannot overwrite a saved linear slot"); }
                let index = value.as_u32() as usize;
                let mut token = before.tokens[index].clone();
                token.saved_type = self.body.values.get(*value).ok().map(|value| value.ty);
                layout.insert(*slot, Slot {
                    token, refs: before.refs[index].clone(), requirements: before.full_deps[index].clone(),
                    identity: before.identities[index], ..Slot::default()
                });
                if let Some(key) = frame_key {
                    flow.frames.insert(key, layout);
                }
            }
            AsyncOp::FrameSave { frame, slots: saved, .. } => {
                let frame_key = before.tokens[frame.as_u32() as usize].frame;
                if frame_key.is_none() {
                    self.error(R::B95, "save needs an allocated or forwarded frame");
                }
                let mut layout = frame_key.and_then(|key| before.frames.get(&key).cloned()).unwrap_or_default();
                let mut seen = BTreeSet::new();
                for slot in saved {
                    let value = slot.value.as_u32() as usize;
                    if layout.get(&slot.slot).is_some_and(|old| old.token.saved_type.is_some_and(|ty| {
                        super::super::contracts::linear(self.module, ty)
                    })) {
                        self.error(R::B95, "save overwrites an occupied linear slot");
                    }
                    let mut token = before.tokens[value].clone();
                    token.saved_type = self.body.values.get(slot.value).ok().map(|data| data.ty);
                    layout.insert(slot.slot, Slot {
                        token, refs: before.refs[value].clone(),
                        identity: before.identities[value], aliases: before.aliases[value].clone(),
                        requirements: before.full_deps[value].clone(), lifetime: before.lifetimes[value],
                        ..Slot::default()
                    });
                    if !seen.insert(slot.slot) {
                        self.error(R::B95, "frame.save has duplicate slots");
                    }
                    if linear(self.module, self.body, slot.value) && slot.mode != OperandMode::Consume {
                        self.error(R::B95, "linear execution state must move into its frame slot");
                    }
                }
                if let Some(key) = frame_key {
                    flow.frames.insert(key, layout);
                }
                if let Some(slot) = slots.first_mut() {
                    slot.token = before.tokens[frame.as_u32() as usize].clone();
                }
            }
            AsyncOp::FrameResume { frame, slots: saved } => {
                let frame_key = before.tokens[frame.as_u32() as usize].frame;
                let mut layout = frame_key.and_then(|key| before.frames.get(&key).cloned()).unwrap_or_default();
                if frame_key.is_none() {
                    self.error(R::B95, "resume needs the matching saved frame");
                }
                let mut seen = BTreeSet::new();
                for (index, slot) in saved.iter().enumerate() {
                    let actual = self.module.types.get(slot.ty).is_ok_and(|ty| {
                        matches!(ty, Type::Frame(_) | Type::Builder(_) | Type::MapBuilder(..))
                            || matches!(ty, Type::Service(service) if service.is_linear())
                    });
                    if let Some(content) = layout.get(&slot.slot).cloned() {
                        if content.token.saved_type != Some(slot.ty) {
                            self.error(R::B95, "resume slot type disagrees with the actual saved value");
                        }
                        if let Some(result) = slots.get_mut(index + 1) {
                            *result = content;
                        }
                        if actual {
                            layout.remove(&slot.slot);
                        }
                    } else { self.error(R::B95, "resume reads an empty/already moved frame slot"); }
                    if actual != slot.linear || !seen.insert(slot.slot) {
                        self.error(R::B95, "resume slot type/linearity or multiplicity is invalid");
                    }
                }
                if let Some(key) = frame_key {
                    flow.frames.insert(key, layout);
                }
                if let Some(slot) = slots.first_mut() {
                    slot.token = before.tokens[frame.as_u32() as usize].clone();
                }
            }
            AsyncOp::FrameCancel { frame, cleanup, .. } => {
                let mut token = before.tokens[cleanup.as_u32() as usize].clone();
                let Some(fact) = &mut token.cleanup else {
                    self.error(R::B95, "frame cancellation needs a checked cleanup automaton");
                    return;
                };
                if !fact.shield || fact.phase > 1 || fact.awaiting_failure {
                    self.error(R::B95, "frame cancellation must be shielded in the service phase");
                }
                let frame_token = &before.tokens[frame.as_u32() as usize];
                if !frame_token.service.is_some_and(|key| fact.services.remove(&key)) {
                    self.error(R::B95, "cancelled frame is not an outstanding service duty of this owner");
                }
                let mut frames: Vec<_> = frame_token.frame.into_iter().collect();
                if frames.is_empty() {
                    self.error(R::B95, "frame cancellation needs its saved layout");
                }
                let mut seen = BTreeSet::new();
                while let Some(key) = frames.pop() {
                    if !self.state.step(1) {
                        return;
                    }
                    if !seen.insert(key) {
                        continue;
                    }
                    if let Some(layout) = flow.frames.remove(&key) {
                        for content in layout.into_values() {
                            frames.extend(content.token.frame);
                            token.pieces.extend(content.token.pieces.iter().cloned());
                            fact.inventory.extend(content.token.pieces.iter().cloned());
                            if let Some(saved) = &content.token.cleanup {
                                token.pieces.extend(saved.reserve.iter().cloned());
                                fact.inventory.extend(saved.reserve.iter().cloned());
                            }
                            if let Some(scope) = content.token.scope {
                                flow.scopes.insert(scope, 2);
                            }
                        }
                    }
                }
                // The frame's intrinsic cancellation settles its captured duties.
                // Cancellation of a child does not replace the owner's primary failure.
                fact.may_fail = true;
                fact.phase = 1;
                fact.plan = None;
                if let Some(slot) = slots.first_mut() {
                    slot.token = token;
                }
            }
            AsyncOp::FutureNew { packet, owner_state, environment, .. } => {
                let packet = &before.tokens[packet.as_u32() as usize];
                let owner = &before.tokens[owner_state.as_u32() as usize];
                let mut input = packet.clone();
                input.pieces = input.pieces.into_iter().map(|mut piece| {
                    piece.holder = Some(origin);
                    piece
                }).collect();
                let mut token = self.union([input, owner.clone()], true);
                token.scope = owner.scope;
                let mut duty = Piece::new(origin, AdmissionProvenance::CacheOwned);
                duty.owner = owner.scope;
                duty.holder = Some(origin);
                token.pieces.insert(duty);
                flow.cache_keys.insert(origin);
                flow.provenance.insert(origin, provenance_bit(AdmissionProvenance::UnboundResult));
                for (result, slot) in schema.results.iter().zip(slots.iter_mut()) {
                    match result.name {
                        SlotName::To => slot.token = token.clone(),
                        SlotName::State => {
                            slot.token = Token {
                                private: true, cache: Some(origin), scope: owner.scope, ..Token::default()
                            };
                            if let FutureEnvironment::Frame(frame) = environment {
                                slot.token.frame = before.tokens[frame.as_u32() as usize].frame;
                            }
                        }
                        SlotName::Value => {
                            slot.refs.insert(origin);
                            slot.identity = Some(origin);
                        }
                        _ => {}
                    }
                }
            }
            AsyncOp::FuturePublish { packet, cache, .. } => {
                if let Some(slot) = slots.first_mut() {
                    slot.token = before.tokens[packet.as_u32() as usize].clone();
                    slot.token.private = true;
                    slot.token.cache = before.tokens[cache.as_u32() as usize].cache;
                    slot.token.pieces = slot.token.pieces.iter().cloned().map(|mut piece| {
                        piece.provenance = provenance_bit(AdmissionProvenance::CacheOwned);
                        piece.holder = before.tokens[cache.as_u32() as usize].cache;
                        piece
                    }).collect();
                }
            }
            _ => {}
        }
    }

    fn unique_proof(&mut self, block: BlockId, proof: &UniqueCacheProof, before: &Flow) {
        let evidence = &proof.evidence;
        if !proof.region.contains(&block) || !evidence.no_external_storage || !evidence.no_other_waiters
            || !evidence.no_live_views || !evidence.no_collector_retention || !evidence.scheduler_cannot_publish {
            self.error(R::B92, "unique-cache evidence is incomplete");
        }
        let creation = match evidence.creation {
            CreationOrigin::Instruction(inst) => self.body.insts.get(inst).ok().map(|inst| &inst.op),
            CreationOrigin::Invoke { block, outcome: Outcome::Normal, .. } => self.body.blocks.get(block).ok()
                .and_then(|block| if let TermKind::InvokeOwn { op, .. } = &block.term.kind { Some(&**op) }
                    else { None }),
            _ => None,
        };
        let expected_origin = match evidence.creation {
            CreationOrigin::Instruction(inst) => Origin::Instruction(inst, 0),
            CreationOrigin::Invoke { block, slot: 0, .. } => Origin::Invoke(block, 0),
            _ => Origin::External(proof.future),
        };
        if self.identity(before, proof.future) != expected_origin {
            self.error(R::B92, "proof creation does not produce the proved Future");
        }
        if !matches!(creation, Some(Operation::Async(AsyncOp::FutureNew { .. }))) {
            self.error(R::B92, "proof must refer to an actual Future creation");
        }
        let aliases = &before.aliases[proof.future.as_u32() as usize];
        if aliases.iter().any(|alias| *alias != proof.future && !evidence.aliases.contains(alias)) {
            self.error(R::B92, "unique-cache evidence omits an existing alias");
        }
        for (value, _) in self.body.values.iter() {
            if before.availability[value.as_u32() as usize] == AVAILABLE
                && before.tokens[value.as_u32() as usize].unique == Some(self.identity(before, proof.future)) {
                self.error(R::B92, "cache has two simultaneous unique capabilities");
            }
        }
        for (_, candidate) in self.body.blocks.iter() {
            for &id in &candidate.insts {
                if !self.state.step(1) {
                    return;
                }
                let Ok(inst) = self.body.insts.get(id) else {
                    continue;
                };
                let uses = inst.op.operand_refs().iter().any(|operand| {
                    matches!(operand.reference, OperandRef::Value(value) if aliases.contains(&value))
                });
                if uses && !matches!(inst.op, Operation::Async(AsyncOp::UniqueCacheProof { .. }
                    | AsyncOp::AwaitTake { .. } | AsyncOp::FutureDone { .. } | AsyncOp::FutureCancelled { .. })) {
                    self.error(R::B92, "cache is published, retained, shared-awaited or stored in the function");
                }
                if let Operation::Async(AsyncOp::FrameSave { slots, .. }) = &inst.op {
                    for slot in slots {
                        if aliases.contains(&slot.value) && !evidence.frame_slots.contains(&slot.slot) {
                            self.error(R::B92, "coro proof omits a retained frame slot");
                        }
                    }
                }
            }
            if let TermKind::InvokeOwn { op, .. } = &candidate.term.kind {
                let uses = op.operand_refs().iter().any(|operand| {
                    matches!(operand.reference, OperandRef::Value(value) if aliases.contains(&value))
                });
                if uses && !matches!(**op, Operation::Async(AsyncOp::AwaitTake { .. })) {
                    self.error(R::B92, "invoke can publish another observer of the unique cache");
                }
            }
        }
    }
}
