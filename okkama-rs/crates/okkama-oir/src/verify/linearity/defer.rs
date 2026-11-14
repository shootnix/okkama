use super::*;

impl Engine<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn defer_invoke(
        &mut self, block: BlockId, function: FuncId, call: &CallSite, continued: &InvokeEdge,
        replacements: &[DeferReplaceEdge], unwind: Option<&InvokeEdge>, before: &Flow,
        outgoing: &mut Vec<(BlockId, Flow)>,
    ) {
        let mut shielded = call.clone();
        shielded.cleanup = CallCleanupMode::Shielded;
        let op = Operation::Call(CallOp::Direct { function, call: shielded });
        let registry = ContractRegistry::new(self.module);
        let Ok(contract) = registry.construction_schema(&op, self.body) else {
            return;
        };
        let arms = std::iter::once((Outcome::DeferContinue, continued))
            .chain(replacements.iter().enumerate().map(|(index, edge)| {
                (Outcome::DeferReplace(index as u16), &edge.edge)
            }))
            .chain(unwind.map(|edge| (Outcome::Unwind, edge)));
        for (outcome, edge) in arms {
            let Ok(results) = registry.defer_results(function, outcome) else {
                continue;
            };
            let mut schema = if outcome == Outcome::Unwind {
                contract.unwind.clone().unwrap_or_else(|| contract.normal.clone())
            } else { contract.normal.clone() };
            schema.results = results;
            if outcome == Outcome::DeferContinue {
                schema.transfers = vec![TransferRule::Settle { inputs: call.packet.into_iter().collect() }];
            }
            let mut branch = before.clone();
            let opaque = if let Outcome::DeferReplace(index) = outcome {
                Some(Origin::Invoke(block, u32::MAX - u32::from(index)))
            } else { None };
            if let Some(origin) = opaque {
                branch.retire_origin(origin);
            }
            let normal = if outcome == Outcome::Unwind { Outcome::Unwind } else { Outcome::Normal };
            let mut slots = self.operation(block, &op, &schema, normal, Origin::Invoke(block, 0), &mut branch);
            if let Some(key) = opaque {
                self.defer_replacement(&schema, key, call, before, &mut slots);
            }
            if before.active_failure != 0 {
                branch.active_failure = before.active_failure;
                branch.active_failure_origin = before.active_failure_origin;
            }
            outgoing.push(self.edge(&branch, edge.block, &edge.args, &slots, R::B101));
        }
    }
}

impl Engine<'_, '_> {
    pub(super) fn defer_return(&mut self, tag: &DeferReturnTag, values: &[ValueId], flow: &Flow) {
        if matches!(tag, DeferReturnTag::Continue) {
            return;
        }
        let language: Vec<_> = values.iter().copied().filter(|value| !matches!(
            super::super::later::value_type(self.module, self.body, *value), Some(Type::Service(_)))).collect();
        let keys = language.iter().flat_map(|value| flow.refs[value.as_u32() as usize].iter().copied()).collect();
        let labels = language.iter().flat_map(|value| flow.contained[value.as_u32() as usize].iter().copied())
            .collect();
        for &value in values {
            if super::super::later::value_type(self.module, self.body, value)
                != Some(&Type::Service(ServiceType::Own)) { continue; }
            for piece in &flow.tokens[value.as_u32() as usize].pieces {
                if !self.subset(piece, &keys, &labels) {
                    self.error(R::B101, "defer may export only its actual replacement's retained packet");
                }
            }
        }
    }

    fn defer_replacement(
        &mut self, schema: &OutcomeContract, key: Origin, call: &CallSite, flow: &Flow, slots: &mut [Slot],
    ) {
        let managed = schema.results.iter().any(|slot| match slot.ty {
            ResultType::Exact(ty) => self.managed.get(ty.as_u32() as usize).copied().unwrap_or(true),
            _ => false,
        });
        let external: BTreeSet<_> = call.args.iter().flat_map(|arg| {
            flow.full_deps[arg.value.as_u32() as usize].iter().copied()
        }).collect();
        for (info, slot) in schema.results.iter().zip(slots) {
            match info.name {
                SlotName::Value if managed => {
                    slot.refs = BTreeSet::from([key]);
                    slot.requirements = external.clone();
                    slot.identity = Some(key);
                }
                SlotName::Packet => {
                    slot.token = Token::default();
                    if managed {
                        let mut piece = Piece::new(key, AdmissionProvenance::Bound);
                        piece.family = true;
                        piece.predicates.insert(Filter {
                            keys: BTreeSet::from([key]), fresh: false, possible: BTreeSet::new(),
                            labels: BTreeSet::new(), stale: false, whole_future: false, state: 0,
                        }, true);
                        slot.token.pieces.insert(piece);
                    }
                }
                SlotName::Deps => {
                    slot.refs = external.clone();
                    if managed {
                        slot.refs.insert(key);
                    }
                }
                _ => {}
            }
        }
    }
}
