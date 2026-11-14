use super::tests::{body, body_mut, checked, rejected_after_roundtrip, verify_fixture};
use super::*;

const CASES: &[(&str, &str, &str, bool)] = &[
    ("local-pair", include_str!("fixtures/local-pair.own.oir"),
        include_str!("fixtures/local-pair.low.oir"), true),
    ("singleton", include_str!("fixtures/singleton.own.oir"),
        include_str!("fixtures/singleton.low.oir"), true),
    ("fresh-call", include_str!("fixtures/fresh-call.own.oir"),
        include_str!("fixtures/fresh-call.low.oir"), true),
    ("return-old", include_str!("fixtures/return-old.own.oir"),
        include_str!("fixtures/return-old.low.oir"), false),
    ("reserve-throwing", include_str!("fixtures/reserve-throwing.own.oir"),
        include_str!("fixtures/reserve-throwing.low.oir"), false),
    ("reserve-nonthrowing", include_str!("fixtures/reserve-nonthrowing.own.oir"),
        include_str!("fixtures/reserve-nonthrowing.low.oir"), true),
    ("iteration", include_str!("fixtures/iteration.own.oir"),
        include_str!("fixtures/iteration.low.oir"), true),
    ("rebind-history", include_str!("fixtures/rebind-history.own.oir"),
        include_str!("fixtures/rebind-history.low.oir"), false),
];
#[test]
fn direct_and_merge_goldens() {
    assert_eq!(CreationWitness::TEXT_VARIANTS.len(), 2);
    assert_eq!(HighOperationSite::TEXT_VARIANTS.len(), 2);
    for (name, source, golden, direct) in CASES {
        eprintln!("fixture {name}");
        let mut output = checked(source);
        let main = body(&output);
        assert_eq!(main.cleanup.plans.get(CleanupPlanId::from_u32(0)).unwrap().direct.is_some(), *direct, "{name}");
        for (_, function) in output.functions.iter_mut() {
            if let Some(body) = &mut function.body {
                body.cleanup.roots = Arena::new();
            }
        }
        let expected = text::parse(golden).unwrap();
        verify_fixture(&expected).unwrap();
        check_cleanup_plans(&expected, &LowOptions::default()).unwrap();
        assert!(text::normalized_eq(&output, &expected).unwrap(), "golden {name}");
    }
}

fn consistency_rejected(module: &Module) {
    let printed = text::print(module).unwrap();
    let reparsed = text::parse(&printed).unwrap();
    for module in [module, &reparsed] {
        assert!(check_cleanup_plans(module, &LowOptions::default()).is_err());
    }
}
#[test]
fn rejects_forged_creation_wrong_order_and_bad_suffix() {
    let output = checked(include_str!("fixtures/local-pair.own.oir"));
    let mut forged = output.clone();
    let main = body_mut(&mut forged);
    let instruction = main.blocks.get(main.entry).unwrap().insts[0];
    main.cleanup.proofs.get_mut(CleanupOrderProofId::from_u32(0)).unwrap().slots[0].origin =
        CreationWitness::Create { site: HighOperationSite::Instruction(instruction) };
    consistency_rejected(&forged);
    let mut reordered = output.clone();
    body_mut(&mut reordered).cleanup.proofs.get_mut(CleanupOrderProofId::from_u32(0)).unwrap().slots.swap(0, 1);
    consistency_rejected(&reordered);
    rejected_after_roundtrip(&reordered, RuleId::B101);
    let mut suffix = output;
    let proof = body_mut(&mut suffix).cleanup.proofs.get_mut(CleanupOrderProofId::from_u32(0)).unwrap();
    proof.suffixes[0].remaining.insert(0, proof.slots[0].identity);
    consistency_rejected(&suffix);
}
#[test]
fn rejects_b2_even_for_a_singleton() {
    let mut output = checked(include_str!("fixtures/reserve-nonthrowing.own.oir"));
    let main = body_mut(&mut output);
    main.cleanup.closes.get_mut(CleanupCloseId::from_u32(0)).unwrap().effects.failures.push(FailureKind::Thrown);
    for (_, inst) in main.insts.iter_mut() {
        if let Operation::Cleanup(CleanupOp::Begin { close, effects, .. }) = &mut inst.op
            && *close == CleanupCloseId::from_u32(0) {
            effects.failures.push(FailureKind::Thrown);
        }
    }
    consistency_rejected(&output);
    rejected_after_roundtrip(&output, RuleId::T1);
}
#[test]
fn rejects_old_result_disguised_as_fresh() {
    let mut output = checked(include_str!("fixtures/return-old.own.oir"));
    let callee = output.functions.iter().find(|(_, function)| function.symbol == "old").unwrap().0;
    let summary = output.functions.get(callee).unwrap().summary.unwrap();
    let relation = &mut output.summaries.get_mut(summary).unwrap().results[0];
    relation.aliases.clear();
    relation.fresh.push(CreationSiteId::from_u32(0));
    let main = body_mut(&mut output);
    let (call_block, returned) = main.blocks.iter().find_map(|(id, block)| {
        if let TermKind::InvokeOwn { op, normal, .. } = &block.term.kind
            && matches!(op.as_ref(), Operation::Call(CallOp::Direct { function, .. }) if *function == callee) {
            Some((id, normal.block))
        } else {
            None
        }
    }).unwrap();
    let identity = main.blocks.get(returned).unwrap().params[0];
    let (descriptor, destroy) = main.insts.iter().find_map(|(_, inst)| {
        if let Operation::Core(CoreOp::TypeOf { descriptor, .. }) = &inst.op {
            Some((inst.results[0], descriptor.cleanup))
        } else {
            None
        }
    }).unwrap();
    let close = main.cleanup.closes.get(CleanupCloseId::from_u32(0)).unwrap();
    let state = main.insts.iter().find_map(|(_, inst)| {
        if let Operation::Cleanup(CleanupOp::Begin { close, state, .. }) = inst.op
            && close == CleanupCloseId::from_u32(0) {
            Some(state)
        } else {
            None
        }
    }).unwrap();
    let proof = CleanupOrderProof {
        state, slots: vec![DirectSlot {
            identity, descriptor, destroy, active_at: close.after_defers,
            origin: CreationWitness::FreshResult {
                site: HighOperationSite::Terminator(call_block), callee, summary,
            },
        }],
        preserves: vec![], joins: vec![],
        suffixes: vec![CleanupSuffix { completed: 1, remaining: vec![], roots: vec![] }],
        reserve_empty: true,
    };
    let proof = main.cleanup.proofs.alloc(proof).unwrap();
    main.cleanup.plans.get_mut(CleanupPlanId::from_u32(0)).unwrap().direct = Some(proof);
    consistency_rejected(&output);
}

