use okkama_oir::{text, *};
use std::{fs, path::{Path, PathBuf}};

fn paths(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() { files.extend(paths(&path)); }
        else if path.extension().is_some_and(|e| e == "oir") { files.push(path); }
    }
    files.sort(); files
}

#[test]
fn full_fixture_modules_roundtrip_without_semantic_validation() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut errors = Vec::new();
    for path in paths(&root) {
        let source = fs::read_to_string(&path).unwrap();
        let result = (|| {
            let module = text::parse(&source).map_err(|e| e.to_string())?;
            let canonical = text::print(&module).map_err(|e| e.to_string())?;
            let parsed = text::parse(&canonical).map_err(|e| e.to_string())?;
            if !text::normalized_eq(&module, &parsed).map_err(|e| e.to_string())? { return Err("normalized round-trip differs".into()); }
            if text::print(&parsed).map_err(|e| e.to_string())? != canonical { return Err("second print differs".into()); }
            let crlf = text::parse(&source.replace('\n', "\r\n")).map_err(|e| e.to_string())?;
            if !text::normalized_eq(&module, &crlf).map_err(|e| e.to_string())? { return Err("CRLF changes semantics".into()); }
            Ok::<_, String>(())
        })();
        if let Err(error) = result { errors.push(format!("{}: {error}", path.display())); }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn manifest_lists_every_fixture_and_keeps_historical_and_frontend_cases_separate() {
    use std::collections::BTreeSet;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let manifest = fs::read_to_string(root.join("spec/manifest.tsv")).unwrap();
    let mut ids = BTreeSet::new();
    let mut files = BTreeSet::new();
    let mut sections = BTreeSet::new();
    let mut pairs = BTreeSet::new();
    let mut rules = BTreeSet::new();
    for row in manifest.lines().skip(1) {
        let fields: Vec<_> = row.split('\t').collect();
        assert_eq!(fields.len(), 12, "{row}");
        assert!(ids.insert(fields[0]), "duplicate manifest id {}", fields[0]);
        sections.extend(fields[1].split(';'));
        let path = root.join(fields[3]);
        assert!(path.is_file(), "{row}");
        files.insert(path.clone());
        let source = fs::read_to_string(&path).unwrap();
        match fields[2] {
            "v2" | "accept" | "semantic-reject" => {
                assert!(source.starts_with(&format!("oir 2 {}\n", fields[4])), "{row}");
                assert!(!source.contains('…') && !source.contains("..."), "unexpanded fragment {row}");
                if fields[2] == "semantic-reject" {
                    assert!(source.lines().nth(1).unwrap().contains(fields[6]), "missing expected rule: {row}");
                    rules.insert(fields[6].split('.').next().unwrap());
                }
                if fields[0].starts_with('M') {
                    pairs.insert((fields[0].split('-').next().unwrap(), fields[2]));
                }
            }
            "historical" => {
                assert_eq!(path.extension().unwrap(), "oir1");
                assert!(source.contains("historical"));
            }
            "frontend-reject" => {
                assert_eq!(path.extension().unwrap(), "ok");
                assert!(source.contains("frontend-reject"));
            }
            "parse-reject" => {
                assert_eq!(format!("{:?}", text::parse(&source).unwrap_err().kind), fields[6]);
            }
            other => panic!("unknown manifest classification {other}"),
        }
        if fields[7] != "-" {
            assert!(root.join(fields[7]).is_file(), "missing expectations: {row}");
        }
        if fields[5] != "-" {
            let model = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/ownership/model");
            assert!(model.join(fields[5]).is_file(), "missing model fixture: {row}");
        }
    }
    for dir in ["spec", "invalid"] {
        for path in paths(&root.join(dir)) {
            assert!(files.contains(&path), "unlisted fixture {}", path.display());
        }
    }
    for section in [
        "3.3", "4.2", "5.1", "5.2", "6.1", "6.2", "6.3", "6.4", "6.5", "7.1", "7.2", "7.3", "8",
        "9.1", "9.2", "9.3", "9.4", "9.5", "10.1", "10.2", "10.3", "10.4", "10.5", "10.6", "11", "12",
        "15.1", "15.2", "16.1", "16.2", "16.3", "16.4", "16.5", "16.6",
    ] {
        assert!(sections.contains(section), "uncovered normative section {section}");
    }
    for n in 1..=9 {
        for class in ["accept", "semantic-reject"] {
            assert!(pairs.contains(&(format!("M{n}").as_str(), class)), "M{n} missing {class}");
        }
    }
    for n in 1..=11 {
        assert!(rules.contains(format!("OIR2-B{n}").as_str()), "B{n} missing negative");
    }
}

fn roundtrip_file(name: &str) -> Module {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/invalid").join(format!("{name}.oir"));
    let module = text::parse(&fs::read_to_string(path).unwrap()).unwrap();
    text::parse(&text::print(&module).unwrap()).unwrap()
}
fn main_body(module: &Module) -> &Body { module.functions.iter().find(|(_, f)| f.symbol == "main").unwrap().1.body.as_ref().unwrap() }
fn operations(body: &Body) -> Vec<&Operation> {
    body.insts.iter().map(|(_, i)| &i.op).chain(body.blocks.iter().filter_map(|(_, b)| if let TermKind::InvokeOwn { op, .. } = &b.term.kind { Some(op.as_ref()) } else { None })).collect()
}

#[test]
fn negative_roundtrips_preserve_the_actual_error_witnesses() {
    let m = roundtrip_file("B1-token-as-reference"); let b = main_body(&m);
    let Operation::Core(CoreOp::IntAdd { left, .. }) = operations(b)[0] else { panic!() };
    assert_eq!(m.types.get(b.values.get(*left).unwrap().ty).unwrap(), &Type::Service(ServiceType::Own));
    let m = roundtrip_file("B2-consumed-token"); let ops = operations(main_body(&m));
    let packets: Vec<_> = ops.iter().filter_map(|op| if let Operation::Ownership(OwnershipOp::Move { packet }) = op { Some(*packet) } else { None }).collect();
    assert_eq!(packets.len(), 2); assert_eq!(packets[0], packets[1]);
    let m = roundtrip_file("B2-duplicate-input"); assert!(operations(main_body(&m)).iter().any(|op| matches!(op, Operation::Ownership(OwnershipOp::Join { packets }) if packets.len() == 2 && packets[0] == packets[1])));
    let m = roundtrip_file("B4-double-close"); let states: Vec<_> = operations(main_body(&m)).iter().filter_map(|op| if let Operation::Ownership(OwnershipOp::ScopeEnd { state, .. }) = op { Some(*state) } else { None }).collect();
    assert_eq!(states.len(), 2); assert_eq!(states[0], states[1]);
    let m = roundtrip_file("B5-missing-summary"); assert!(operations(main_body(&m)).iter().any(|op| matches!(op, Operation::Call(CallOp::Direct { call: CallSite { summary: None, .. }, .. }))));
    let m = roundtrip_file("B6-block-local-unwind"); let b = main_body(&m); let entry = b.blocks.get(b.entry).unwrap();
    let value = entry.unwind.as_ref().unwrap().target.args[0];
    assert!(matches!(b.values.get(value).unwrap().def, Definition::InstResult { inst, .. } if entry.insts.contains(&inst)));
    let m = roundtrip_file("B8-reused-builder"); let pairs: Vec<_> = operations(main_body(&m)).iter().filter_map(|op| if let Operation::Collection(CollectionOp::BuilderFinish { buffer, packet }) = op { Some((*buffer, *packet)) } else { None }).collect();
    assert_eq!(pairs.len(), 2); assert_eq!(pairs[0], pairs[1]);
    let m = roundtrip_file("B9-no-capability"); let ops = operations(main_body(&m));
    assert!(!ops.iter().any(|op| matches!(op, Operation::Async(AsyncOp::UniqueCacheProof { .. }))));
    assert!(ops.iter().any(|op| matches!(op, Operation::Core(CoreOp::Undef { ty }) if m.types.get(*ty).unwrap() == &Type::Service(ServiceType::UniqueCache))));
    let m = roundtrip_file("B10-early-commit"); let ops = operations(main_body(&m));
    assert!(ops.iter().any(|op| matches!(op, Operation::Exit(ExitOp::Commit { .. }))));
    assert!(!ops.iter().any(|op| matches!(op, Operation::Ownership(OwnershipOp::ScopeEnd { .. }) | Operation::Cleanup(_))));
    let m = roundtrip_file("B11-residual-plain"); assert!(main_body(&m).values.iter().any(|(_, v)| m.types.get(v.ty).unwrap() == &Type::Any && v.facts == ValueFacts::Plain { certificate: PlainCertificate::Scalar }));
    let m = roundtrip_file("M1-object-method-is-not-observation");
    for op in operations(main_body(&m)) { if let Operation::Call(CallOp::Direct { function, call }) = op { assert_eq!(m.summaries.get(call.summary.unwrap()).unwrap().entry_kind, EntryKind::ObservationAdmissionStaysCaller); assert_ne!(m.functions.get(*function).unwrap().summary, call.summary); } }
    let m = roundtrip_file("M2-bound-take-not-fresh"); let b = main_body(&m);
    assert!(operations(b).iter().any(|op| matches!(op, Operation::Ownership(OwnershipOp::Accept { .. }))));
    assert!(operations(b).iter().any(|op| matches!(op, Operation::Ownership(OwnershipOp::ScopeSplit { selection, .. }) if matches!(b.selections.get(*selection).unwrap(), SelectionExpr::Value(_)))));
    let m = roundtrip_file("M7-double-release"); let graphs: Vec<_> = operations(main_body(&m)).iter().filter_map(|op| if let Operation::Carrier(CarrierOp::Release { state, .. }) = op { Some(*state) } else { None }).collect();
    assert_eq!(graphs.len(), 2); assert_eq!(graphs[0], graphs[1]);
    for name in ["M8-cancelled-return-cannot-commit", "M9-incomplete-service-schedule"] {
        let m = roundtrip_file(name); let ops = operations(main_body(&m));
        if name.starts_with("M8") { assert!(ops.iter().any(|op| matches!(op, Operation::Cleanup(CleanupOp::DeliverCancel { .. })))); assert!(!ops.iter().any(|op| matches!(op, Operation::Exit(ExitOp::Abort { .. })))); }
        else { assert_eq!(ops.iter().filter(|op| matches!(op, Operation::Cleanup(CleanupOp::Settle { .. }))).count(), 1); assert!(ops.iter().any(|op| matches!(op, Operation::Cleanup(CleanupOp::Plan { .. })))); }
    }
}

#[test]
fn remaining_negative_witnesses_keep_lifetimes_provenance_and_cleanup_order() {
    let m = roundtrip_file("B3-dead-borrow"); let b = main_body(&m);
    assert!(b.values.iter().any(|(_, v)| matches!(v.facts, ValueFacts::Managed { .. })));
    assert!(operations(b).iter().any(|o| matches!(o, Operation::Ownership(OwnershipOp::ScopeEnd { .. }))));
    assert!(operations(b).iter().any(|o| matches!(o, Operation::Call(_))));
    for name in ["B7-stale-epoch", "M5-rehome-stales-snapshot"] {
        let m = roundtrip_file(name); let b = main_body(&m);
        assert!(operations(b).iter().any(|o| matches!(o, Operation::Ownership(OwnershipOp::DynamicCheck { .. }))));
        assert!(!operations(b).iter().any(|o| matches!(o, Operation::Ownership(OwnershipOp::DynamicRefresh { .. }))));
    }
    let m = roundtrip_file("B2-lost-empty-rest"); let b = main_body(&m);
    let rest = b.insts.iter().find(|(_, i)| matches!(i.op, Operation::Ownership(OwnershipOp::Split { .. }))).unwrap().1.results[0];
    assert!(b.blocks.iter().any(|(_, b)| matches!(&b.term.kind, TermKind::Ret { values } if !values.contains(&rest))));
    for holder in ["channel", "carrier", "pending", "inputs"] {
        let m = roundtrip_file(&format!("M3-{holder}-split")); let b = main_body(&m);
        assert!(b.values.iter().any(|(_, v)| matches!(v.facts, ValueFacts::Managed { .. })));
        assert!(operations(b).iter().any(|op| matches!(op, Operation::Ownership(OwnershipOp::ScopeSplit { .. }))));
        assert!(!b.keys.is_empty());
    }
    let m = roundtrip_file("M4-empty-selection-is-managed"); let b = main_body(&m);
    assert!(b.values.iter().any(|(_, v)| matches!(v.facts, ValueFacts::Plain { certificate: PlainCertificate::Scalar }) && matches!(m.types.get(v.ty).unwrap(), Type::Object { .. })));
    let m = roundtrip_file("M6-panic-is-not-typed-cleanup"); let b = main_body(&m);
    let active = b.blocks.get(b.entry).unwrap().params[0];
    assert_eq!(m.types.get(b.values.get(active).unwrap().ty).unwrap(), &Type::Service(ServiceType::Failure));
    assert!(operations(b).iter().any(|op| matches!(op, Operation::Const(Literal::Enum { .. }))));
    assert!(b.blocks.iter().any(|(_, b)| matches!(b.term.kind, TermKind::Throw(_))));
    assert!(!b.blocks.iter().any(|(_, b)| matches!(b.term.kind, TermKind::Raise(value) if value == active)));
}
