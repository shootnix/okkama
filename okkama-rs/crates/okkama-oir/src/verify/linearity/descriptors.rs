use super::*;

impl Engine<'_, '_> {
    pub(super) fn descriptor_operation(&mut self, op: &Operation, origin: Origin, slots: &mut [Slot]) {
        let Operation::Core(CoreOp::TypeOf { ty, descriptor }) = op else {
            return;
        };
        let managed = self.managed.get(ty.as_u32() as usize).copied().unwrap_or(true);
        if descriptor.managed == DescriptorManaged::Plain && managed {
            self.error(R::B111, "type.of cannot certify a managed or residual type as plain");
        }
        let effects = super::super::effects::cleanup(self.module, self.body, *ty, self.state);
        if effects.suspends && !descriptor.async_destroy {
            self.error(R::B112, "descriptor hides asynchronous cleanup in its complete type graph");
        }
        let cleanup = self.module.functions.get(descriptor.cleanup).ok();
        if cleanup.is_none_or(|function| function.summary != Some(descriptor.summary)) {
            self.error(R::B111, "descriptor cleanup and dictionary summary must describe the same entry");
        }
        if managed {
            let nominal = self.module.types.get(*ty).ok().and_then(|ty| match ty {
                Type::Object { decl, .. } => self.module.declarations.get(*decl).ok(),
                _ => None,
            }).and_then(|decl| match decl.kind {
                DeclarationKind::Object { destroy, .. } => destroy,
                _ => None,
            });
            let explicit = cleanup.is_some_and(|function| function.signature.params.iter().any(|parameter| {
                self.module.types.get(*parameter) == Ok(&Type::Service(ServiceType::Own))
            }) && function.summary.and_then(|id| self.module.summaries.get(id).ok())
                .is_some_and(|summary| summary.entry_kind == EntryKind::CallBoundary));
            if nominal != Some(descriptor.cleanup) && !explicit {
                self.error(R::B111, "managed descriptor needs its declared destroy or a checked packet cleanup entry");
            }
        }
        if let Origin::Instruction(inst, _) = origin && let Some(slot) = slots.first_mut() {
            slot.token.dictionary = Some(DictionarySource::Instruction(inst));
        }
    }

    pub(super) fn descriptor_arguments(&mut self, call: &CallSite, target: Option<FuncId>, flow: &Flow) {
        let Some(body) = target.and_then(|id| self.module.functions.get(id).ok())
            .and_then(|function| function.body.as_ref()) else {
            return;
        };
        let Ok(entry) = body.blocks.get(body.entry) else {
            return;
        };
        let dictionaries: Vec<_> = entry.params.iter().copied().filter(|value| {
            super::super::later::value_type(self.module, body, *value) == Some(&Type::Descriptor)
        }).collect();
        for binding in &body.descriptor_bindings {
            let Some(index) = dictionaries.iter().position(|value| *value == binding.dictionary) else {
                continue;
            };
            let actual = call.descriptors.get(index).and_then(|value| flow.tokens[value.as_u32() as usize].dictionary)
                .and_then(|source| match source {
                    DictionarySource::Instruction(inst) => self.body.insts.get(inst).ok().and_then(|inst| {
                        if let Operation::Core(CoreOp::TypeOf { descriptor, .. }) = &inst.op { Some(descriptor) }
                        else { None }
                    }),
                    DictionarySource::Parameter(value) => self.body.descriptor_bindings.iter()
                        .find(|binding| binding.dictionary == value).map(|binding| &binding.contract),
                });
            let expected = &binding.contract;
            if let SummarySelection::Descriptor(parameter) = expected.retention {
                let expected_type = call.type_args.get(usize::from(parameter)).copied();
                let source = call.descriptors.get(index)
                    .and_then(|value| flow.tokens[value.as_u32() as usize].dictionary);
                let matches_type = match source {
                    Some(DictionarySource::Instruction(inst)) => self.body.insts.get(inst).is_ok_and(|inst| {
                        matches!(inst.op, Operation::Core(CoreOp::TypeOf { ty, .. }) if Some(ty) == expected_type)
                    }),
                    Some(DictionarySource::Parameter(value)) => self.body.descriptor_bindings.iter()
                        .find(|binding| binding.dictionary == value).is_some_and(|binding| {
                            matches!(binding.contract.retention, SummarySelection::Descriptor(index)
                                if expected_type.is_some_and(|ty| matches!(self.module.types.get(ty),
                                    Ok(Type::Parameter(parameter)) if parameter.index == u32::from(index)
                                        && parameter.owner == GenericOwner::Function(self.function))))
                        }),
                    None => false,
                };
                if !matches_type { self.error(R::B111, "dictionary describes a different generic substitution"); }
            }
            if actual.is_none_or(|actual| {
                expected.managed != DescriptorManaged::Runtime && actual.managed != expected.managed
                    || actual.async_destroy && !expected.async_destroy
            }) {
                self.error(R::B111, "call dictionary does not establish the callee's plain/cleanup contract");
            }
        }
    }
}
