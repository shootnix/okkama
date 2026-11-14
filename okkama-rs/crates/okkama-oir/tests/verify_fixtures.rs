use okkama_oir::{text, verify::*};
use okkama_oir::visit::Rewrite;
use std::{fs, path::Path};
mod support;
use support::FixtureContracts;

fn location(entity: &EntityLocation) -> String {
    match entity {
        EntityLocation::Scope(id) => format!("scope:{}", id.as_u32()),
        EntityLocation::Function(id) => format!("function:{}", id.as_u32()),
        EntityLocation::Instruction { function, instruction } => {
            format!("instruction:{}:{}", function.as_u32(), instruction.as_u32())
        }
        EntityLocation::Terminator { function, block } => {
            format!("terminator:{}:{}", function.as_u32(), block.as_u32())
        }
        EntityLocation::Block { function, block } => format!("block:{}:{}", function.as_u32(), block.as_u32()),
        EntityLocation::Value { function, value } => format!("value:{}:{}", function.as_u32(), value.as_u32()),
        EntityLocation::Operand { site: OperationSite::Instruction { function, instruction }, path } => {
            format!("operand:{}:{}:{:?}", function.as_u32(), instruction.as_u32(), path.0)
        }
        EntityLocation::Operand { site: OperationSite::Terminator { function, block }, path } => {
            format!("operand-term:{}:{}:{:?}", function.as_u32(), block.as_u32(), path.0)
        }
        EntityLocation::Edge { function, edge } => format!("edge:{}:{}", function.as_u32(), edge.0),
        other => format!("{other:?}"),
    }
}
fn fixture_result(module: &okkama_oir::Module, errors: ErrorPolicy) -> Result<(), VerifyReport> {
    let options = VerifyOptions {
        execution: ExecutionMode::Tests,
        errors,
        ..VerifyOptions::default()
    };
    let context = VerifyContext {
        external_contracts: &FixtureContracts(module),
        ..VerifyContext::default()
    };
    verify_with(module, &options, context).map(|_| ())
}

fn fixture_module(root: &Path, path: &str) -> okkama_oir::Module {
    let source = fs::read_to_string(root.join(path)).unwrap();
    text::parse(&source).unwrap_or_else(|error| panic!("{path}: {error}"))
}

