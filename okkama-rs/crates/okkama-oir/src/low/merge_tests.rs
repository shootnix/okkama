use super::tests::{body, body_mut, checked, rejected_after_roundtrip, verify_fixture};
use super::*;

#[test]
fn merge_inventory_goldens() {
    for (source, golden) in [
        (include_str!("fixtures/mixed-journal.own.oir"),
            include_str!("fixtures/mixed-journal.low.oir")),
        (include_str!("fixtures/mixed-protocol.own.oir"),
            include_str!("fixtures/mixed-protocol.low.oir")),
        (include_str!("fixtures/two-scope-end-reserve.own.oir"),
            include_str!("fixtures/two-scope-end-reserve.low.oir")),
        (include_str!("fixtures/two-scope-unwind-reserve.own.oir"),
            include_str!("fixtures/two-scope-unwind-reserve.low.oir")),
        (include_str!("fixtures/replacement-rest.own.oir"),
            include_str!("fixtures/replacement-rest.low.oir")),
        (include_str!("fixtures/two-scope-suffix.own.oir"),
            include_str!("fixtures/two-scope-suffix.low.oir")),
    ] {
        let output = checked(source);
        let expected = text::parse(golden).unwrap_or_else(|error| panic!("golden: {error}"));
        verify_fixture(&expected).unwrap_or_else(|report| panic!("golden: {report:?}"));
        assert_eq!(canonical_roots(output.clone()), canonical_roots(expected), "golden {}", output.name);
        assert!(body(&output).cleanup.plans.iter().all(|(_, plan)| plan.direct.is_none()));
    }
}

// Root bindings are sets. Compare them in definition order without making arena allocation order
// part of the semantic golden contract. CFG, plans, boundary maps and all operations remain exact.
fn canonical_roots(mut module: Module) -> String {
    for (_, function) in module.functions.iter_mut() {
        let Some(body) = &mut function.body else {
            continue;
        };
        let mut order = std::collections::BTreeMap::new();
        for (_, block) in body.blocks.iter() {
            for value in block.params.iter().chain(block.insts.iter().flat_map(|id| {
                &body.insts.get(*id).unwrap().results
            })) {
                order.insert(*value, order.len());
            }
        }
        for (_, map) in body.cleanup.roots.iter_mut() {
            map.bindings.sort_by_key(|binding| order[&binding.value]);
        }
    }
    text::print(&module).unwrap()
}

#[test]
fn journals_are_inventory_and_only_declared_dynamic_sites_enter_the_census() {
    for (source, expected) in [
        (include_str!("fixtures/mixed-journal.own.oir"), 0),
        (include_str!("fixtures/mixed-protocol.own.oir"), 1),
    ] {
        let output = checked(source);
        let verified = verify_fixture(&output).unwrap();
        assert_eq!(verified.stats().dynamic.census.module_count(DynamicReason::Merge), expected);
        assert_eq!(verified.stats().dynamic.sites.len(), expected as usize);
        assert!(body(&output).insts.iter().any(|(_, inst)| {
            matches!(inst.op, Operation::Cleanup(CleanupOp::Destroy { .. }))
        }));
    }
}

#[test]
fn rejects_journal_static_double_destroy_and_lost_history() {
    let golden = include_str!("fixtures/mixed-journal.low.oir");
    let duplicate = golden.replace(
        "none ^m0_next(%m0_after)", "none ^duplicate(%m0_after)");
    let duplicate = duplicate.trim_end().strip_suffix('}').unwrap().to_owned() + "\n\
^duplicate(%repeat: cleanup-state) in S:\n\
    (%repeated: cleanup-state, %repeat_failure: failure-state) = \
        destroy.direct %repeat, %a, %descriptor, destroy @destroy\n\
    failure.dispatch %repeat_failure none ^m0_next(%repeated) some ^m0_record(%repeated, $failure)\n}\n";
    let duplicate = text::parse(&duplicate).unwrap();
    rejected_after_roundtrip(&duplicate, RuleId::B102);

    let mut lost = checked(include_str!("fixtures/mixed-journal.own.oir"));
    let main = body_mut(&mut lost);
    let start = main.cleanup.closes.get(CleanupCloseId::from_u32(0)).unwrap().after_defers;
    let finish = main.blocks.iter().find_map(|(id, block)| {
        block.insts.iter().any(|instruction| matches!(main.insts.get(*instruction).unwrap().op,
            Operation::Cleanup(CleanupOp::Finish { .. }))).then_some(id)
    }).unwrap();
    let TermKind::Br(target) = main.blocks.get(start).unwrap().term.kind.clone() else {
        unreachable!();
    };
    let condition = main.blocks.get(main.entry).unwrap().params[1];
    main.blocks.get_mut(start).unwrap().term.kind = TermKind::Cond {
        condition, yes: Target { block: finish, args: target.args.clone() }, no: target,
    };
    rejected_after_roundtrip(&lost, RuleId::B102);
}

