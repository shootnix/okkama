use okkama_oir::{text, verify::*, *};
mod support;

fn fixture_verify(module: &Module) -> Result<(), VerifyReport> {
    let context = VerifyContext {
        external_contracts: &support::FixtureContracts(module),
        ..VerifyContext::default()
    };
    verify_with(module, &VerifyOptions::default(), context).map(|_| ())
}

fn roundtrip(source: &str) -> Module {
    let module = text::parse(source).unwrap();
    text::parse(&text::print(&module).unwrap()).unwrap()
}

fn rejected(module: &Module, rule: RuleId) -> VerifyReport {
    let report = fixture_verify(module).unwrap_err();
    assert_eq!(report.failure, VerifyFailure::Rejected, "{report:?}");
    assert!(report.diagnostics.iter().any(|diagnostic| diagnostic.rule == rule), "{report:?}");
    report
}

#[test]
fn loops_conserve_packets_for_zero_one_and_many_iterations() {
    let prefix = "oir 2 own\nmodule loops\nscope S action in @f\n\
        summary M0 internal parameters [parameter_effect(convention=borrow, destination=none)]\n\
        contract call_contract(failures=[thrown, program, panic, native, cancelled], publication=atomic)\n\
        func @f(%again: bool, %packet: own) -> void summary(M0) {\n\
        boundary B0 call in S\n^entry in S:\n";
    for iterations in [0, 1, 32] {
        let mut source = prefix.to_owned();
        source.push_str("    br ^step0(%packet)\n");
        for step in 0..iterations {
            source.push_str(&format!("^step{step}(%p{step}: own) in S:\n"));
            source.push_str(&format!("    %m{step}: own = own.move %p{step}\n"));
            source.push_str(&format!("    br ^step{}(%m{step})\n", step + 1));
        }
        source.push_str(&format!("^step{iterations}(%last: own) in S:\n"));
        source.push_str("    %closed: closed = end %last, boundary B0\n    ret\n}\n");
        let module = roundtrip(&source);
        verify(&module).unwrap();
        let invalid = source.replace("    ret\n", "    %again_closed: closed = end %last, boundary B0\n    ret\n");
        rejected(&roundtrip(&invalid), RuleId::B21);
    }
    let loop_body = "    br ^header(%packet)\n\
        ^header(%current: own) in S:\n\
            cond %again, ^body(%current), ^done(%current)\n\
        ^body(%iteration: own) in S:\n\
            %next: own = own.move %iteration\n\
            br ^header(%next)\n\
        ^done(%remaining: own) in S:\n\
            %closed: closed = end %remaining, boundary B0\n    ret\n}\n";
    let source = format!("{prefix}{loop_body}");
    verify(&roundtrip(&source)).unwrap();
    rejected(&roundtrip(&source.replace("own.move %iteration", "own.move %packet")), RuleId::B21);
    let rebind = include_str!("fixtures/verify/valid/B4-rebind-loop-and-staging.oir");
    verify(&roundtrip(rebind)).unwrap();
    let bad = include_str!("fixtures/verify/invalid/B4-rebind-loses-retired.oir");
    rejected(&roundtrip(bad), RuleId::B43);
}