#[test]
fn canonical_verifier_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/verify");
    let mut failures = Vec::new();
    for class in ["valid", "invalid"] {
        let mut paths: Vec<_> = fs::read_dir(root.join(class)).unwrap()
            .map(|entry| entry.unwrap().path()).collect();
        paths.sort();
        for path in paths {
            let source = fs::read_to_string(&path).unwrap();
            let module = text::parse(&source).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            let result = fixture_result(&module, ErrorPolicy::All);
            assert_eq!(fixture_result(&module, ErrorPolicy::All), result, "{}", path.display());
            let canonical = text::print(&module).unwrap();
            let roundtrip = text::parse(&canonical).unwrap();
            let repeated = fixture_result(&roundtrip, ErrorPolicy::All);
            if class == "valid" {
                if let Err(error) = result {
                    failures.push(format!("{}: {error:?}", path.display()));
                }
                assert!(repeated.is_ok(), "{}: {repeated:?}", path.display());
                continue;
            }
            let expected = source.lines().find_map(|line| line.strip_prefix("// expect ")).unwrap();
            let (rule, primary) = expected.split_once(' ').unwrap();
            let Err(report) = result else {
                failures.push(format!("{}: accepted invalid fixture", path.display()));
                continue;
            };
            assert_eq!(report.failure, VerifyFailure::Rejected, "{}: {report:?}", path.display());
            if !report.diagnostics.iter().any(|diagnostic| diagnostic.rule.as_str() == rule
                && location(&diagnostic.primary.entity) == primary) {
                failures.push(format!("{}: {expected}; {report:?}", path.display()));
            }
            let repeated = repeated.unwrap_err();
            assert_eq!(repeated.failure, VerifyFailure::Rejected);
            assert!(repeated.diagnostics.iter().any(|diagnostic| diagnostic.rule.as_str() == rule),
                "canonical roundtrip lost {rule}: {}", path.display());
            let first = fixture_result(&module, ErrorPolicy::First).unwrap_err();
            assert_eq!(first.diagnostics, report.diagnostics[..1], "{}", path.display());
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_rule_has_a_verified_canonical_positive_and_negative() {
    use std::collections::BTreeSet;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/verify");
    let mut covered = BTreeSet::new();
    for line in include_str!("fixtures/verify/matrix.tsv").lines().skip(1) {
        let row: Vec<_> = line.split('\t').collect();
        assert_eq!(row.len(), 3, "{line}");
        assert!(covered.insert(row[0]), "duplicate RuleId: {line}");
        let positive = fixture_module(&root, row[1]);
        fixture_result(&positive, ErrorPolicy::All).unwrap_or_else(|report| panic!("{line}: {report:?}"));
        let negative = fixture_module(&root, row[2]);
        let report = fixture_result(&negative, ErrorPolicy::All).unwrap_err();
        assert_eq!(report.failure, VerifyFailure::Rejected, "{line}");
        assert!(report.diagnostics.iter().any(|diagnostic| diagnostic.rule.as_str() == row[0]),
            "{line}: {report:?}");
    }
    let required: BTreeSet<_> = RuleId::ALL.iter().map(|rule| rule.as_str()).collect();
    assert_eq!(covered, required, "every RuleId needs both canonical witnesses");
}

#[test]
fn nontextual_outcome_and_slot_corruption() {
    use okkama_oir::*;
    let source = include_str!("fixtures/verify/valid/checked-split-normal-and-unwind.oir");
    for corruption in 0..3 {
        let mut module = text::parse(source).unwrap();
        let function = FuncId::from_u32(0);
        let bool_ty = module.types.intern(Type::Bool).unwrap();
        let body = module.functions.get_mut(function).unwrap().body.as_mut().unwrap();
        if corruption == 2 {
            let param = body.blocks.get(BlockId::from_u32(1)).unwrap().params[0];
            body.values.get_mut(param).unwrap().ty = bool_ty;
        } else {
            let term = &mut body.blocks.get_mut(body.entry).unwrap().term.kind;
            let TermKind::InvokeOwn { normal, .. } = term else { panic!("expected invoke"); };
            let EdgeArg::Produced(result) = &mut normal.args[0] else { panic!("expected produced slot"); };
            if corruption == 0 { result.outcome = Outcome::Unwind; }
            else { result.slot = u16::MAX; }
        }
        let report = verify(&module).unwrap_err();
        assert!(report.diagnostics.iter().any(|d| d.rule == RuleId::B62 && d.primary.entity == EntityLocation::Edge {
            function, edge: VerifyEdgeId(0),
        }), "{report:?}");
    }
}

#[test]
fn verify_function_limits_analysis_to_the_selected_body() {
    use okkama_oir::*;
    let source = include_str!("fixtures/verify/valid/H3-dominating-defer-prefix.oir");
    let mut module = text::parse(source).unwrap();
    let broken = FuncId::from_u32(1);
    let body = module.functions.get_mut(broken).unwrap().body.as_mut().unwrap();
    let inst = body.blocks.get(body.entry).unwrap().insts[0];
    body.insts.get_mut(inst).unwrap().op = Operation::Ownership(OwnershipOp::Empty {});
    assert!(verify(&module).is_err());
    let report = verify_function(&module, FuncId::from_u32(0), VerifyContext::default(), &VerifyOptions::default());
    assert_eq!(report.failure, None, "{report:?}");
}

#[test]
fn graph_and_high_analysis_budgets_never_produce_a_certificate() {
    for source in [
        include_str!("fixtures/verify/valid/SSA2-irreducible-CFG.oir"),
        include_str!("fixtures/verify/valid/H3-dominating-defer-prefix.oir"),
        include_str!("fixtures/verify/valid/A2-completion-after-merge.oir"),
    ] {
        let module = text::parse(source).unwrap();
        let stats = verify(&module).unwrap().stats().clone();
        for limits in [
            VerifyLimits { steps: stats.steps - 1, ..VerifyLimits::default() },
            VerifyLimits { memory_bytes: stats.memory_bytes - 1, ..VerifyLimits::default() },
        ] {
            let options = VerifyOptions { limits, ..VerifyOptions::default() };
            let report = verify_with(&module, &options, VerifyContext::default()).unwrap_err();
            assert_eq!(report.failure, VerifyFailure::Incomplete, "{report:?}");
        }
    }
}

#[test]
fn production_execution_rejects_test_authority() {
    use okkama_oir::*;
    let module = text::parse(include_str!("fixtures/verify/valid/A3-test-authority.oir")).unwrap();
    let report = verify(&module).unwrap_err();
    assert!(report.diagnostics.iter().any(|d| d.rule == RuleId::A3 && d.primary.entity == EntityLocation::Instruction {
        function: FuncId::from_u32(0), instruction: InstId::from_u32(0),
    }), "{report:?}");
}

#[test]
fn high_intents_and_shared_await_do_not_create_ownership_results() {
    use okkama_oir::*;
    for (source, expected_high, expected_own) in [
        (include_str!("fixtures/verify/valid/H4-prepared-return.oir"), 0, 1),
        (include_str!("fixtures/verify/valid/A3-async-test.oir"), 1, 2),
    ] {
        let mut module = text::parse(source).unwrap();
        let op = module.functions.get(FuncId::from_u32(0)).unwrap().body.as_ref().unwrap()
            .insts.get(InstId::from_u32(0)).unwrap().op.clone();
        let high = ContractRegistry::new(&module).lookup(&op).unwrap();
        assert_eq!(high.normal.results.len(), expected_high);
        if let Some(unwind) = high.unwind { assert_eq!(unwind.results.len(), 1); }
        module.profile = Profile::Own;
        let own = ContractRegistry::new(&module).lookup(&op).unwrap();
        assert_eq!(own.normal.results.len(), expected_own);
    }
}

#[test]
fn specification_manifest_verifier_corpus() {
    use std::collections::BTreeSet;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let manifest = fs::read_to_string(root.join("spec/manifest.tsv")).unwrap();
    let mut positives = BTreeSet::new();
    let mut negatives = BTreeSet::new();
    let mut reviews = BTreeSet::new();
    let mut examples = BTreeSet::new();
    let mut pairs = BTreeSet::new();
    for line in manifest.lines().skip(1) {
        let row: Vec<_> = line.split('\t').collect();
        assert_eq!(row.len(), 12, "{line}");
        if row[0].starts_with("example-") || row[0].starts_with("review-A.1-") {
            examples.insert(row[0]);
        }
        if matches!(row[8], "historical" | "parse-reject") {
            assert!(row[9..].iter().all(|field| *field == "-"), "{line}");
            continue;
        }
        assert!(matches!(row[8], "static-ir" | "protocol-contract" | "runtime-behavior" | "frontend-reject"),
            "missing verification level: {line}");
        assert_ne!(row[11], "-", "missing verifier RuleIds: {line}");
        // A codec illustration and its behavior sidecar are not verifier certificates.
        // The explicit columns identify complete canonical translations for each example.
        if positives.insert(row[9]) {
            let module = fixture_module(&root, row[9]);
            fixture_result(&module, ErrorPolicy::All).unwrap_or_else(|report| panic!("{line}: {report:?}"));
        }
        if negatives.insert((row[10], row[11])) {
            let module = fixture_module(&root, row[10]);
            let report = fixture_result(&module, ErrorPolicy::All).unwrap_err();
            assert_eq!(report.failure, VerifyFailure::Rejected, "{line}");
            for rule in row[11].split(',') {
                assert!(RuleId::ALL.iter().any(|known| known.as_str() == rule), "{line}");
                assert!(report.diagnostics.iter().any(|diagnostic| diagnostic.rule.as_str() == rule),
                    "{line}: {report:?}");
            }
        }
        if let Some(number) = row[0].strip_prefix("review-A.1-") {
            reviews.insert(number.parse::<u8>().unwrap());
        }
        if row[0].starts_with('M') {
            pairs.insert((row[0].split('-').next().unwrap(), row[2]));
        }
    }
    let required = include_str!("fixtures/spec/examples.txt").lines().collect();
    assert_eq!(examples, required, "every spec example must remain wired to canonical witnesses");
    assert_eq!(reviews, (1..=24).collect(), "preserve every numbered A.1 example");
    for number in 1..=9 {
        for class in ["accept", "semantic-reject"] {
            assert!(pairs.contains(&(format!("M{number}").as_str(), class)), "M{number} lacks {class}");
        }
    }
}


#[test]
fn ownership_verdicts_survive_codec_and_arena_permutations() {
    use okkama_oir::*;
    use std::collections::BTreeSet;
    fn verdict(module: &Module) -> (Option<VerifyFailure>, BTreeSet<RuleId>) {
        let options = VerifyOptions { execution: ExecutionMode::Tests, ..VerifyOptions::default() };
        let context = VerifyContext { external_contracts: &FixtureContracts(module), ..VerifyContext::default() };
        match verify_with(module, &options, context) {
            Ok(_) => (None, BTreeSet::new()),
            Err(report) => (Some(report.failure), report.diagnostics.into_iter().map(|d| d.rule).collect()),
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/verify");
    for class in ["valid", "invalid"] {
        let mut paths: Vec<_> = fs::read_dir(root.join(class)).unwrap().map(|e| e.unwrap().path()).collect();
        paths.sort();
        for path in paths {
            let original = text::parse(&fs::read_to_string(&path).unwrap())
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            let expected = verdict(&original);
            let printed = text::print(&original).unwrap();
            let mut renamed = original.clone();
            for (_, function) in renamed.functions.iter_mut() {
                function.symbol = format!("renamed_{}", function.symbol);
            }
            let renamed = text::print(&renamed).unwrap().replace('%', "%renamed_");
            let renamed = text::parse(&renamed).unwrap();
            assert_eq!(verdict(&renamed), expected, "SSA names: {}", path.display());
            let roundtrip = text::parse(&printed).unwrap();
            assert_eq!(verdict(&roundtrip), expected, "codec: {}", path.display());
            let mut permuted = original.clone();
            for (_, function) in permuted.functions.iter_mut() {
                let Some(body) = &mut function.body else { continue; };
                let mut remap = IdRemap::default();
                let mut blocks = Arena::new();
                for (old, block) in body.blocks.iter().collect::<Vec<_>>().into_iter().rev() {
                    remap.blocks.insert(old, blocks.alloc(block.clone()).unwrap());
                }
                body.blocks = blocks;
                let mut values = Arena::new();
                for (old, value) in body.values.iter().collect::<Vec<_>>().into_iter().rev() {
                    remap.values.insert(old, values.alloc(value.clone()).unwrap());
                }
                let mut instructions = Arena::new();
                for (old, inst) in body.insts.iter().collect::<Vec<_>>().into_iter().rev() {
                    remap.instructions.insert(old, instructions.alloc(inst.clone()).unwrap());
                }
                let mut closes = Arena::new();
                for (old, item) in body.cleanup.closes.iter().collect::<Vec<_>>().into_iter().rev() {
                    remap.cleanup_closes.insert(old, closes.alloc(item.clone()).unwrap());
                }
                body.cleanup.closes = closes;
                let mut schedules = Arena::new();
                for (old, item) in body.cleanup.schedules.iter().collect::<Vec<_>>().into_iter().rev() {
                    remap.cleanup_schedules.insert(old, schedules.alloc(item.clone()).unwrap());
                }
                body.cleanup.schedules = schedules;
                let mut plans = Arena::new();
                for (old, item) in body.cleanup.plans.iter().collect::<Vec<_>>().into_iter().rev() {
                    remap.cleanup_plans.insert(old, plans.alloc(item.clone()).unwrap());
                }
                body.cleanup.plans = plans;
                let mut proofs = Arena::new();
                for (old, item) in body.cleanup.proofs.iter().collect::<Vec<_>>().into_iter().rev() {
                    remap.cleanup_proofs.insert(old, proofs.alloc(item.clone()).unwrap());
                }
                body.cleanup.proofs = proofs;
                let mut roots = Arena::new();
                for (old, item) in body.cleanup.roots.iter().collect::<Vec<_>>().into_iter().rev() {
                    remap.root_maps.insert(old, roots.alloc(item.clone()).unwrap());
                }
                body.cleanup.roots = roots;
                body.values = values;
                body.insts = instructions;
                body.rewrite(&mut remap);
            }
            assert_eq!(verdict(&permuted), expected, "arena order: {}", path.display());
        }
    }
}

#[test]
fn ownership_fixpoint_limits_are_incomplete() {
    let source = include_str!("fixtures/verify/valid/B4-rebind-loop-and-staging.oir");
    let module = text::parse(source).unwrap();
    let stats = verify(&module).unwrap().stats().clone();
    for limits in [
        VerifyLimits { steps: stats.steps - 1, ..VerifyLimits::default() },
        VerifyLimits { memory_bytes: stats.memory_bytes - 1, ..VerifyLimits::default() },
    ] {
        let options = VerifyOptions { limits, ..VerifyOptions::default() };
        let report = verify_with(&module, &options, VerifyContext::default()).unwrap_err();
        assert_eq!(report.failure, VerifyFailure::Incomplete, "{report:?}");
    }
}

#[test]
fn absent_call_summaries_are_specific_to_high() {
    use okkama_oir::Profile;
    let source = include_str!("fixtures/verify/valid/B5-high-optional-summary.oir");
    let mut module = text::parse(source).unwrap();
    verify(&module).unwrap();
    for profile in [Profile::Own, Profile::Coro, Profile::Low] {
        module.profile = profile;
        let report = verify(&module).unwrap_err();
        assert!(report.diagnostics.iter().any(|diagnostic| diagnostic.rule == RuleId::B51), "{report:?}");
    }
}

#[test]
fn protocol_census_and_structural_gaps_are_explicit() {
    use okkama_oir::*;
    let module = text::parse(include_str!("fixtures/verify/valid/B7-any-family.oir")).unwrap();
    let verified = verify(&module).unwrap();
    assert_eq!(verified.stats().dynamic.sites.len(), 1);
    assert_eq!(verified.stats().dynamic.census.module_count(DynamicReason::Any), 1);
    assert_eq!(verified.stats().dynamic.census.module_count(DynamicReason::Merge), 0);
    assert!(matches!(verified.stats().dynamic.sites[0].site, OperationSite::Terminator { .. }));
    let module = text::parse(include_str!("fixtures/verify/valid/B9-frame-save-resume-cancel.oir")).unwrap();
    assert!(verify_coro(&module).unwrap().stats().structural_checks
        .contains(&StructuralCheck::B95FrameLayoutAndContinuations));
    let module = text::parse(include_str!("fixtures/verify/valid/B9-duplicate-positions-shared-outcome.oir")).unwrap();
    assert!(verify_own(&module).unwrap().stats().structural_checks
        .contains(&StructuralCheck::B96CollectorPositions));
}

#[test]
fn malformed_cleanup_proof_arenas_are_rejected_without_panics() {
    use okkama_oir::*;
    for corruption in 0..3 {
        let mut module = text::parse(include_str!("fixtures/verify/valid/B10-common-cleanup-plan.oir")).unwrap();
        let body = module.functions.get_mut(FuncId::from_u32(0)).unwrap().body.as_mut().unwrap();
        match corruption {
            0 => body.cleanup.plans.get_mut(CleanupPlanId::from_u32(0)).unwrap().close =
                CleanupCloseId::from_u32(u32::MAX),
            1 => body.cleanup.roots.get_mut(RootMapId::from_u32(0)).unwrap().bindings =
                vec![RootBinding { value: ValueId::from_u32(u32::MAX), trace: RootTrace::CleanupActivation }],
            _ => body.cleanup.roots.get_mut(RootMapId::from_u32(0)).unwrap().boundary =
                RootBoundary::BeforeInstruction(InstId::from_u32(u32::MAX)),
        }
        let report = verify_low(&module).unwrap_err();
        assert_eq!(report.failure, VerifyFailure::Rejected);
        assert!(report.diagnostics.iter().any(|diagnostic| diagnostic.rule == RuleId::S3));
    }
}