#[test]
fn identical_joins_keep_evidence_and_ambiguous_presence_uses_merge() {
    let source = include_str!("fixtures/singleton.own.oir")
        .replace("parameters [parameter_effect(convention=borrow, destination=none)]\n    results",
            "parameters [parameter_effect(convention=borrow, destination=none), \
                parameter_effect(convention=borrow, destination=none)]\n    results")
        .replace("%out: Res facts(managed(deps=dout)))", "%out: Res facts(managed(deps=dout)), %choose: bool)");
    let joined = source.replace("    invoke.own scope.end %q1, effects", "\
    cond %choose, ^left(%q1), ^right(%q1)\n\
^left(%ql: scope<S>) in S:\n\
    br ^join(%ql)\n\
^right(%qr: scope<S>) in S:\n\
    br ^join(%qr)\n\
^join(%joined: scope<S>) in S:\n\
    invoke.own scope.end %joined, effects");
    let output = checked(&joined);
    assert!(body(&output).cleanup.proofs.iter().any(|(_, proof)| !proof.joins.is_empty()));
    // A resource exists only on the successful creation edge. Joining with the empty state
    // must not turn this guarded alternative into an unconditional direct slot.
    let ambiguous = source.replace("    %ea: own = own.empty", "\
    cond %choose, ^make(%q0), ^skip(%q0)\n\
^make(%making: scope<S>) in S:\n\
    %ea: own = own.empty")
        .replace("$packet, %q0)", "$packet, %making)")
        .replace("$failure, %q0)", "$failure, %making)")
        .replace("    invoke.own scope.end %q1, effects", "\
    br ^join(%q1)\n\
^skip(%skipped: scope<S>) in S:\n\
    br ^join(%skipped)\n\
^join(%joined: scope<S>) in S:\n\
    invoke.own scope.end %joined, effects");
    let output = checked(&ambiguous);
    assert!(body(&output).cleanup.plans.get(CleanupPlanId::from_u32(0)).unwrap().direct.is_none());
}
#[test]
fn suffix_roots_exclude_dead_current_and_keep_the_next_identity() {
    let output = checked(include_str!("fixtures/local-pair.own.oir"));
    let main = body(&output);
    let proof = main.cleanup.proofs.get(CleanupOrderProofId::from_u32(0)).unwrap();
    let b = proof.slots[0].identity;
    let a = proof.slots[1].identity;
    assert_eq!(proof.suffixes[0].remaining, [a]);
    let destroy = main.insts.iter().find_map(|(id, inst)| {
        matches!(inst.op, Operation::Cleanup(CleanupOp::DestroyDirect { identity, .. }) if identity == b)
            .then_some(id)
    }).unwrap();
    let map = main.cleanup.roots.iter().find(|(_, map)| {
        map.boundary == RootBoundary::AfterInstruction(destroy)
    }).unwrap().1;
    assert!(map.bindings.iter().any(|binding| binding.value == a));
    assert!(!map.bindings.iter().any(|binding| binding.value == b));
}