#[test]
fn each_linear_result_slot_is_required_independently() {
    // Drop uses, not definitions: each mutation remains structurally typed SSA.
    let split = include_str!("fixtures/verify/valid/SSA1-multi-result-definitions.oir");
    verify(&roundtrip(split)).unwrap();
    for lost in ["%m", "%n"] {
        let other = if lost == "%m" { "%n" } else { "%m" };
        let source = split.replace("own.join [%m, %n]", &format!("own.join [{other}]"));
        rejected(&roundtrip(&source), RuleId::B22);
    }
    let invoke = include_str!("fixtures/verify/valid/checked-split-normal-and-unwind.oir");
    verify(&roundtrip(invoke)).unwrap();
    for (full, survivor) in [("[%r, %s]", "[%r]"), ("[%r, %s]", "[%s]")] {
        rejected(&roundtrip(&invoke.replace(full, survivor)), RuleId::B22);
    }
    // Buffer and journal are separate linear slots on both normal and failure outcomes.
    let builder = include_str!("fixtures/verify/valid/builder-push-failure-preserves-buffer.oir");
    let positive = roundtrip(builder);
    fixture_verify(&positive).unwrap();
    let mut exercised = 0;
    for (function, data) in positive.functions.iter() {
        let Some(body) = &data.body else {
            continue;
        };
        for (block_id, block) in body.blocks.iter() {
            let TermKind::InvokeOwn { op, normal, unwind } = &block.term.kind else {
                continue;
            };
            let Operation::Collection(CollectionOp::BuilderPush { buffer, packet: Some(packet), .. }) =
                op.as_ref() else {
                    continue;
                };
            for (is_unwind, edge) in [(false, normal), (true, unwind)] {
                let target = body.blocks.get(edge.block).unwrap();
                for (index, argument) in edge.args.iter().enumerate() {
                    let EdgeArg::Produced(_) = argument else {
                        continue;
                    };
                    let value = body.values.get(target.params[index]).unwrap();
                    let ty = positive.types.get(value.ty).unwrap();
                    if !matches!(ty, Type::Builder(_) | Type::Service(ServiceType::Own)) {
                        continue;
                    }
                    let mut negative = positive.clone();
                    let body = negative.functions.get_mut(function).unwrap().body.as_mut().unwrap();
                    let TermKind::InvokeOwn { normal, unwind, .. } =
                        &mut body.blocks.get_mut(block_id).unwrap().term.kind else {
                            unreachable!();
                        };
                    let edge = if is_unwind { unwind } else { normal };
                    // Substituting the old, correctly typed input loses exactly this outcome slot.
                    let old = if matches!(ty, Type::Builder(_)) { *buffer } else { *packet };
                    edge.args[index] = EdgeArg::Value(old);
                    let negative = roundtrip(&text::print(&negative).unwrap());
                    let report = rejected(&negative, RuleId::B62);
                    assert!(!report.diagnostics.iter().any(|diagnostic| diagnostic.rule == RuleId::S3));
                    exercised += 1;
                }
            }
        }
    }
    assert_eq!(exercised, 4);
}

#[test]
fn deep_cfg_is_checked_on_a_small_stack_without_path_recursion() {
    std::thread::Builder::new().stack_size(256 * 1024).spawn(|| {
        let mut source = "oir 2 high\nmodule deep\nscope S action in @f\n\
            func @f() -> void {\n^entry in S:\n    br ^b0\n".to_owned();
        for index in 0..4096 {
            source.push_str(&format!("^b{index} in S:\n    br ^b{}\n", index + 1));
        }
        source.push_str("^b4096 in S:\n    ret\n}\n");
        let module = roundtrip(&source);
        let verified = verify(&module).unwrap();
        assert!(verified.stats().steps < VerifyLimits::default().steps);
    }).unwrap().join().unwrap();
}

#[test]
fn shared_metadata_dag_is_bounded_when_it_is_used() {
    let mut source = "oir 2 own\nmodule shared\nscope S action in @f\n\
        summary M0 internal parameters [parameter_effect(convention=borrow, destination=none)]\n\
        contract call_contract(failures=[thrown, program, panic, native, cancelled], publication=atomic)\n\
        func @f(%v: array<any> facts(managed(deps=d40))) -> void summary(M0) {\n\
        key k0: key = created(site=0, instance=%v)\n\
        dependency d0: dependency = key(k0)\n".to_owned();
    for index in 1..=40 {
        source.push_str(&format!("    dependency d{index}: dependency = union([d{}, d{}])\n",
            index - 1, index - 1));
    }
    source.push_str("^entry in S:\n    %size: int = array.len %v\n    ret\n}\n");
    let module = roundtrip(&source);
    assert!(verify(&module).unwrap().stats().steps < 20_000);
    let options = VerifyOptions {
        limits: VerifyLimits { expression_nodes_per_function: 20, ..VerifyLimits::default() },
        ..VerifyOptions::default()
    };
    let report = verify_with(&module, &options, VerifyContext::default()).unwrap_err();
    assert_eq!(report.failure, VerifyFailure::Incomplete);
}

#[test]
fn structural_inventory_is_exact_and_reports_suspension_gaps() {
    assert_eq!(StructuralCheck::ALL, &[
        StructuralCheck::B95FrameLayoutAndContinuations,
        StructuralCheck::B105SuspensionRootMaps,
        StructuralCheck::B96CollectorPositions,
    ]);
    let source = include_str!("fixtures/verify/valid/B9-unique-temporary-take.oir");
    let module = roundtrip(source);
    let verified = verify_coro(&module).unwrap();
    for check in [StructuralCheck::B95FrameLayoutAndContinuations, StructuralCheck::B105SuspensionRootMaps] {
        assert!(verified.stats().structural_checks.contains(&check));
    }
}

