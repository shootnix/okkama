use super::*;

fn parsed(source: &str) -> Module {
    text::parse(source).unwrap_or_else(|error| panic!("{error}"))
}
pub(super) fn checked(source: &str) -> Module {
    let module = parsed(source);
    verify_fixture(&module).unwrap_or_else(|report| panic!("Own: {report:?}"));
    let output = lower_cleanup(module, &LowOptions::default()).unwrap();
    verify_fixture(&output).unwrap_or_else(|report| panic!("Low: {report:?}"));
    let printed = text::print(&output).unwrap();
    let reparsed = parsed(&printed);
    verify_fixture(&reparsed).unwrap_or_else(|report| panic!("round-trip: {report:?}\n{printed}"));
    assert!(text::normalized_eq(&output, &reparsed).unwrap());
    output
}
pub(super) fn body(module: &Module) -> &Body {
    module.functions.iter().find(|(_, function)| function.symbol == "main").unwrap().1.body.as_ref().unwrap()
}
fn body_mut(module: &mut Module) -> &mut Body {
    module.functions.iter_mut().find(|(_, function)| function.symbol == "main").unwrap().1.body.as_mut().unwrap()
}
fn without_roots(mut module: Module) -> Module {
    for (_, function) in module.functions.iter_mut() {
        if let Some(body) = &mut function.body {
            body.cleanup.roots = Arena::new();
        }
    }
    module
}
#[test]
fn four_d3_handwritten_goldens() {
    for (own, golden) in [
        (include_str!("fixtures/end.own.oir"), include_str!("fixtures/end.low.oir")),
        (include_str!("fixtures/end-reserve.own.oir"), include_str!("fixtures/end-reserve.low.oir")),
        (include_str!("fixtures/unwind.own.oir"), include_str!("fixtures/unwind.low.oir")),
        (include_str!("fixtures/unwind-reserve.own.oir"), include_str!("fixtures/unwind-reserve.low.oir")),
    ] {
        let output = checked(own);
        let expected = parsed(golden);
        verify_fixture(&expected).unwrap();
        let output = without_roots(output);
        assert!(text::normalized_eq(&output, &expected).unwrap(), "{}\nexpected:\n{}",
            text::print(&output).unwrap(), text::print(&expected).unwrap());
    }
}
#[test]
fn spec_16_1_keeps_borrowed_view_and_uses_common_merge() {
    let output = checked(include_str!("fixtures/spec16-1.own.oir"));
    let body = body(&output);
    assert_eq!(body.cleanup.closes.len(), 2);
    assert!(body.insts.iter().all(|(_, inst)| !matches!(inst.op,
        Operation::Ownership(OwnershipOp::ScopeEnd { .. } | OwnershipOp::ScopeUnwind { .. }
        | OwnershipOp::End { .. }) | Operation::Lexical(_))));
    assert!(body.cleanup.plans.iter().all(|(_, plan)| plan.sources == [MergeSource::ScopeInventory]));
    assert!(body.cleanup.roots.iter().any(|(_, roots)| roots.bindings.iter()
        .any(|binding| binding.trace == RootTrace::CleanupActivation)));
}
#[test]
fn bounded_and_synchronous_eligibility() {
    let source = include_str!("fixtures/end.own.oir");
    for options in [
        LowOptions { max_entities: 0, ..LowOptions::default() },
        LowOptions { max_steps: 0, ..LowOptions::default() },
        LowOptions { max_generated_blocks: 0, ..LowOptions::default() },
        LowOptions { max_root_bindings: 0, ..LowOptions::default() },
    ] {
        assert!(matches!(lower_cleanup(parsed(source), &options), Err(LowError::LimitExceeded(_))));
    }
    let mut module = parsed(source);
    module.summaries.get_mut(SummaryId::from_u32(0)).unwrap().contract.async_destroy = true;
    assert!(matches!(lower_cleanup(module, &LowOptions::default()), Err(LowError::Unsupported { .. })));
    let mut module = parsed(source);
    module.profile = Profile::High;
    assert!(matches!(lower_cleanup(module, &LowOptions::default()), Err(LowError::InvalidInput { .. })));
}
#[test]
fn metadata_codec_branch_inventory() {
    assert_eq!(CleanupSource::TEXT_VARIANTS.len(), 2);
    assert_eq!(MergeSource::TEXT_VARIANTS.len(), 3);
    assert_eq!(RootBoundary::TEXT_VARIANTS.len(), 4);
    assert_eq!(RootTrace::TEXT_VARIANTS.len(), 9);
}

