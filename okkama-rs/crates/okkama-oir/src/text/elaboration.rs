use super::*;

impl Reader<'_> {
    fn high_instruction(&mut self) -> Result<InstId, Diagnostic> {
        let mut token = self.next()?;
        if let Some(number) = token.text.strip_prefix('#') {
            token.text = format!("i{number}");
        }
        Ok(InstId::from_u32(self.named_reference("inst", &token, true)?))
    }

    pub(super) fn high_result(&mut self, block: BlockId) -> Result<HighResultExit, Diagnostic> {
        self.expect("result.exit")?;
        self.expect("edge")?;
        let edge = HighEdgeSelector::read(self)?;
        self.expect(",")?;
        self.expect("value")?;
        let value = ValueId::read(self)?;
        self.expect(",")?;
        self.expect("demand")?;
        let demand = ResultDemand::read(self)?;
        self.expect(",")?;
        self.expect("prepare")?;
        let prepared = if self.eat("none") { None } else { Some(self.high_instruction()?) };
        Ok(HighResultExit { block, edge, value, demand, prepared })
    }

    pub(super) fn high_effect(&mut self) -> Result<HighEffectRecord, Diagnostic> {
        self.expect("high.effects")?;
        self.expect("at")?;
        let site = if self.eat("terminator") {
            HighOperationSite::Terminator(BlockId::read(self)?)
        } else {
            HighOperationSite::Instruction(self.high_instruction()?)
        };
        self.expect("failures")?;
        let failures = Vec::read(self)?;
        self.expect(",")?;
        self.expect("suspends")?;
        let may_suspend = bool::read(self)?;
        Ok(HighEffectRecord { site, effects: HighEffectEnvelope { failures, may_suspend } })
    }
}

impl Writer<'_> {
    pub(super) fn elaboration_declarations(&mut self, body: &Body) -> Result<(), PrintError> {
        for binding in &body.descriptor_bindings {
            self.push("    descriptor.binding ");
            binding.dictionary.write(self)?;
            self.push(" contract ");
            binding.contract.write(self)?;
            self.push("\n");
        }
        for schedule in &body.service_schedules {
            self.push("    service.schedule ");
            self.word(&schedule.name)?;
            self.push(" owner ");
            schedule.owner.write(self)?;
            self.push(", protocol ");
            schedule.protocol.write(self)?;
            self.push("\n");
        }
        Ok(())
    }

    fn high_instruction(&mut self, instruction: InstId) -> Result<(), PrintError> {
        self.push("#");
        // Canonical instruction labels are iN, in block/instruction order.
        let name = self.names.get(&(self.scope, "inst", instruction.as_u32()))
            .ok_or_else(|| PrintError::new("missing high instruction reference"))?.clone();
        self.push(name.strip_prefix('i').unwrap_or(&name));
        Ok(())
    }

    pub(super) fn high_records(&mut self, body: &Body, block: BlockId) -> Result<(), PrintError> {
        for result in body.high_results.iter().filter(|record| record.block == block) {
            self.push("result.exit edge ");
            result.edge.write(self)?;
            self.push(", value ");
            result.value.write(self)?;
            self.push(", demand ");
            result.demand.write(self)?;
            self.push(", prepare ");
            if let Some(prepared) = result.prepared {
                self.high_instruction(prepared)?;
            } else {
                self.push("none");
            }
            self.push("\n    ");
        }
        for record in &body.high_effects {
            let belongs = match record.site {
                HighOperationSite::Instruction(inst) => body.blocks.get(block)
                    .is_ok_and(|data| data.insts.contains(&inst)),
                HighOperationSite::Terminator(target) => target == block,
            };
            if !belongs { continue; }
            self.push("high.effects at ");
            match record.site {
                HighOperationSite::Instruction(inst) => self.high_instruction(inst)?,
                HighOperationSite::Terminator(target) => {
                    self.push("terminator ");
                    target.write(self)?;
                }
            }
            self.push(" failures ");
            record.effects.failures.write(self)?;
            self.push(", suspends ");
            record.effects.may_suspend.write(self)?;
            self.push("\n    ");
        }
        Ok(())
    }
}