#[test]
fn untaken_irreducible_and_parallel_edges_are_checked() {
    let untaken = include_str!("fixtures/verify/invalid/B6-error-only-on-untaken-branch.oir");
    rejected(&roundtrip(untaken), RuleId::SSA2);
    let irreducible = include_str!("fixtures/verify/valid/SSA2-irreducible-CFG.oir");
    verify(&roundtrip(irreducible)).unwrap();
    let normal = include_str!("fixtures/verify/valid/SSA2-several-edges-one-target.oir");
    verify(&roundtrip(normal)).unwrap();
    let source = "oir 2 own
module parallel
scope S action in @f
summary M0 internal parameters [parameter_effect(convention=borrow, destination=none)]
    contract call_contract(failures=[thrown, program, panic, native, cancelled], publication=atomic)
func @f(%x: int, %active: failure) -> void summary(M0) {
^entry in S:
    invoke.own int.mod %x, %x
        normal ^join($value, %active)
        unwind ^join(%x, $failure)
^join(%result: int, %failure: failure) in S:
    raise %failure
}
";
    verify(&roundtrip(source)).unwrap();
    let bad = source.replace("normal ^join($value, %active)", "normal ^join(%x, %active)");
    // The non-linear scalar slot may be discarded; both semantic edges still must type-check.
    verify(&roundtrip(&bad)).unwrap();
    let bad = source.replace("unwind ^join(%x, $failure)", "unwind ^join(true, $failure)");
    rejected(&roundtrip(&bad), RuleId::T3);
}

#[test]
fn all_suppresses_poisoned_prerequisites_but_keeps_independent_errors() {
    let source = "oir 2 high
module poisoned
scope S action in @broken
scope Sother action in @independent
func @broken(%x: int) -> void {
^entry in S:
    %v: int = int.add %x, %x
    ret
}
func @independent(%x: int) -> void {
^entry in Sother:
    %v: int = int.add %x, %x
    ret
}
";
    let mut module = roundtrip(source);
    let broken = FuncId::from_u32(0);
    let independent = FuncId::from_u32(1);
    let body = module.functions.get_mut(broken).unwrap().body.as_mut().unwrap();
    body.insts.get_mut(InstId::from_u32(0)).unwrap().op = Operation::Core(CoreOp::IntAdd {
        left: ValueId::from_u32(u32::MAX),
        right: ValueId::from_u32(u32::MAX),
    });
    let boolean = module.types.intern(Type::Bool).unwrap();
    let body = module.functions.get_mut(independent).unwrap().body.as_mut().unwrap();
    body.values.get_mut(ValueId::from_u32(1)).unwrap().ty = boolean;
    let all = rejected(&module, RuleId::S3);
    assert!(all.diagnostics.iter().any(|diagnostic| diagnostic.rule == RuleId::T2));
    assert!(all.diagnostics.iter().all(|diagnostic| match diagnostic.primary.entity {
        EntityLocation::Instruction { function, .. } if function == broken => diagnostic.rule == RuleId::S3,
        EntityLocation::Operand { site: OperationSite::Instruction { function, .. }, .. }
            if function == broken => false,
        _ => true,
    }), "{all:?}");
    let options = VerifyOptions { errors: ErrorPolicy::First, ..VerifyOptions::default() };
    let first = verify_with(&module, &options, VerifyContext::default()).unwrap_err();
    assert_eq!(first.diagnostics, all.diagnostics[..1]);
}