struct FixtureContracts<'a>(&'a Module);
impl ExternalContracts for FixtureContracts<'_> {
    fn native(&self, name: &str) -> Option<&Signature> {
        self.0.functions.iter().find_map(|(_, function)| function.attrs.iter().any(|attr| {
            matches!(attr, FunctionAttr::Extern(symbol) if symbol == name && symbol.starts_with("fixture."))
        }).then_some(&function.signature))
    }
    fn intrinsic(&self, _: &str) -> Option<&Signature> { None }
    fn native_summary(&self, name: &str) -> Option<&Summary> {
        self.0.functions.iter().find_map(|(_, function)| function.attrs.iter().any(|attr| {
            matches!(attr, FunctionAttr::Extern(symbol) if symbol == name && symbol.starts_with("fixture."))
        }).then_some(function.summary).flatten()).and_then(|id| self.0.summaries.get(id).ok())
    }
}
fn verify_fixture(module: &Module) -> Result<VerifiedModule<'_>, VerifyReport> {
    verify_with(module, &VerifyOptions::default(), VerifyContext {
        external_contracts: &FixtureContracts(module), ..VerifyContext::default()
    })
}

#[test]
fn explicit_defers_preserve_both_risky_call_paths() {
    let source = include_str!("fixtures/spec16-4.own.oir");
    let input = parsed(source);
    let before: Vec<_> = body(&input).blocks.iter().filter_map(|(_, block)| {
        matches!(block.term.kind, TermKind::InvokeDefer { .. }).then_some(block.term.clone())
    }).collect();
    let output = checked(source);
    let after: Vec<_> = body(&output).blocks.iter().filter_map(|(_, block)| {
        matches!(block.term.kind, TermKind::InvokeDefer { .. }).then_some(block.term.clone())
    }).collect();
    assert_eq!(before, after);
    assert_eq!(after.len(), 2);
}
#[test]
fn valued_exit_commits_only_the_successful_reserve() {
    let output = checked(include_str!("fixtures/valued.own.oir"));
    let body = body(&output);
    assert_eq!(body.cleanup.closes.len(), 2);
    assert!(body.cleanup.closes.get(CleanupCloseId::from_u32(0)).unwrap().reserve);
    assert!(body.cleanup.roots.iter().any(|(_, map)| map.bindings.iter()
        .any(|binding| binding.trace == RootTrace::PendingValueReserveDeps)));
}