#[test]
fn throwing_direct_accepts_only_a_proved_empty_reserve() {
    let source = include_str!("fixtures/reserve-throwing.own.oir")
        .replace("    %pending: pending = exit.prepare return %b, %pb at B0", "\
    %q2: scope<S> = own.admit %qb, %pb\n\
    %empty_reserve: own = own.empty\n\
    %pending: pending = exit.prepare return %out, %empty_reserve at B0")
        .replace("scope.end %qb, reserve", "scope.end %q2, reserve");
    let output = checked(&source);
    let main = body(&output);
    let plan = main.cleanup.plans.get(CleanupPlanId::from_u32(0)).unwrap();
    let proof = main.cleanup.proofs.get(plan.direct.unwrap()).unwrap();
    assert!(proof.reserve_empty);
    assert_eq!(proof.slots.len(), 2);
}
#[test]
fn forged_active_join_and_successful_edge_evidence_is_rejected() {
    let output = checked(include_str!("fixtures/fresh-call.own.oir"));
    let mut inactive = output.clone();
    let main = body_mut(&mut inactive);
    let entry = main.entry;
    main.cleanup.proofs.get_mut(CleanupOrderProofId::from_u32(0)).unwrap().slots[0].active_at = entry;
    consistency_rejected(&inactive);
    let mut unwind = output;
    let main = body_mut(&mut unwind);
    let proof = main.cleanup.proofs.get_mut(CleanupOrderProofId::from_u32(0)).unwrap();
    let CreationWitness::FreshResult { site, .. } = &mut proof.slots[0].origin else {
        panic!("expected a verified fresh call");
    };
    let HighOperationSite::Terminator(call) = *site else {
        panic!("expected an owning call");
    };
    let TermKind::InvokeOwn { unwind: failed, .. } = &main.blocks.get(call).unwrap().term.kind else {
        unreachable!();
    };
    *site = HighOperationSite::Terminator(failed.block);
    consistency_rejected(&unwind);
}

#[test]
fn completion_is_the_creation_event_and_survives_codec_without_root_labels() {
    let source = include_str!("fixtures/singleton.own.oir")
        .replace("object Res {", "object Res {\n    field $pending: bool")
        .replace("    %ea: own = own.empty", "    %uninitialized: bool = undef ty bool\n    %ea: own = own.empty")
        .replace("obj.new Res {; %ea} construction complete",
            "obj.new Res { $pending: %uninitialized; %ea } construction incomplete")
        .replace("    invoke.own scope.end %q1, effects", "\
    %q2: scope<S> = obj.init %a, false, field field_ref(declaration=Res, index=0), \
        completion construction_completion(state=%q1)\n\
    invoke.own scope.end %q2, effects");
    let mut output = checked(&source);
    let main = body_mut(&mut output);
    let proof = main.cleanup.proofs.get(CleanupOrderProofId::from_u32(0)).unwrap();
    let CreationWitness::Create { site: HighOperationSite::Instruction(completion) } = proof.slots[0].origin else {
        panic!("allocation is not a creation commit");
    };
    assert!(matches!(main.insts.get(completion).unwrap().op, Operation::Core(CoreOp::ObjInit { .. })));
    main.cleanup.roots = Arena::new();
    let printed = text::print(&output).unwrap();
    let reparsed = text::parse(&printed).unwrap();
    verify_fixture(&reparsed).unwrap();
    check_cleanup_plans(&reparsed, &LowOptions::default()).unwrap();
    assert!(text::normalized_eq(&output, &reparsed).unwrap());
}

pub(super) fn triplet_source() -> String {
    include_str!("fixtures/local-pair.own.oir")
        .replace("    dependency db:",
            "    dependency dh: dependency = retention(value=%h, selection=selection(%h))\n    dependency db:")
        .replace("    invoke.own scope.end %q2, effects", "\
    %eh: own = own.empty\n\
    invoke.own obj.new Res {; %eh} construction complete\n\
        normal ^h($value, $packet, %q2)\n\
        unwind ^create_failed($packet, $failure, %q2)\n\
^h(%h: Res facts(managed(deps=dh)), %ph: own, %qh: scope<S>) in S:\n\
    %q3: scope<S> = own.admit %qh, %ph\n\
    invoke.own scope.end %q3, effects")
}
#[test]
fn h_b_a_direct_chain_has_no_runtime_selector() {
    let output = checked(&triplet_source());
    let main = body(&output);
    let plan = main.cleanup.plans.get(CleanupPlanId::from_u32(0)).unwrap();
    let proof = main.cleanup.proofs.get(plan.direct.unwrap()).unwrap();
    assert_eq!(proof.slots.len(), 3);
    let mut pending = vec![main.cleanup.closes.get(plan.close).unwrap().after_defers];
    let mut seen = std::collections::BTreeSet::new();
    let mut steps = 0;
    while let Some(block) = pending.pop() {
        if !seen.insert(block) {
            continue;
        }
        let data = main.blocks.get(block).unwrap();
        assert!(!matches!(data.term.kind, TermKind::CleanupNext { .. }));
        for instruction in &data.insts {
            let op = &main.insts.get(*instruction).unwrap().op;
            assert!(!matches!(op, Operation::Cleanup(CleanupOp::Next { .. } | CleanupOp::Destroy { .. })));
            if matches!(op, Operation::Cleanup(CleanupOp::DestroyDirect { .. })) {
                steps += 1;
            }
        }
        pending.extend(graph::successors(main, block).unwrap());
    }
    assert_eq!(steps, 3);
}
