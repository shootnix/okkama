use okkama_oir::{text, verify::*};
use okkama_oir::visit::Rewrite;
use std::{fs, path::Path};

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
#[test]
fn canonical_verifier_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/verify");
    let mut failures = Vec::new();
    let mut covered = std::collections::BTreeSet::new();
    for class in ["valid", "invalid"] {
        let mut paths: Vec<_> = fs::read_dir(root.join(class)).unwrap().map(|e| e.unwrap().path()).collect();
        paths.sort();
        for path in paths {
            let source = fs::read_to_string(&path).unwrap();
            let module = match text::parse(&source) {
                Ok(module) => module,
                Err(error) => {
                    failures.push(format!("{}: {error}", path.display()));
                    continue;
                }
            };
            let options = VerifyOptions { execution: ExecutionMode::Tests, ..VerifyOptions::default() };
            let context = VerifyContext { external_contracts: &FixtureContracts(&module), ..VerifyContext::default() };
            let result = verify_with(&module, &options, context);
            if class == "valid" {
                if let Err(error) = result { failures.push(format!("{}: {error:?}", path.display())); }
            } else {
                let expected = source.lines().find_map(|line| line.strip_prefix("// expect ")).unwrap();
                let (rule, primary) = expected.split_once(' ').unwrap();
                covered.insert(rule.to_owned());
                let report = match result {
                    Err(report) => report,
                    Ok(_) => {
                        failures.push(format!("{}: accepted invalid fixture", path.display()));
                        continue;
                    }
                };
                assert_eq!(report.failure, VerifyFailure::Rejected, "{}: {report:?}", path.display());
                if !report.diagnostics.iter().any(|d| d.rule.as_str() == rule
                    && location(&d.primary.entity) == primary) {
                    failures.push(format!("{}: {expected}; {report:?}", path.display()));
                }
                let first = verify_with(&module, &VerifyOptions { errors: ErrorPolicy::First, ..options },
                    context).unwrap_err();
                assert_eq!(first.diagnostics, report.diagnostics[..1], "{}", path.display());
            }
        }
    }
    for rule in [
        "B1.1", "B1.2", "B2.1", "B2.2", "B2.3", "B3.1", "B3.2", "B4.1", "B4.2", "B4.3",
        "B5.1", "B5.2", "B5.3", "B5.4", "B6.3", "B8.1", "B8.2", "B10.1",
    ] {
        assert!(covered.contains(&format!("OIR2-{rule}")), "missing canonical negative for {rule}");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
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
fn specification_core_verifier_corpus() {
    use std::collections::BTreeSet;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let manifest = fs::read_to_string(root.join("spec/manifest.tsv")).unwrap();
    let core = include_str!("fixtures/core-corpus.txt");
    let mut seen = BTreeSet::new();
    let mut failures = Vec::new();
    for line in manifest.lines().skip(1) {
        let row: Vec<_> = line.split('\t').collect();
        if !core.lines().any(|path| path == row[3]) || !seen.insert(row[3]) { continue; }
        let path = root.join(row[3]);
        let source = fs::read_to_string(&path).unwrap();
        let module = text::parse(&source).unwrap();
        let options = VerifyOptions { execution: ExecutionMode::Tests, ..VerifyOptions::default() };
        let context = VerifyContext {
            external_contracts: &FixtureContracts(&module), ..VerifyContext::default()
        };
        let result = verify_with(&module, &options, context);
        match result {
            Ok(_) if row[2] == "semantic-reject" => failures.push(format!("{} ACCEPTED expected {}", row[3], row[6])),
            Err(report) if row[2] != "semantic-reject" => failures.push(format!("{}: {:?}", row[3],
                report.diagnostics.iter().map(|d| (d.rule.as_str(), location(&d.primary.entity), &d.message))
                    .collect::<Vec<_>>())),
            Err(report) if !report.diagnostics.iter().any(|d| d.rule.as_str() == row[6]) => {
                failures.push(format!("{} EXPECTED {}: {:?}", row[3], row[6], report.diagnostics));
            }
            _ => {}
        }
    }
    assert_eq!(seen.len(), core.lines().count(), "core inventory contains a path absent from the manifest");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// The fixture files are the frozen native test ABI catalogue. Production
// verification never registers external contracts from input declarations.
struct FixtureContracts<'a>(&'a okkama_oir::Module);
impl ExternalContracts for FixtureContracts<'_> {
    fn native(&self, name: &str) -> Option<&okkama_oir::Signature> {
        self.0.functions.iter().find_map(|(_, f)| f.attrs.iter().any(|a| {
            matches!(a, okkama_oir::FunctionAttr::Extern(n) if n == name && n.starts_with("fixture."))
        }).then_some(&f.signature))
    }
    fn intrinsic(&self, _: &str) -> Option<&okkama_oir::Signature> { None }
    fn native_summary(&self, name: &str) -> Option<&okkama_oir::Summary> {
        self.0.functions.iter().find_map(|(_, f)| f.attrs.iter().any(|a| {
            matches!(a, okkama_oir::FunctionAttr::Extern(n) if n == name && n.starts_with("fixture."))
        }).then_some(f.summary).flatten()).and_then(|id| self.0.summaries.get(id).ok())
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
            let original = text::parse(&fs::read_to_string(&path).unwrap()).unwrap();
            let expected = verdict(&original);
            let printed = text::print(&original).unwrap();
            let renamed = text::parse(&printed.replace('%', "%renamed_")).unwrap();
            assert_eq!(verdict(&renamed), expected, "SSA names: {}", path.display());
            let roundtrip = text::parse(&printed).unwrap();
            assert_eq!(verdict(&roundtrip), expected, "codec: {}", path.display());
            let mut permuted = original.clone();
            for (_, function) in permuted.functions.iter_mut() {
                let Some(body) = &mut function.body else { continue; };
                let mut remap = IdRemap::default();
                let mut values = Arena::new();
                for (old, value) in body.values.iter().collect::<Vec<_>>().into_iter().rev() {
                    remap.values.insert(old, values.alloc(value.clone()).unwrap());
                }
                let mut instructions = Arena::new();
                for (old, inst) in body.insts.iter().collect::<Vec<_>>().into_iter().rev() {
                    remap.instructions.insert(old, instructions.alloc(inst.clone()).unwrap());
                }
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