fn rejected_after_roundtrip(module: &Module, rule: RuleId) {
    let printed = text::print(module).unwrap();
    let reparsed = parsed(&printed);
    for module in [module, &reparsed] {
        let report = verify_fixture(module).unwrap_err();
        assert!(report.diagnostics.iter().any(|diagnostic| diagnostic.rule == rule), "{report:?}");
        assert!(report.diagnostics.iter().all(|diagnostic| diagnostic.primary.entity != EntityLocation::Module));
    }
}
#[test]
fn verifier_rejects_skipped_remainder_early_finish_and_own_only_close() {
    let output = checked(include_str!("fixtures/end-reserve.own.oir"));
    let mut skipped = output.clone();
    let body = body_mut(&mut skipped);
    let finish = body.blocks.iter().find(|(_, block)| block.insts.iter().any(|instruction| {
        matches!(body.insts.get(*instruction).unwrap().op, Operation::Cleanup(CleanupOp::Finish { .. }))
    })).unwrap().0;
    let record = body.blocks.iter_mut().find(|(_, block)| block.insts.iter().any(|instruction| {
        matches!(body.insts.get(*instruction).unwrap().op, Operation::Cleanup(CleanupOp::RecordFailure { .. }))
    })).unwrap().1;
    let TermKind::Br(target) = &mut record.term.kind else { unreachable!() };
    target.block = finish;
    rejected_after_roundtrip(&skipped, RuleId::B101);

    let mut early = output.clone();
    let body = body_mut(&mut early);
    let begin = body.insts.iter().find(|(_, inst)| {
        matches!(inst.op, Operation::Cleanup(CleanupOp::Begin { .. }))
    }).unwrap().1.results[0];
    let plan = body.insts.iter_mut().find(|(_, inst)| {
        matches!(inst.op, Operation::Cleanup(CleanupOp::Plan { .. }))
    }).unwrap().1;
    let Operation::Cleanup(CleanupOp::Plan { cleanup, .. }) = &mut plan.op else { unreachable!() };
    *cleanup = begin;
    rejected_after_roundtrip(&early, RuleId::B101);

    let mut own_only = output.clone();
    let body = body_mut(&mut own_only);
    let (instruction, inst) = body.insts.iter_mut().find(|(_, inst)| {
        matches!(inst.op, Operation::Cleanup(CleanupOp::Begin { .. }))
    }).unwrap();
    let Operation::Cleanup(CleanupOp::Begin { state, effects, .. }) = &inst.op else { unreachable!() };
    inst.op = Operation::Ownership(OwnershipOp::ScopeEnd { state: *state, reserve: None, effects: effects.clone() });
    let report = verify_fixture(&own_only).unwrap_err();
    assert!(report.diagnostics.iter().any(|diagnostic| diagnostic.rule == RuleId::B114
        && matches!(diagnostic.primary.entity, EntityLocation::Instruction { instruction: id, .. }
            if id == instruction)));
    rejected_after_roundtrip(&own_only, RuleId::B114);
}
#[test]
fn verifier_types_dispatch_variants_and_checks_empty_schedule() {
    let output = checked(include_str!("fixtures/end-reserve.own.oir"));
    let mut incomplete = output.clone();
    let body = body_mut(&mut incomplete);
    let term = body.blocks.iter_mut().find(|(_, block)| matches!(block.term.kind, TermKind::ExitDispatch { .. }))
        .unwrap().1;
    let TermKind::ExitDispatch { pending, .. } = &mut term.term.kind else { unreachable!() };
    *pending = None;
    rejected_after_roundtrip(&incomplete, RuleId::T1);

    let mut nonempty = output;
    let body = body_mut(&mut nonempty);
    let param = body.blocks.get(body.entry).unwrap().params[0];
    body.cleanup.schedules.get_mut(CleanupScheduleId::from_u32(0)).unwrap().holders.push(param);
    rejected_after_roundtrip(&nonempty, RuleId::T1);
}

#[test]
fn spec_and_valued_exit_handwritten_cfg_goldens() {
    for (own, low) in [
        (include_str!("fixtures/spec16-1.own.oir"), include_str!("fixtures/spec16-1.low.oir")),
        (include_str!("fixtures/spec16-4.own.oir"), include_str!("fixtures/spec16-4.low.oir")),
        (include_str!("fixtures/valued.own.oir"), include_str!("fixtures/valued.low.oir")),
    ] {
        let actual = without_roots(checked(own));
        let expected = parsed(low);
        verify_fixture(&expected).unwrap();
        assert!(text::normalized_eq(&actual, &expected).unwrap(), "{}\nexpected:\n{}",
            text::print(&actual).unwrap(), text::print(&expected).unwrap());
    }
}

