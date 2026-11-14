use okkama_oir::{text, verify::*};
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
        EntityLocation::Edge { function, edge } => format!("edge:{}:{}", function.as_u32(), edge.0),
        other => format!("{other:?}"),
    }
}
#[test]
fn canonical_verifier_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/verify");
    let mut failures = Vec::new();
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
            let result = verify_with(&module, &options, VerifyContext::default());
            if class == "valid" {
                if let Err(error) = result { failures.push(format!("{}: {error:?}", path.display())); }
            } else {
                let expected = source.lines().find_map(|line| line.strip_prefix("// expect ")).unwrap();
                let (rule, primary) = expected.split_once(' ').unwrap();
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
                    VerifyContext::default()).unwrap_err();
                assert_eq!(first.diagnostics, report.diagnostics[..1], "{}", path.display());
            }
        }
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