#[test]
fn every_represented_linear_invoke_slot_is_lost_one_at_a_time() {
    use std::{collections::BTreeSet, fs, path::Path};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/verify/valid");
    let mut paths: Vec<_> = fs::read_dir(root).unwrap().map(|entry| entry.unwrap().path()).collect();
    paths.sort();
    let mut exercised = BTreeSet::new();
    for path in paths {
        let source = fs::read_to_string(&path).unwrap();
        let positive = roundtrip(&source);
        for (function, data) in positive.functions.iter() {
            let Some(body) = &data.body else {
                continue;
            };
            for (block_id, block) in body.blocks.iter() {
                let TermKind::InvokeOwn { op, normal, unwind } = &block.term.kind else {
                    continue;
                };
                for (is_unwind, edge) in [(false, normal), (true, unwind)] {
                    if edge.args.len() < 2 {
                        continue;
                    }
                    let target = body.blocks.get(edge.block).unwrap();
                    for (index, argument) in edge.args.iter().enumerate() {
                        let EdgeArg::Produced(result) = argument else {
                            continue;
                        };
                        let ty = body.values.get(target.params[index]).unwrap().ty;
                        let linear = match positive.types.get(ty).unwrap() {
                            Type::Service(service) => service.is_linear(),
                            Type::Builder(_) | Type::MapBuilder(..) | Type::Frame(_) => true,
                            _ => false,
                        };
                        if !linear || !exercised.insert((op.opcode(), is_unwind, result.slot)) {
                            continue;
                        }
                        let mut negative = positive.clone();
                        let body = negative.functions.get_mut(function).unwrap().body.as_mut().unwrap();
                        let TermKind::InvokeOwn { normal, unwind, .. } =
                            &mut body.blocks.get_mut(block_id).unwrap().term.kind else {
                                unreachable!();
                            };
                        let edge = if is_unwind { unwind } else { normal };
                        edge.args.remove(index);
                        let report = rejected(&roundtrip(&text::print(&negative).unwrap()), RuleId::B62);
                        assert!(!report.diagnostics.iter().any(|diagnostic| diagnostic.rule == RuleId::S3),
                            "{}: {report:?}", path.display());
                    }
                }
            }
        }
    }
    for expected in [
        ("own.split.checked", false, 0),
        ("own.split.checked", false, 1),
        ("builder.push", false, 0),
        ("builder.push", false, 1),
        ("builder.push", true, 0),
        ("builder.push", true, 1),
        ("await.take", true, 1),
        ("await.take", true, 2),
    ] {
        assert!(exercised.contains(&expected), "missing independent slot mutation: {expected:?}");
    }
}

#[test]
fn adding_an_alias_after_a_unique_proof_changes_the_verdict() {
    let source = include_str!("fixtures/verify/valid/B9-unique-proof-rechecked-after-coro.oir");
    verify_coro(&roundtrip(source)).unwrap();
    let changed = include_str!("fixtures/verify/invalid/B9-alias-inserted-after-proof.oir");
    let report = rejected(&roundtrip(changed), RuleId::B92);
    assert!(report.diagnostics.iter().any(|diagnostic| diagnostic.message.contains("published")));
}

#[test]
fn every_budget_failure_is_incomplete_and_never_certifies_the_module() {
    let source = include_str!("fixtures/verify/valid/B4-rebind-loop-and-staging.oir");
    let module = roundtrip(source);
    verify(&module).unwrap();
    for limits in [
        VerifyLimits { entities: 0, ..VerifyLimits::default() },
        VerifyLimits { blocks_per_function: 0, ..VerifyLimits::default() },
        VerifyLimits { instructions_per_function: 0, ..VerifyLimits::default() },
        VerifyLimits { values_per_function: 0, ..VerifyLimits::default() },
        VerifyLimits { expression_nodes_per_function: 0, ..VerifyLimits::default() },
        VerifyLimits { expression_depth: 0, ..VerifyLimits::default() },
        VerifyLimits { outcome_slots: 0, ..VerifyLimits::default() },
        VerifyLimits { memory_bytes: 0, ..VerifyLimits::default() },
        VerifyLimits { steps: 0, ..VerifyLimits::default() },
    ] {
        let options = VerifyOptions { limits, ..VerifyOptions::default() };
        let report = verify_with(&module, &options, VerifyContext::default()).unwrap_err();
        assert_eq!(report.failure, VerifyFailure::Incomplete, "{options:?}: {report:?}");
    }
    let source = include_str!("fixtures/verify/invalid/B2-consumed-token.oir");
    let module = roundtrip(source);
    let options = VerifyOptions {
        limits: VerifyLimits { diagnostics: 0, ..VerifyLimits::default() },
        ..VerifyOptions::default()
    };
    let report = verify_with(&module, &options, VerifyContext::default()).unwrap_err();
    assert_eq!(report.failure, VerifyFailure::Incomplete);
}