#[test]
fn roots_bind_activation_reserve_failures_and_terminal_exit() {
    let output = checked(include_str!("fixtures/end-reserve.own.oir"));
    let body = body(&output);
    let (begin_id, begin) = body.insts.iter().find(|(_, inst)| {
        matches!(inst.op, Operation::Cleanup(CleanupOp::Begin { .. }))
    }).unwrap();
    let Operation::Cleanup(CleanupOp::Begin { pending: Some(pending), .. }) = begin.op else { unreachable!() };
    let before = body.cleanup.roots.iter().find(|(_, map)| map.boundary == RootBoundary::BeforeInstruction(begin_id))
        .unwrap().1;
    assert!(before.bindings.contains(&RootBinding { value: pending, trace: RootTrace::PendingValueReserveDeps }));
    let after = body.cleanup.roots.iter().find(|(_, map)| map.boundary == RootBoundary::AfterInstruction(begin_id))
        .unwrap().1;
    assert!(!after.bindings.iter().any(|binding| binding.value == pending));
    assert!(after.bindings.contains(&RootBinding {
        value: begin.results[0], trace: RootTrace::CleanupActivation,
    }));
    let (destroy_id, destroy) = body.insts.iter().find(|(_, inst)| {
        matches!(inst.op, Operation::Cleanup(CleanupOp::Destroy { .. }))
    }).unwrap();
    let Operation::Cleanup(CleanupOp::Destroy { cleanup, .. }) = destroy.op else { unreachable!() };
    let after_destroy = body.cleanup.roots.iter().find(|(_, map)| {
        map.boundary == RootBoundary::AfterInstruction(destroy_id)
    }).unwrap().1;
    assert!(!after_destroy.bindings.iter().any(|binding| binding.value == cleanup));
    assert!(after_destroy.bindings.contains(&RootBinding {
        value: destroy.results[0], trace: RootTrace::CleanupActivation,
    }));
    assert!(after_destroy.bindings.contains(&RootBinding {
        value: destroy.results[1], trace: RootTrace::FailureGraph,
    }));
    let (_, finish) = body.insts.iter().find(|(_, inst)| {
        matches!(inst.op, Operation::Cleanup(CleanupOp::Finish { .. }))
    }).unwrap();
    assert!(body.cleanup.roots.iter().any(|(_, map)| map.bindings.contains(&RootBinding {
        value: finish.results[2], trace: RootTrace::ExitReserve,
    })));
}
#[test]
fn borrow_of_external_async_resource_is_allowed() {
    let source = r#"oir 2 own
module borrowed_async
object AsyncRes {
    destroy @destroy
}
summary Md internal parameters [parameter_effect(convention=borrow, destination=none)]
    contract call_contract(suspends=true, async_destroy=true, publication=atomic)
func @destroy(%self: AsyncRes) -> void [extern "fixture.destroy", async, destroy AsyncRes] summary(Md);
scope S action in @main
summary M0 observation parameters [parameter_effect(convention=borrow, destination=none)]
    contract call_contract(publication=atomic)
func @main(%borrowed: AsyncRes facts(managed(deps=d0))) -> void abi observation summary(M0) {
    dependency d0: dependency = retention(value=%borrowed, selection=selection(%borrowed))
^entry in S:
    ret
}
"#;
    let output = checked(source);
    assert!(body(&output).cleanup.closes.is_empty());
}

#[test]
fn packet_end_uses_the_original_boundary_without_a_new_scope() {
    let source = r#"oir 2 own
module packet_end
scope S action in @main
summary M0 internal contract call_contract(failures=[thrown, program, panic, native], publication=atomic)
func @main(%packet: own) -> void summary(M0) {
    boundary B0 call in S
^entry in S:
    %closed: closed = end %packet, boundary B0
    ret
}
"#;
    let output = checked(source);
    assert_eq!(output.scopes.len(), 1);
    let body = body(&output);
    assert_eq!(body.cleanup.closes.get(CleanupCloseId::from_u32(0)).unwrap().source,
        CleanupSource::Packet(BoundaryId::from_u32(0)));
    assert_eq!(body.cleanup.plans.get(CleanupPlanId::from_u32(0)).unwrap().sources,
        [MergeSource::PacketInventory]);
}

#[test]
fn merge_plan_cannot_omit_the_whole_reserve() {
    let mut output = checked(include_str!("fixtures/end-reserve.own.oir"));
    body_mut(&mut output).cleanup.plans.get_mut(CleanupPlanId::from_u32(0)).unwrap().sources.pop();
    rejected_after_roundtrip(&output, RuleId::T1);
}