#[test]
fn rejects_a_reserve_merged_twice() {
    let mut duplicate = checked(include_str!("fixtures/two-scope-end-reserve.own.oir"));
    let main = body_mut(&mut duplicate);
    let (close, pending) = main.insts.iter().find_map(|(_, inst)| match inst.op {
        Operation::Cleanup(CleanupOp::Begin { pending: Some(pending), close, .. }) => Some((close, pending)),
        _ => None,
    }).unwrap();
    let plan = main.insts.iter_mut().find(|(_, inst)| matches!(inst.op,
        Operation::Cleanup(CleanupOp::Plan { .. }))).unwrap().1;
    let Operation::Cleanup(CleanupOp::Plan { cleanup, .. }) = plan.op else {
        unreachable!();
    };
    plan.op = Operation::Exit(ExitOp::Abort { pending, cleanup });
    assert!(main.cleanup.closes.get(close).unwrap().reserve);
    rejected_after_roundtrip(&duplicate, RuleId::B101);

    let mut repeated_source = checked(include_str!("fixtures/two-scope-end-reserve.own.oir"));
    body_mut(&mut repeated_source).cleanup.plans.get_mut(CleanupPlanId::from_u32(0)).unwrap()
        .sources.push(MergeSource::WholeReserveOnFailure);
    rejected_after_roundtrip(&repeated_source, RuleId::T1);
}

#[test]
fn roots_are_required_at_every_boundary_even_without_suspension() {
    let output = checked(include_str!("fixtures/end.own.oir"));
    let output = text::parse(&text::print(&output).unwrap()).unwrap();
    let function = output.functions.iter().find(|(_, function)| function.symbol == "main").unwrap().0;
    let count = body(&output).cleanup.roots.len();
    for missing in 0..count {
        let boundary = &body(&output).cleanup.roots.get(RootMapId::from_u32(missing as u32)).unwrap().boundary;
        let location = match *boundary {
            RootBoundary::BeforeInstruction(instruction) | RootBoundary::AfterInstruction(instruction) => {
                EntityLocation::Instruction { function, instruction }
            }
            RootBoundary::Entry(block) => EntityLocation::Block { function, block },
            RootBoundary::Terminator(block) => EntityLocation::Terminator { function, block },
        };
        let mut module = output.clone();
        let main = body_mut(&mut module);
        let mut maps = Arena::new();
        for (index, (_, map)) in main.cleanup.roots.iter().enumerate() {
            if index != missing {
                maps.alloc(map.clone()).unwrap();
            }
        }
        main.cleanup.roots = maps;
        rejected_after_roundtrip(&module, RuleId::B105);
        let reparsed = text::parse(&text::print(&module).unwrap()).unwrap();
        for module in [&module, &reparsed] {
            let report = verify_fixture(module).unwrap_err();
            assert!(report.diagnostics.iter().any(|diagnostic| {
                diagnostic.rule == RuleId::B105 && diagnostic.primary.entity == location
            }), "{report:?}");
        }
    }
    let output = checked(include_str!("fixtures/replacement-rest.own.oir"));
    for trace in [RootTrace::PendingValueReserveDeps, RootTrace::ScopeInventory,
        RootTrace::PacketInventory, RootTrace::FullDependencies, RootTrace::CleanupActivation,
        RootTrace::FailureGraph, RootTrace::ExitReserve] {
        let mut module = output.clone();
        let binding = body_mut(&mut module).cleanup.roots.iter_mut().find_map(|(_, map)| {
            map.bindings.iter().position(|binding| binding.trace == trace).map(|index| (map, index))
        }).unwrap();
        binding.0.bindings.remove(binding.1);
        rejected_after_roundtrip(&module, RuleId::B105);
    }
}

#[test]
fn storage_protocol_outcomes_are_checked_by_producer_and_verifier() {
    let source = include_str!("fixtures/mixed-protocol.own.oir");
    for mutation in 0..3 {
        let mut input = text::parse(source).unwrap();
        let protocol = input.protocols.get_mut(ProtocolId::from_u32(0)).unwrap();
        match mutation {
            0 => protocol.rules[0].unwind.as_mut().unwrap().state = HolderState::Accepted,
            1 => protocol.rules[0].cancel = protocol.rules[0].unwind,
            _ => protocol.rules.push(protocol.rules[0].clone()),
        }
        assert!(matches!(lower_cleanup(input, &LowOptions::default()), Err(LowError::Unsupported { .. })));
        let mut output = checked(source);
        let protocol = output.protocols.get_mut(ProtocolId::from_u32(0)).unwrap();
        match mutation {
            0 => protocol.rules[0].unwind.as_mut().unwrap().state = HolderState::Accepted,
            1 => protocol.rules[0].cancel = protocol.rules[0].unwind,
            _ => protocol.rules.push(protocol.rules[0].clone()),
        }
        rejected_after_roundtrip(&output, RuleId::B71);
    }
}


#[test]
fn duplicate_boundary_maps_and_untyped_service_roots_are_rejected() {
    let output = checked(include_str!("fixtures/end.own.oir"));
    let mut duplicate = output.clone();
    let main = body_mut(&mut duplicate);
    let map = main.cleanup.roots.get(RootMapId::from_u32(0)).unwrap().clone();
    main.cleanup.roots.alloc(map).unwrap();
    rejected_after_roundtrip(&duplicate, RuleId::B105);
    let mut untyped = output;
    let main = body_mut(&mut untyped);
    let binding = main.cleanup.roots.iter_mut().flat_map(|(_, map)| &mut map.bindings)
        .find(|binding| binding.trace == RootTrace::CleanupActivation).unwrap();
    binding.trace = RootTrace::Value(main.values.get(binding.value).unwrap().ty);
    rejected_after_roundtrip(&untyped, RuleId::B105);
}
