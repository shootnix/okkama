use super::{LowerError, lower_to_oir2};
use crate::analysis::{self, thir};
use crate::oir2 as ir;
use crate::syntax::{self, ParseOptions};
use crate::SourceFile;

fn with_analysis<T>(text: &str, test: impl FnOnce(&mut analysis::Analysis<'_>) -> T) -> T {
    let source = SourceFile::new("oir2.ok", text).expect("small source");
    let program = syntax::parse(&source, &ParseOptions::default()).expect("valid source syntax");
    let mut analysis = analysis::analyze(vec![analysis::SourceModule {
        path: "app".into(), program: &program, imports: vec![],
    }]).unwrap_or_else(|errors| panic!("analysis: {errors:?}"));
    test(&mut analysis)
}

/// Developer test helper: frontend -> independent high producer -> canonical text.
fn print_oir2(source: &str) -> Result<String, LowerError> {
    with_analysis(source, |analysis| {
        let module = lower_to_oir2(analysis)?;
        verify_high(&module, source);
        let text = ir::text::print(&module).expect("printable high");
        let parsed = ir::text::parse(&text).expect("high round trip parses");
        verify_high(&parsed, "printed lowering");
        assert!(ir::text::normalized_eq(&module, &parsed).expect("normalized comparison"));
        assert_eq!(ir::text::print(&parsed).unwrap(), text);
        Ok(text)
    })
}

fn golden(source: &str, expected: &str) {
    let actual = print_oir2(source).unwrap_or_else(|error| panic!("{error}"));
    if std::env::var_os("OKKAMA_OIR2_UPDATE_GOLDENS").is_some() {
        let directory = format!("{}/src/lowering/oir2/goldens", env!("CARGO_MANIFEST_DIR"));
        for entry in std::fs::read_dir(directory).expect("golden directory") {
            let path = entry.expect("golden entry").path();
            if expected == std::fs::read_to_string(&path).expect("existing golden") {
                std::fs::write(path, &actual).expect("regenerated golden");
                return;
            }
        }
    }
    let expected = ir::text::parse(expected).expect("generated golden parses");
    verify_high(&expected, "stored golden");
    let expected = ir::text::print(&expected).expect("generated golden prints");
    assert!(actual == expected, "expected:\n{expected}\nactual:\n{actual}");
}

#[test]
fn oir2_scalar_golden() {
    golden("module app v1.0.0; main() is int { let n = -7 + 3 * 4 - 2; n % 5; }",
        include_str!("goldens/scalar.oir"));
}

#[test]
fn oir2_block_shadow_return_golden() {
    golden("module app; main() is int { let n = 1; { let n = 2; n; } return n; }",
        include_str!("goldens/block.oir"));
}

#[test]
fn oir2_if_demand_golden() {
    golden("module app; main() is int { if true { 8; } let v = if false { 1; } else { 2; }; v; }",
        include_str!("goldens/if.oir"));
}

#[test]
fn oir2_logical_golden() {
    golden("module app; main() is int { let b = false && (1 % 0 == 0) || !false; if b { 1; } else { 0; } }",
        include_str!("goldens/logical.oir"));
}

#[test]
fn oir2_match_golden() {
    golden("module app; main() is int { match \"b\" { case \"a\", \"b\" { 2; } else { 0; } } }",
        include_str!("goldens/match.oir"));
}

#[test]
fn oir2_loop_golden() {
    golden("module app; main() is int { let n = loop { next if false; last 7; }; n; }",
        include_str!("goldens/loop.oir"));
}

#[test]
fn oir2_range_for_golden() {
    golden("module app; main() is int { let r = 1..=3; for 0..3 -> i { next if i == 1; last if i == 2; } 0; }",
        include_str!("goldens/for.oir"));
}

#[test]
fn oir2_valued_for_golden() {
    golden("module app; main() is int { for 1..=3 -> i { last 9 if i == 2; i; } }",
        include_str!("goldens/for-value.oir"));
}

#[test]
fn oir2_declarations_golden() {
    golden(concat!("module app; enum Stop is Error = (stop, failed); ",
        "enum Color = (red, blue); main() is int { 0; }"),
        include_str!("goldens/declarations.oir"));
}

#[test]
fn oir2_scalar_builtins_golden() {
    golden("module app; main() is int { let s = true.string() + 12.string(); let b = s != \"x\"; s.length(); }",
        include_str!("goldens/builtins.oir"));
}

#[test]
fn oir2_unsupported_source_constructs_are_diagnostics() {
    for (body, expected) in [
        ("let a = [1]; 0;", "collection"),
        ("defer [1]; 0;", "collection"),
        ("let n = try? 1; 0;", "try?"),
        ("1.5; 0;", "constant"),
        ("1 & 2;", "binary"),
        ("1 / 2; 0;", "binary"),
    ] {
        let source = format!("module app; main() is int {{ {body} }}");
        let error = print_oir2(&source).expect_err("unsupported program must stop");
        let LowerError::Unsupported { message, span } = error else {
            panic!("wrong diagnostic: {error}");
        };
        assert!(message.contains("unsupported in OIR-2 lowering"), "{message}");
        assert!(message.contains(expected), "{message}");
        assert!(!span.is_empty());
    }
}

#[test]
fn oir2_calls_and_async_do_not_enter_the_old_pipeline() {
    for (source, expected) in [
        ("module app; async main() is int { 0; }", "async"),
        ("module app; object O { O(); f() is int { once let x = 1; x; } } main() is int { 0; }", "once let"),
    ] {
        let error = print_oir2(source).expect_err("explicit unsupported diagnostic");
        assert!(matches!(error, LowerError::Unsupported { .. }), "{error}");
        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[test]
fn oir2_rebind_keeps_original_binding_across_loop_and_join() {
    // The source frontend only permits successor rebinds. S1 cannot construct
    // those until S2, so exercise the typed scalar CFG directly after analysis.
    let source = "module app; main() is int { let n = 0; loop { let update = n + 1; last if n > 2; } n; }";
    with_analysis(source, |analysis| {
        let body = analysis.bodies.iter_mut().find(|b| b.owner == thir::BodyOwner::Entry).unwrap();
        let thir::StmtKind::Expr(thir::Expr { kind: thir::ExprKind::Loop(block), .. }) =
            &mut body.block.stmts[1].kind else {
            panic!("loop fixture");
        };
        let thir::StmtKind::Let { value, .. } = block.stmts.remove(0).kind else {
            panic!("update fixture");
        };
        block.stmts.insert(0, thir::Stmt {
            span: value.span,
            kind: thir::StmtKind::Rebind { local: thir::LocalId(0), value },
        });
        let module = lower_to_oir2(analysis).unwrap();
        let text = ir::text::print(&module).unwrap();
        verify_high(&module, "direct rebind lowering");
        let parsed = ir::text::parse(&text).unwrap();
        verify_high(&parsed, "printed rebind lowering");
        assert!(ir::text::normalized_eq(&module, &parsed).unwrap());
        compare_golden("rebind", &text, include_str!("goldens/rebind.oir"));
    });
}

#[test]
fn oir2_scalar_allowlist_golden() {
    golden(concat!(
        "module app; main() is int { ",
        "let a = 1; let b = 2; let p = true; let q = false; let s = \"a\"; let t = \"b\"; ",
        "-a; a == b; a != b; a < b; a <= b; a > b; a >= b; p == q; p != q; ",
        "s == t; s != t; s.less(t); s.less_or_equal(t); t.less(s); t.less_or_equal(s); int(a); 0; }"),
        include_str!("goldens/allowlist.oir"));
}

#[test]
fn oir2_wanted_block_golden() {
    golden("module app; main() is int { { 3; } }", include_str!("goldens/block-value.oir"));
}

#[test]
fn oir2_infinite_loop_golden() {
    golden("module app; main() is int { loop { next; } }", include_str!("goldens/infinite.oir"));
}

#[test]
fn oir2_boundary_edge_cases_roundtrip() {
    for body in [
        "let n = 1; n; return n;",
        "let n = 1; int(n); return n;",
        "let n = 1; { n; } n;",
        "if true { return 1; } else { return 2; }",
        "loop { loop { last; } last; } 0;",
        "match true { case true { 1; } case false { 0; } }",
        "match 3 { case 1 { 1; } } 0;",
        "for 0..0 -> i { next; } 0;",
        "for 0..3 -> i { last; } 0;",
        "for 0..3 -> i { return i; } 0;",
        "for 0..3 -> i { last i; }",
        "for 0..1 -> i { i; }",
        "for 0..3 -> i { next if i == 1; last if i == 2; i; }",
        "let r = 0..3; for r -> i { next; } 0;",
    ] {
        print_oir2(&format!("module app; main() is int {{ {body} }}")).unwrap();
    }
}

#[test]
fn oir2_declared_unsupported_bodies_name_the_function() {
    for (source, name, construct) in [
        ("module app; object O { O() { try { 1 & 2; } catch e is Error { 0; } } } main() is int { 0; }",
            "app.O.new#0", "binary"),
        ("module app; object O { O(); destroy { 1 & 2; } } main() is int { 0; }",
            "app.O.destroy", "binary"),
        ("module app; object O { has n is int = 1 & 2; O(); } main() is int { 0; }",
            "app.O.default#0", "binary"),
    ] {
        let error = print_oir2(source).expect_err("unsupported declared body must stop the module");
        let LowerError::Unsupported { span, message } = error else {
            panic!("wrong diagnostic: {error}");
        };
        assert!(!span.is_empty());
        assert!(message.contains("unsupported in OIR-2 lowering"), "{message}");
        assert!(message.contains(name), "{message}");
        assert!(message.contains(construct), "{message}");
    }
}

#[test]
fn oir2_default_method_and_constructor_bodies_are_lowered() {
    let source = concat!("module app; object O { has n is int = 3; O(); ",
        "calc(x is int) is int { x + 1; } } main() is int { 0; }");
    with_analysis(source, |analysis| {
        let module = lower_to_oir2(analysis).unwrap();
        for name in ["app.O.default#0", "app.O.calc#0", "app.O.new#0", "main"] {
            let function = module.functions.iter().find(|(_, function)| function.symbol == name).unwrap().1;
            assert!(function.body.is_some(), "{name}");
        }
        super::structure::check(&module).unwrap();
    });
}

#[test]
fn oir2_verifier_rejects_missing_or_conflicting_function_forms() {
    let module = ir::text::parse(include_str!("goldens/scalar.oir")).unwrap();
    let id = module.entry.unwrap();
    for attrs in [
        vec![],
        vec![ir::FunctionAttr::Extern(String::new())],
        vec![ir::FunctionAttr::Extern("ffi".into()), ir::FunctionAttr::Extern("other".into())],
    ] {
        let mut broken = module.clone();
        let function = broken.functions.get_mut(id).unwrap();
        function.body = None;
        function.attrs = attrs;
        assert!(check_verified(&broken).is_err());
    }
    for attr in [
        ir::FunctionAttr::Extern("ffi".into()),
        ir::FunctionAttr::Intrinsic { name: "intrinsic".into(), objects: vec![], helpers: vec![] },
    ] {
        let mut external = module.clone();
        external.functions.get_mut(id).unwrap().attrs.push(attr);
        assert!(check_verified(&external).is_err(), "body and ABI conflict");
        external.functions.get_mut(id).unwrap().body = None;
        external.functions.get_mut(id).unwrap().attrs.retain(|attr| *attr != ir::FunctionAttr::Entry);
        external.entry = None;
        let report = ir::verify::verify_high(&external).unwrap_err();
        assert!(report.diagnostics.iter().all(|diagnostic| diagnostic.rule != ir::verify::RuleId::S1));
        assert!(report.diagnostics.iter().any(|diagnostic| diagnostic.rule == ir::verify::RuleId::T5));
    }
}

#[test]
fn oir2_verifier_rejects_nonempty_unreachable_blocks() {
    let module = ir::text::parse(include_str!("goldens/infinite.oir")).unwrap();
    let id = module.entry.unwrap();
    for mutation in 0..4 {
        let mut broken = module.clone();
        let body = broken.functions.get_mut(id).unwrap().body.as_mut().unwrap();
        let dead = body.blocks.iter().find(|(_, block)| block.term.kind == ir::TermKind::Unreachable).unwrap().0;
        let failure = body.blocks.iter().find(|(_, block)| matches!(block.term.kind, ir::TermKind::Raise(_)))
            .unwrap().0;
        let value = body.blocks.get(failure).unwrap().params[0];
        let instruction = body.insts.iter().next().unwrap().0;
        let block = body.blocks.get_mut(dead).unwrap();
        match mutation {
            0 => block.params.push(value),
            1 => block.insts.push(instruction),
            2 => block.unwind = Some(ir::BlockUnwind { target: ir::Target { block: failure, args: vec![] } }),
            _ => block.term.kind = ir::TermKind::Ret { values: vec![] },
        }
        let error = check_verified(&broken).expect_err("nonempty tombstone");
        assert!(error.to_string().contains("empty unreachable tombstone"), "{error}");
    }
}

#[test]
fn oir2_self_check_rejects_unterminated_reachable_draft() {
    with_analysis("module app; main() is int { 0; }", |analysis| {
        let mut declarations = super::declarations::Declarations::new(analysis).unwrap();
        let function = declarations.functions[&thir::BodyOwner::Entry];
        let root = declarations.builder.declare_scope(ir::ScopeDecl {
            function, parent: None, kind: ir::ScopeKind::Action, loc: None,
        }).unwrap();
        let builder = declarations.builder.function_builder(function, root).unwrap();
        let entry = builder.entry();
        assert_eq!(builder.finish().unwrap_err(), ir::BuildError::Unterminated(vec![entry]));
    });
}

#[test]
fn oir2_diverging_continuations_with_locals_are_empty_tombstones() {
    for body in [
        "let n = 1; loop { next; } n;",
        "let n = 1; { return n; }",
        "let n = 1; if true { return n; } else { return 2; }",
        "let n = 1; match n { case 1 { return n; } else { return 2; } }",
        "let n = 1; for 0..3 -> i { return n; } 0;",
        "let n = 1; loop { loop { next; } }",
    ] {
        print_oir2(&format!("module app; main() is int {{ {body} }}")).unwrap();
    }
}

fn s2_golden(name: &str, source: &str, expected: &str) {
    let actual = print_oir2(source).unwrap_or_else(|error| panic!("{name}: {error}"));
    compare_golden(name, &actual, expected);
}

fn compare_golden(name: &str, actual: &str, expected: &str) {
    if std::env::var_os("OKKAMA_OIR2_UPDATE_GOLDENS").is_some() {
        let path = format!("{}/src/lowering/oir2/goldens/{name}.oir", env!("CARGO_MANIFEST_DIR"));
        std::fs::write(path, actual).expect("regenerated golden");
    } else {
        let parsed = ir::text::parse(expected).expect("stored golden parses");
        verify_high(&parsed, name);
        assert_eq!(actual, expected, "{name}");
    }
}

#[test]
fn oir2_constructor_default_golden() {
    s2_golden("constructor-default", concat!("module app; object O { has n is int = 3; O(:n); } ",
        "main() is int { O(); O(n: 9); 0; }"), include_str!("goldens/constructor-default.oir"));
}

#[test]
fn oir2_constructor_full_golden() {
    s2_golden("constructor-full", concat!("module app; object O { has n is int; ",
        "O(n is int) { self.n = n + 1; } } main() is int { O(4); 0; }"),
        include_str!("goldens/constructor-full.oir"));
}

#[test]
fn oir2_method_fresh_receiver_and_temporary_field_golden() {
    s2_golden("method-fresh", concat!("module app; object O { has n is int = 3; O(); ",
        "value() is int { n; } } main() is int { O().value(); }"),
        include_str!("goldens/method-fresh.oir"));
}

#[test]
fn oir2_call_bound_and_fresh_arguments_golden() {
    s2_golden("call-mixed", concat!("module app; object O { O(); ",
        "accept(a is O, b is O) is void { a; b; } } ",
        "main() is int { let bound = O(); O().accept(bound, O()); 0; }"),
        include_str!("goldens/call-mixed.oir"));
}

#[test]
fn oir2_successor_staging_golden() {
    s2_golden("successor-stage", concat!("module app; object O { has n is int = 0; O(); ",
        "bump() is ^O { { ^n = n + 1; } ^n = n + 2; } } ",
        "main() is int { let o = O(); o = o.bump(); 0; }"),
        include_str!("goldens/successor-stage.oir"));
}

#[test]
fn oir2_early_self_golden() {
    s2_golden("early-self", concat!("module app; object O { has n is int; ",
        "O(n is int) { let alias = self; self.n = n; } } main() is int { O(5); 0; }"),
        include_str!("goldens/early-self.oir"));
}

#[test]
fn oir2_plain_error_values_golden() {
    s2_golden("error-values", "module app; enum Stop is Error = (now, later); main() is int { Stop.now; 0; }",
        include_str!("goldens/error-values.oir"));
}

#[test]
fn oir2_call_argument_boundaries_follow_all_evaluations_golden() {
    s2_golden("call-order", concat!("module app; object O { O(); next() is ^O { } ",
        "accept(a is O, b is O) is O { b; } } ",
        "main() is int { O().accept(O().next(), O()); 0; }"), include_str!("goldens/call-order.oir"));
}

#[test]
fn oir2_object_fields_and_inherited_successor_golden() {
    s2_golden("object-fields", concat!("module app; object Item { Item(); } ",
        "object Holder { has item is Item = Item(); Holder(:item); ",
        "get() is Item { item; } copy() is ^Holder { } } ",
        "main() is int { let h = Holder(item: Item()); h = h.copy(); h.get(); 0; }"),
        include_str!("goldens/object-fields.oir"));
}

#[test]
fn oir2_construction_self_check_rejects_broken_early_self() {
    let module = ir::text::parse(include_str!("goldens/early-self.oir")).unwrap();
    let ctor = module.functions.iter().find(|(_, function)| {
        function.attrs.iter().any(|attr| matches!(attr, ir::FunctionAttr::Ctor(_)))
    }).unwrap().0;
    for mutation in 0..7 {
        let mut broken = module.clone();
        let body = broken.functions.get_mut(ctor).unwrap().body.as_mut().unwrap();
        let entry = body.entry;
        let inits: Vec<_> = body.blocks.get(entry).unwrap().insts.iter().copied().filter(|id| {
            matches!(body.insts.get(*id).unwrap().op, ir::Operation::Core(ir::CoreOp::ObjInit { .. }))
        }).collect();
        match mutation {
            0 => {
                let ir::Operation::Core(ir::CoreOp::ObjInit { object, field, .. }) =
                    body.insts.get(inits[0]).unwrap().op else { unreachable!() };
                body.insts.get_mut(inits[0]).unwrap().op = ir::Operation::Core(ir::CoreOp::ObjGet { object, field });
            }
            1 => {
                let block = body.blocks.get_mut(entry).unwrap();
                let index = block.insts.iter().position(|id| *id == inits[0]).unwrap();
                block.insts.insert(index, inits[0]);
            }
            2 => body.blocks.get_mut(entry).unwrap().insts.retain(|id| *id != inits[1]),
            3 => {
                let block = body.blocks.get_mut(entry).unwrap();
                let first = block.insts.iter().position(|id| *id == inits[0]).unwrap();
                let second = block.insts.iter().position(|id| *id == inits[1]).unwrap();
                block.insts.swap(first, second);
            }
            4 => {
                for (_, inst) in body.insts.iter_mut() {
                    if let ir::Operation::Core(ir::CoreOp::ObjNew { construction, .. }) = &mut inst.op {
                        *construction = ir::ConstructionStatus::Complete;
                    }
                }
            }
            5 => {
                for (_, inst) in body.insts.iter_mut() {
                    if inst.op == ir::Operation::Const(ir::Literal::Bool(false)) {
                        inst.op = ir::Operation::Const(ir::Literal::Bool(true));
                    }
                }
            }
            _ => {
                let block = body.blocks.get_mut(entry).unwrap();
                let index = block.insts.iter().position(|id| *id == inits[1]).unwrap();
                block.insts.insert(index, inits[1]);
            }
        }
        let error = super::structure::check(&broken).expect_err("invalid construction must fail self-check");
        assert!(error.to_string().contains("construction"), "{mutation}: {error}");
    }
}

#[test]
fn oir2_s2_control_flow_and_aliases_roundtrip() {
    for source in [
        concat!("module app; object O { has n is int; O(b is bool) { ",
            "if b { n = 1; } else { n = 2; } } } main() is int { O(true); 0; }"),
        concat!("module app; object O { has n is int; O(b is bool) { ",
            "if b { let alias = self; n = 1; } else { let alias = self; n = 2; } } } ",
            "main() is int { O(true); 0; }"),
        concat!("module app; object O { has n is int = 1; O(); ",
            "f(b is bool) is ^O { if b { ^n = 2; } else { ^n = 3; } } } ",
            "main() is int { let o = O(); o = o.f(true); 0; }"),
        concat!("module app; object O { has n is int = 1; O(); ",
            "f(b is bool) is ^O { ^n = 2; return if b; ^n = 3; } } ",
            "main() is int { let o = O(); o = o.f(true); 0; }"),
        concat!("module app; object O { O(); accept(a is O, b is O) is void { } } ",
            "main() is int { let o = O(); o.accept(o, o); 0; }"),
        concat!("module app; object O { has a is int = 1; has b is int = 2; O(:b, :a) { a; b; } } ",
            "main() is int { O(); O(a: 4); O(a: 3, b: 5); 0; }"),
    ] {
        print_oir2(source).unwrap_or_else(|error| panic!("{source}\n{error}"));
    }
}

#[test]
fn oir2_s2_unsupported_bodies_name_the_function() {
    for (source, name) in [
        ("module app; object O { O() { try { 1 & 2; } catch e is Error { 2; } } } main() is int { 0; }",
            "app.O.new#0"),
        ("module app; object O { O(); f() is int { 1 & 2; } } main() is int { 0; }", "app.O.f#0"),
        ("module app; object O { has n is int = 1 & 2; O(); } main() is int { 0; }",
            "app.O.default#0"),
        ("module app; object O { O(); destroy { 1 & 2; } } main() is int { 0; }", "app.O.destroy"),
        (concat!("module app; object O { has n is int = 1; O() { let alias = self; n = 2; } } ",
            "main() is int { 0; }"), "app.O.new#0"),
        (concat!("module app; object O { has n is int; O(b is bool) { ",
            "if b { n = 1; } let v = self.n; n = 2; } } main() is int { 0; }"), "app.O.new#0"),
    ] {
        let error = print_oir2(source).expect_err("#567 body remains unsupported");
        assert!(matches!(error, LowerError::Unsupported { .. }), "{error}");
        assert!(error.to_string().contains("unsupported in OIR-2 lowering"), "{error}");
        assert!(error.to_string().contains(name), "{error}");
    }
}

#[test]
fn oir2_pending_layout_and_creation_follow_constructor_self_escape() {
    let source = concat!(
        "module app; object Empty { Empty(); } ",
        "object Plain { has n is int; Plain(n is int) { self.n = n; } ",
        "value() is int { n; } } ",
        "object Early { has n is int; Early() { n = 0; } ",
        "Early(n is int) { if true { let alias = self; } self.n = n; } ",
        "value() is int { n; } } ",
        "main() is int { Empty(); Plain(1).value(); Early().value(); Early(2).value(); 0; }",
    );
    with_analysis(source, |analysis| {
        let module = lower_to_oir2(analysis).unwrap();
        for (name, expected) in [
            ("app.Empty", vec![]),
            ("app.Plain", vec!["n"]),
            ("app.Early", vec!["n", "$pending"]),
        ] {
            let (_, declaration) = module.declarations.iter().find(|(_, decl)| decl.name == name).unwrap();
            let ir::DeclarationKind::Object { fields, .. } = &declaration.kind else {
                panic!("object layout");
            };
            assert_eq!(fields.iter().map(|field| field.name.as_str()).collect::<Vec<_>>(), expected);
            if name == "app.Early" {
                assert_eq!(module.types.get(fields.last().unwrap().ty).unwrap(), &ir::Type::Bool);
            }
        }
        let mut complete = 0;
        let mut incomplete = 0;
        for (_, function) in module.functions.iter() {
            let body = function.body.as_ref().unwrap();
            for (_, inst) in body.insts.iter() {
                let ir::Operation::Core(ir::CoreOp::ObjNew { declaration, fields, construction, .. }) = &inst.op
                else {
                    continue;
                };
                let name = &module.declarations.get(*declaration).unwrap().name;
                if *construction == ir::ConstructionStatus::Incomplete {
                    assert_eq!(name, "app.Early");
                    incomplete += 1;
                    continue;
                }
                complete += 1;
                match name.as_str() {
                    "app.Empty" => assert!(fields.is_empty()),
                    "app.Plain" => assert_eq!(fields.len(), 1),
                    "app.Early" => {
                        assert_eq!(fields.len(), 2);
                        let ir::Definition::InstResult { inst, .. } =
                            body.values.get(*fields.last().unwrap()).unwrap().def else {
                            panic!("literal pending completion");
                        };
                        assert_eq!(body.insts.get(inst).unwrap().op, ir::Operation::Const(ir::Literal::Bool(false)));
                    }
                    _ => panic!("unexpected object {name}"),
                }
            }
        }
        assert_eq!(complete, 3);
        assert_eq!(incomplete, 1);
        let main = module.functions.get(module.entry.unwrap()).unwrap().body.as_ref().unwrap();
        let pending: Vec<_> = main.insts.iter().filter_map(|(_, inst)| {
            if let ir::Operation::Core(ir::CoreOp::ObjPending { field, .. }) = &inst.op {
                Some(field)
            } else {
                None
            }
        }).collect();
        assert_eq!(pending.len(), 2);
        for field in pending {
            assert_eq!(module.declarations.get(field.declaration).unwrap().name, "app.Early");
            assert_eq!(field.index, 1);
        }
    });
}

#[test]
fn oir2_scopes_and_defer_goldens() {
    let object = "module app; object Res { has name is string; Res(s is string) { name = s; } ";
    let object = format!("{object}label() is string {{ name; }} renamed(s is string) is ^Res {{ ^name = s; }} }} ");
    for (name, body, expected) in [
        ("scope-temp", "{ { Res(\"inner\"); 1; } Res(\"outer\"); 2; } 0;",
            include_str!("goldens/scope-temp.oir")),
        ("discard-resource-branch", "if true { Res(\"A\"); } Res(\"checkpoint\"); 0;",
            include_str!("goldens/discard-resource-branch.oir")),
        ("discard-block-let", "{ let r = Res(\"bound\"); r; } 0;",
            include_str!("goldens/discard-block-let.oir")),
        ("defer-rebind-loop", "let r = Res(\"A\"); defer r.label(); for 1..=3 -> i { r = r.renamed(\"B\"); } 0;",
            include_str!("goldens/defer-rebind-loop.oir")),
        ("defer-lifo", "let r = Res(\"A\"); defer r.label(); defer Res(\"second\").label(); 0;",
            include_str!("goldens/defer-lifo.oir")),
        ("defer-failure-prefix", "let r = Res(\"A\"); r.label(); defer r.label(); r.label(); 0;",
            include_str!("goldens/defer-failure-prefix.oir")),
        ("defer-iteration", "for 0..3 -> i { let r = Res(\"iteration\"); defer r.label(); next if i == 1; } 0;",
            include_str!("goldens/defer-iteration.oir")),
    ] {
        s2_golden(name, &format!("{object}main() is int {{ {body} }}"), expected);
    }
}

#[test]
fn oir2_defer_lexical_captures_roundtrip() {
    for source in [
        "module app; main() is int { defer 1 % 0; 0; }",
        "module app; main() is int { defer 1; defer 2; 0; }",
        "module app; main() is int { if true { defer 1; } else { defer 2; } defer 3; 0; }",
        "module app; main() is int { let n = 1; { let n = 2; defer n; } defer n; 0; }",
        "module app; main() is int { defer if true { let n = 1; defer n; n; } else { 0; }; 0; }",
        "module app; main() is int { for 0..3 -> i { defer i; next; } 0; }",
        "module app; main() is int { loop { defer 1; next if false; last; } 0; }",
        "module app; object O { has n is int = 1; O(); f() is void { defer n; defer self; } } main() is int { 0; }",
        "module app; object O { O() { defer self; } } main() is int { O(); 0; }",
        "module app; object O { has n is int; O() { n = 1; defer self.n; } } main() is int { O(); 0; }",
        "module app; main() is int { let n = 1; defer if true { for 0..2 -> i { i + n; } } else { 0; }; 0; }",
    ] {
        print_oir2(source).unwrap_or_else(|error| panic!("{source}\n{error}"));
    }
}

#[test]
fn oir2_outside_s3_constructs_report_unsupported() {
    for body in [
        "defer if true { return 1; } else { 0; }; 0;",
        "let n = try? 1; 0;",
        "try { throw Stop.now; } catch Stop.now { 0; }",
    ] {
        let source = format!("module app; enum Stop is Error = (now); main() is int {{ {body} }}");
        let error = print_oir2(&source).expect_err("outside the slice must remain unsupported");
        assert!(matches!(error, LowerError::Unsupported { .. }), "{error}");
        assert!(error.to_string().contains("unsupported in OIR-2 lowering"), "{error}");
    }
}

fn check_high_fixture(body: &str) -> Result<(), LowerError> {
    let source = format!(r#"oir 2 high
module prefixes
requires core
scope SD action in @d
scope S action in @f
scope SI iteration in @f parent S
func @d() -> void {{
^entry in SD:
    ret
}}
func @f(%c: bool, %x: int) -> void {{
{body}
}}
"#);
    let module = ir::text::parse(&source).unwrap();
    check_verified(&module)
}

#[test]
fn oir2_verifier_defer_prefixes_at_merges_and_failure_points() {
    for (body, expected) in [
        (r#"^entry in S:
    cond %c, ^left, ^right
^left in S:
    defer.register S, action defer_action(function=@d)
    br ^done
^right in S:
    br ^done
^done in S:
    ret"#, "H3"),
        (r#"^entry in S:
    br ^again
^again in S:
    defer.register S, action defer_action(function=@d)
    br ^again"#, "H3"),
        (r#"^entry in S unwind ^failed:
    %a: int = int.mod %x, %x
    defer.register S, action defer_action(function=@d)
    %b: int = int.mod %x, %x
    ret
^failed(%e: failure) in S:
    raise %e"#, "H3"),
        (r#"^entry in S:
    defer.register SI, action defer_action(function=@d)
    ret"#, "H3"),
        (r#"^entry in S:
    cond %c, ^left, ^right
^left in SI:
    ret
^right in SI:
    ret"#, "H1"),

        (r#"^entry in S:
    cond %c, ^left, ^right
^left in S:
    defer.register S, action defer_action(function=@d)
    ret
^right in S:
    defer.register S, action defer_action(function=@d)
    ret"#, "H3"),
        (r#"^entry in S:
    br ^nested
^nested in SI:
    %a: int = int.mod %x, %x
    ret"#, "H5"),
        (r#"^entry in S:
    temp %x, SI
    ret"#, "H8"),
    ] {
        let error = check_high_fixture(body).expect_err("invalid lexical high must fail self-check");
        assert!(error.to_string().contains(expected), "{error}");
    }
    check_high_fixture(r#"^entry in S unwind ^before_failed:
    %a: int = int.mod %x, %x
    defer.register S, action defer_action(function=@d)
    br ^after
^after in S unwind ^after_failed:
    %b: int = int.mod %x, %x
    ret
^before_failed(%e: failure) in S:
    raise %e
^after_failed(%f: failure) in S:
    raise %f"#).unwrap();
    check_high_fixture(r#"^entry in S:
    br ^head
^head in S:
    br ^iteration
^iteration in SI:
    defer.register SI, action defer_action(function=@d)
    br ^head"#).unwrap();
}

#[test]
fn oir2_verifier_defer_bindings_need_initial_dominance() {
    let module = ir::text::parse(include_str!("goldens/defer-failure-prefix.oir")).unwrap();
    for mutation in 0..3 {
        let mut broken = module.clone();
        let body = broken.functions.get_mut(broken.entry.unwrap()).unwrap().body.as_mut().unwrap();
        let entry = body.entry;
        let data = body.blocks.get_mut(entry).unwrap();
        let bind = data.insts.iter().position(|&id| {
            matches!(body.insts.get(id).unwrap().op, ir::Operation::Lexical(ir::LexicalOp::Bind { .. }))
        }).unwrap();
        let instruction = data.insts.remove(bind);
        match mutation {
            0 => {}
            1 => data.insts.push(instruction),
            _ => {
                data.insts.insert(bind, instruction);
                data.insts.insert(bind, instruction);
            }
        }
        let error = check_verified(&broken).expect_err("invalid initial binding");
        let expected = if mutation == 1 { "H7" } else { "S3" };
        assert!(error.to_string().contains(expected), "{error}");
    }
}

fn body_for_entry(module: &ir::Module) -> &ir::Body {
    module.functions.get(module.entry.unwrap()).unwrap().body.as_ref().unwrap()
}

#[test]
fn oir2_defer_receiver_snapshots_and_binding_arguments_are_lazy() {
    let source = concat!("module app; object O { has n is int = 1; O(); ",
        "f(p is int) is void { defer n + p; } } main() is int { 0; }");
    with_analysis(source, |analysis| {
        let module = lower_to_oir2(analysis).unwrap();
        let function = module.functions.iter().find(|(_, f)| f.symbol == "app.O.f#0").unwrap().1;
        let body = function.body.as_ref().unwrap();
        let action = body.insts.iter().find_map(|(_, inst)| {
            if let ir::Operation::Lexical(ir::LexicalOp::DeferRegister { action, .. }) = &inst.op {
                Some(action)
            } else {
                None
            }
        }).unwrap();
        assert_eq!(action.captures.len(), 1);
        assert_eq!(action.captures[0], body.blocks.get(body.entry).unwrap().params[0]);
        assert_eq!(action.bindings.len(), 1);
        assert_eq!(body.bindings.get(action.bindings[0]).unwrap().name, "p");
        assert!(!body.insts.iter().any(|(_, inst)| matches!(inst.op, ir::Operation::Core(_))));
        let deferred = module.functions.get(action.function).unwrap();
        assert_eq!(deferred.signature.params, function.signature.params);
        let deferred = deferred.body.as_ref().unwrap();
        assert!(deferred.insts.iter().any(|(_, inst)| {
            matches!(inst.op, ir::Operation::Core(ir::CoreOp::ObjGet { .. }))
        }));
        assert!(deferred.insts.iter().any(|(_, inst)| {
            matches!(inst.op, ir::Operation::Core(ir::CoreOp::IntAdd { .. }))
        }));
    });
}

#[test]
fn oir2_defer_nonlocal_writes_remain_unsupported() {
    for source in [
        concat!("module app; object O { O(); bump() is ^O { } } main() is int { let o = O(); ",
            "defer if true { o = o.bump(); } else { 0; }; 0; }"),
        concat!("module app; object O { has n is int = 0; O(); bump() is ^O { ",
            "defer if true { ^n = 1; } else { 0; }; } } main() is int { 0; }"),
    ] {
        let error = print_oir2(source).expect_err("nonlocal defer writes require later outcome plumbing");
        assert!(matches!(error, LowerError::Unsupported { .. }), "{error}");
        assert!(error.to_string().contains("unsupported in OIR-2 lowering"), "{error}");
    }
}

const S3_RES: &str = concat!(
    "object Res { has name is string; Res(s is string) { name = s; } ",
    "destroy { name; } } ",
);

fn s3_sources() -> Vec<(&'static str, String)> {
    let prelude = format!("module app; enum Stop is Error = (now); {S3_RES}");
    vec![
        ("s3-defer-risky", format!(concat!("{}object Work {{ Work(); risky() is void {{ throw Stop.now; }} ",
            "log() is void {{ }} }} main() is int {{ {{ let r = Res(\"r\"); ",
            "defer Work().log(); Work().risky(); }} 0; }}"), prelude)),
        ("s3-catch", "module app; enum Stop is Error = (now); main() is int { \
            try { throw Stop.now; } catch e is Stop { e; 7; } }".into()),
        ("s3-unmatched", "module app; enum Stop is Error = (now); enum Other is Error = (bad); \
            main() is int { try { throw Other.bad; } catch e is Stop { 7; } }".into()),
        ("s3-rethrow", "module app; enum Stop is Error = (now); main() is int { \
            try { throw Stop.now; } catch e is Stop { throw e; } }".into()),
        ("s3-cause", "module app; enum Stop is Error = (now); enum Other is Error = (bad); \
            main() is int { try { throw Stop.now; } catch e is Stop { throw Other.bad; } }".into()),
        ("s3-destroy-local", format!(concat!("{}object Owner {{ Owner(); destroy {{ let local = Res(\"local\"); ",
            "self; throw Stop.now; }} }} main() is int {{ Owner(); 0; }}"), prelude)),
        ("s3-return-nested", format!(concat!("{}object Work {{ Work(); make() is Res {{ ",
            "{{ let r = Res(\"nested\"); return r; }} }} }} main() is int {{ Work().make(); 0; }}"), prelude)),
        ("s3-return-try", format!(concat!("{}object Work {{ Work(); make() is Res {{ ",
            "try {{ let r = Res(\"try\"); return r; }} catch e is Stop {{ Res(\"catch\"); }} ",
            "}} }} main() is int {{ Work().make(); 0; }}"), prelude)),
        ("s3-late-argument", format!(concat!("{}object Work {{ Work(); take(a is Res, b is int) is void {{ }} ",
            "fail() is int {{ throw Stop.now; }} }} main() is int {{ ",
            "try {{ Work().take(Res(\"A\"), Work().fail()); }} catch e is Stop {{ e; }} 0; }}"), prelude)),
        ("s3-traps", "module app; enum Stop is Error = (now); object Work { Work(); \
            check(b is bool) is void { confirm b; } \
            expected(b is bool) is void { confirm b else throw Stop.now; } \
            emergency(b is bool) is void { confirm b else panic \"emergency\"; } } \
            main() is int { try { panic \"panic\"; } catch e is Stop { 0; } }".into()),
        ("s3-try-value", format!(concat!("{}main() is int {{ let r = try {{ Res(\"body\"); }} ",
            "catch e is Stop {{ Res(\"caught\"); }}; r; ",
            "try {{ Res(\"discard\"); }} catch e is Stop {{ Res(\"discard-catch\"); }} 0; }}"), prelude)),
    ]
}

#[test]
fn oir2_s3_source_goldens() {
    for (name, source) in s3_sources() {
        let actual = print_oir2(&source).unwrap_or_else(|error| panic!("{name}: {source}\n{error}"));
        let expected = match name {
            "s3-defer-risky" => include_str!("goldens/s3-defer-risky.oir"),
            "s3-catch" => include_str!("goldens/s3-catch.oir"),
            "s3-unmatched" => include_str!("goldens/s3-unmatched.oir"),
            "s3-rethrow" => include_str!("goldens/s3-rethrow.oir"),
            "s3-cause" => include_str!("goldens/s3-cause.oir"),
            "s3-destroy-local" => include_str!("goldens/s3-destroy-local.oir"),
            "s3-return-nested" => include_str!("goldens/s3-return-nested.oir"),
            "s3-return-try" => include_str!("goldens/s3-return-try.oir"),
            "s3-late-argument" => include_str!("goldens/s3-late-argument.oir"),
            "s3-traps" => include_str!("goldens/s3-traps.oir"),
            "s3-try-value" => include_str!("goldens/s3-try-value.oir"),
            _ => unreachable!("each S3 source has a reviewed high golden"),
        };
        compare_golden(name, &actual, expected);
    }
}

#[test]
fn oir2_s3_nested_handlers_and_lazy_failures_roundtrip() {
    for body in [
        "try { throw Stop.now; } catch e is Stop { let n = 2; n; }",
        "try { throw Stop.now; } catch e is Stop { throw; }",
        "try { throw Stop.now; } catch e is Stop { let alias = e; { throw alias; } }",
        "try { try { throw Stop.now; } catch e is Other { 1; } } catch e is Stop { 2; }",
        "try { throw Stop.now; } catch e is Stop, Other { throw e; }",
        "try { throw Stop.now; } catch e is Error { throw e; }",
        "try { throw Stop.now; } catch e is Stop { try { throw Other.bad; } catch x is Other { e; 1; } }",
        "try { throw Stop.now; } catch e is Stop { defer e; { let x = e; throw x; } }",
        "defer try { throw Stop.now; } catch e is Stop { e; }; 0;",
        "defer if true { throw Stop.now; } else { 0; }; 0;",
        "try { defer 1; throw Stop.now; } catch e is Stop { defer 2; 7; }",
        "try { 1; } catch e is Stop { 2; } catch e is Other { 3; }",
        "try { throw Stop.now; } catch e is Stop { let x = if true { e; } else { Stop.now; }; throw x; }",
        "try { throw Stop.now; } catch e is Stop { if true { throw e; } else { throw; } }",
        "try { loop { throw Stop.now; } } catch e is Stop { for 0..3 -> i { return i; } 0; }",
        "try { throw Stop.now; } catch e is Stop { let alias = if true { e; } else { e; }; throw alias; }",
        "try { throw Stop.now; } catch e is Stop { let alias = e; defer alias; { throw e; } }",
    ] {
        let source = format!("module app; enum Stop is Error = (now); enum Other is Error = (bad); \
            main() is int {{ {body} }}");
        print_oir2(&source).unwrap_or_else(|error| panic!("{source}\n{error}"));
    }
}

#[test]
fn oir2_s3_self_check_rejects_missing_unwind_and_dispatch_bypass() {
    let original = ir::text::parse(include_str!("goldens/s3-late-argument.oir")).unwrap();
    let body = body_for_entry(&original);
    let failing: Vec<_> = body.blocks.iter().filter_map(|(block, data)| {
        data.insts.iter().any(|&id| {
            matches!(body.insts.get(id).unwrap().op, ir::Operation::Call(_))
        }).then_some(block)
    }).collect();
    assert!(!failing.is_empty());
    for block in failing {
        let mut broken = original.clone();
        let body = broken.functions.get_mut(broken.entry.unwrap()).unwrap().body.as_mut().unwrap();
        body.blocks.get_mut(block).unwrap().unwind = None;
        let error = super::high::check(&broken, body_for_entry(&broken)).expect_err("call needs unwind");
        assert!(error.to_string().contains("H5"), "{error}");
    }
    let mut broken = ir::text::parse(include_str!("goldens/s3-catch.oir")).unwrap();
    let body = broken.functions.get_mut(broken.entry.unwrap()).unwrap().body.as_mut().unwrap();
    let dispatch = body.blocks.iter().find_map(|(id, block)| {
        matches!(block.term.kind, ir::TermKind::Catch { .. }).then_some(id)
    }).unwrap();
    body.blocks.get_mut(body.entry).unwrap().term.kind = ir::TermKind::Br(ir::Target {
        block: dispatch, args: vec![],
    });
    let error = super::high::check(&broken, body_for_entry(&broken)).expect_err("normal entry into dispatch");
    assert!(error.to_string().contains("without unwind"), "{error}");
}

#[test]
fn oir2_s3_self_check_catch_payload_and_binding_dominance() {
    let original = ir::text::parse(include_str!("goldens/s3-catch.oir")).unwrap();
    for mutation in 0..3 {
        let mut broken = original.clone();
        let body = broken.functions.get_mut(broken.entry.unwrap()).unwrap().body.as_mut().unwrap();
        let (bind, value) = body.insts.iter().find_map(|(id, inst)| match inst.op {
            ir::Operation::Lexical(ir::LexicalOp::Bind { value, .. }) => Some((id, value)),
            _ => None,
        }).unwrap();
        let try_block = body.blocks.iter().find_map(|(id, data)| {
            matches!(data.term.kind, ir::TermKind::Throw(_)).then_some(id)
        }).unwrap();
        match mutation {
            0 => body.blocks.get_mut(try_block).unwrap().term.kind = ir::TermKind::Throw(value),
            1 => {
                for (_, block) in body.blocks.iter_mut() {
                    block.insts.retain(|&id| id != bind);
                }
            }
            _ => {
                let entry = body.entry;
                for (_, block) in body.blocks.iter_mut() {
                    block.insts.retain(|&id| id != bind);
                }
                body.blocks.get_mut(entry).unwrap().insts.push(bind);
            }
        }
        let error = super::high::check(&broken, body_for_entry(&broken))
            .and_then(|()| check_verified(&broken)).expect_err("invalid catch binding");
        let expected = if mutation == 0 { "SSA2" } else { "catch" };
        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[test]
fn oir2_s3_self_check_requires_unwind_for_suspension() {
    let source = "oir 2 high
module suspend
requires core
scope S action in @f
        func @f(%future: future<int>) -> void {
^entry in S:
        %value: int = await %future, result int
ret
}
";
    let module = ir::text::parse(source).unwrap();
    let body = module.functions.iter().next().unwrap().1.body.as_ref().unwrap();
    let error = super::high::check(&module, body).expect_err("suspension needs unwind");
    assert!(error.to_string().contains("H5"), "{error}");
}

#[test]
fn oir2_s3_failure_observes_latest_outer_rebind() {
    let source = concat!("module app; enum Stop is Error = (now); ",
        "object Res { Res(); renew() is ^Res { } risky() is void { throw Stop.now; } } ",
        "main() is int { let r = Res(); try { r = r.renew(); r.risky(); } catch e is Stop { r; } 0; }");
    with_analysis(source, |analysis| {
        let module = lower_to_oir2(analysis).unwrap();
        let body = body_for_entry(&module);
        let (block, updated) = body.blocks.iter().find_map(|(_, block)| {
            block.insts.iter().find_map(|&id| match body.insts.get(id).unwrap().op {
                ir::Operation::Lexical(ir::LexicalOp::Rebind { value, .. }) => Some((block, value)),
                _ => None,
            })
        }).unwrap();
        let ir::TermKind::Br(target) = &block.term.kind else {
            panic!("a rebind must split before the next failing call");
        };
        assert_eq!(target.args[0], updated);
        let next = body.blocks.get(target.block).unwrap();
        assert_eq!(next.unwind.as_ref().unwrap().target.args[0], next.params[0]);
        let dispatch = body.blocks.get(next.unwind.as_ref().unwrap().target.block).unwrap();
        let ir::TermKind::Catch { handler, .. } = &dispatch.term.kind else {
            panic!("rebind failure leaves Try before catch selection");
        };
        assert_eq!(handler.args[0], ir::EdgeArg::Value(dispatch.params[0]));
    });
}

#[test]
fn oir2_s3_destroy_normal_locals_and_nested_resource_exits_roundtrip() {
    let prelude = format!("module app; enum Stop is Error = (now); {S3_RES}");
    for declarations in [
        concat!("object Owner { has name is string = \"owner\"; Owner(); destroy { ",
            "let local = Res(\"local\"); defer self.name; local; } } "),
        concat!("object Owner { Owner(); make() is Res { try { { let r = Res(\"r\"); ",
            "defer self; return r; } } catch e is Stop { return Res(\"catch\"); } } } "),
        concat!("object Owner { has name is string; Owner() { name = \"old\"; ",
            "try { self.name; throw Stop.now; } catch e is Stop { let alias = self; } } } "),
    ] {
        let source = format!("{prelude}{declarations}main() is int {{ Owner(); 0; }}");
        print_oir2(&source).unwrap_or_else(|error| panic!("{source}\n{error}"));
    }
}

fn verify_high(module: &ir::Module, context: &str) {
    assert_eq!(module.profile, ir::Profile::High);
    let catalog = super::native_summaries::P1NativeSummaries::for_module(module).expect("trusted P1 catalog");
    let context_with_catalog = ir::verify::VerifyContext { text_locations: None, external_contracts: &catalog };
    let checked = if module.functions.iter().all(|(_, function)| function.body.is_some()) {
        ir::verify::verify_high(module)
    } else {
        ir::verify::verify_with(module, &ir::verify::VerifyOptions::default(), context_with_catalog)
    };
    if let Err(report) = checked {
        panic!("{context}: {report:#?}");
    }
}

#[test]
fn oir2_all_stored_goldens_pass_verify_high() {
    let directory = format!("{}/src/lowering/oir2/goldens", env!("CARGO_MANIFEST_DIR"));
    let mut count = 0;
    for entry in std::fs::read_dir(directory).expect("golden directory") {
        let path = entry.expect("golden entry").path();
        if path.extension().is_none_or(|extension| extension != "oir") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read golden");
        let module = ir::text::parse(&text).expect("stored golden parses");
        verify_high(&module, path.to_str().expect("golden path"));
        count += 1;
    }
    assert_eq!(count, 44, "every lowering golden must be verified");
}

fn check_verified(module: &ir::Module) -> Result<(), LowerError> {
    ir::verify::verify_high(module).map(|_| ()).map_err(|report| {
        LowerError::invalid(crate::source::Span::default(), format!("{report:?}"))
    })
}

#[test]
fn oir2_d11_call_envelopes_preserve_catches_and_empty_d8_maps() {
    let source = concat!("module app; object O { O(); work() is void { } } ",
        "main() is int { let o = O(); defer o.work(); ",
        "try { o.work(); 1; } catch e is Error { 2; } }");
    with_analysis(source, |analysis| {
        let module = lower_to_oir2(analysis).unwrap();
        verify_high(&module, source);
        let mut calls = 0;
        let mut registrations = 0;
        let mut caught_call = false;
        for (function_id, function) in module.functions.iter() {
            let body = function.body.as_ref().unwrap();
            for (block, data) in body.blocks.iter() {
                for &instruction in &data.insts {
                    match &body.insts.get(instruction).unwrap().op {
                        ir::Operation::Call(ir::CallOp::Direct { call, .. }) => {
                            calls += 1;
                            assert!(call.summary.is_none());
                            assert_eq!(call.cleanup, ir::CallCleanupMode::Ordinary);
                            let effects = body.high_effects.iter().find(|record| {
                                record.site == ir::HighOperationSite::Instruction(instruction)
                            }).expect("summary(none) needs a site envelope");
                            assert_eq!(effects.effects.failures, vec![
                                ir::FailureKind::Thrown, ir::FailureKind::Program, ir::FailureKind::Panic,
                                ir::FailureKind::Native, ir::FailureKind::Cancelled,
                            ]);
                            assert!(effects.effects.may_suspend);
                            let unwind = data.unwind.as_ref().expect("unknown effects preserve unwind");
                            caught_call |= matches!(body.blocks.get(unwind.target.block).unwrap().term.kind,
                                ir::TermKind::Catch { .. });
                            if function.symbol == "main" {
                                let mut broken = module.clone();
                                let function = broken.functions.get_mut(function_id).unwrap();
                                let body = function.body.as_mut().unwrap();
                                body.blocks.get_mut(block).unwrap().unwind = None;
                                let report = ir::verify::verify_high(&broken).unwrap_err();
                                // Removing the route may first orphan the failure dispatch (S3).
                                assert!(report.diagnostics.iter().any(|diagnostic| {
                                    matches!(diagnostic.rule, ir::verify::RuleId::H5 | ir::verify::RuleId::S3)
                                }), "{report:?}");
                            }
                        }
                        ir::Operation::Lexical(ir::LexicalOp::DeferRegister { action, .. }) => {
                            registrations += 1;
                            assert!(action.replacements.is_empty());
                        }
                        _ => {}
                    }
                }
            }
        }
        assert_eq!(calls, 3);
        assert_eq!(registrations, 1);
        assert!(caught_call);
    });
}

#[test]
fn oir2_d11_local_discards_keep_their_scope_before_divergence() {
    for body in [
        "1; panic \"stop\";",
        "1; if true { return 2; } else { return 3; }",
        "1; 1; loop { next; }",
        "try { 1; throw Stop.now; } catch e is Stop { 2; }",
    ] {
        let source = format!("module app; enum Stop is Error = (now); main() is int {{ {body} }}");
        let text = print_oir2(&source).unwrap();
        let module = ir::text::parse(&text).unwrap();
        let body = body_for_entry(&module);
        let mut discarded = 0;
        for record in &body.high_results {
            if record.demand != ir::ResultDemand::Discard {
                continue;
            }
            discarded += 1;
            assert!(record.prepared.is_none());
            let block = body.blocks.get(record.block).unwrap();
            let target = match (&block.term.kind, record.edge) {
                (ir::TermKind::Br(target), ir::HighEdgeSelector::Normal(0)) => target,
                (ir::TermKind::Cond { yes, .. }, ir::HighEdgeSelector::Normal(0)) => yes,
                (ir::TermKind::Cond { no, .. }, ir::HighEdgeSelector::Normal(1)) => no,
                _ => panic!("local discard must precede an explicit ordinary edge"),
            };
            let target_scope = body.blocks.get(target.block).unwrap().lexical_scope;
            assert!(target_scope == block.lexical_scope
                || module.scopes.get(target_scope).unwrap().parent == Some(block.lexical_scope));
        }
        assert!(discarded > 0);
    }
}

struct StdLoader;

impl crate::lowering::ModuleLoader for StdLoader {
    fn load(&mut self, path: &str, _: &str) -> Result<Option<crate::lowering::LoadedModule>, String> {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std").join(format!("{path}.ok"));
        let text = std::fs::read_to_string(&file).map_err(|error| error.to_string())?;
        let source = SourceFile::new(file.to_string_lossy(), text).map_err(|error| error.to_string())?;
        let program = syntax::parse(&source, &ParseOptions::default()).map_err(|error| format!("{error:?}"))?;
        let program = syntax::prune::strip_test_items(program);
        Ok(Some(crate::lowering::LoadedModule { id: file.to_string_lossy().into_owned(), source, program }))
    }
}

fn lower_std(source: &str) -> Result<ir::Module, LowerError> {
    let file = SourceFile::new("oir2-std.ok", source).expect("small source");
    let program = syntax::parse(&file, &ParseOptions::default()).expect("valid source syntax");
    let roots = [(&program, "oir2-std.ok", Some(&file))];
    let graph = crate::lowering::modules::load_graph(&roots, &mut StdLoader).expect("local std graph");
    let modules = crate::lowering::modules::source_modules(&graph, &roots);
    let analysis = analysis::analyze(modules).unwrap_or_else(|errors| panic!("std analysis: {errors:?}"));
    lower_to_oir2(&analysis)
}

const STD_SOURCES: [(&str, &str, &str); 3] = [
    ("std-log", include_str!("fixtures/std-log.ok"), include_str!("goldens/std-log.oir")),
    ("std-cell", include_str!("fixtures/std-cell.ok"), include_str!("goldens/std-cell.oir")),
    ("std-error", include_str!("fixtures/std-error.ok"), include_str!("goldens/std-error.oir")),
];

#[test]
fn oir2_std_source_goldens_and_ownership_inference() {
    for (name, source, expected) in STD_SOURCES {
        let module = lower_std(source).unwrap_or_else(|error| panic!("{name}: {error}"));
        verify_high(&module, name);
        let actual = ir::text::print(&module).expect("print std high");
        let parsed = ir::text::parse(&actual).expect("std high round trip");
        verify_high(&parsed, name);
        assert!(ir::text::normalized_eq(&module, &parsed).unwrap());
        compare_golden(name, &actual, expected);
        let catalog = super::native_summaries::P1NativeSummaries::for_module(&module).unwrap();
        let inferred = ir::own::elaborate_ownership(module, &catalog, &ir::own::OwnOptions::default())
            .unwrap_or_else(|error| panic!("{name}: {error:#?}"));
        verify_high(&inferred.module, name);
        for (_, function) in inferred.module.functions.iter() {
            let summary = inferred.module.summaries.get(function.summary.expect("inferred summary")).unwrap();
            assert!(!summary.contract.suspends, "{name}: {}", function.symbol);
            assert!(!summary.contract.async_destroy);
        }
        // This branch exports classification/inference only (own/mod.rs). There is
        // no synchronous High -> Own rewrite to call. Never relabel High as Own:
        // call packets, paired deps, scope state and failure outcome edges need it.
        assert_eq!(inferred.module.profile, ir::Profile::High);
    }
}

#[test]
fn oir2_std_closed_specializations_and_error_cases() {
    for source in [
        concat!("module app; import \"async/atomic\" take Atomic, AtomicClosed; ",
            "import \"io/stdout\" take Stdout; main() is int { let a = Atomic(1); a.close(); ",
            "try { a.value(); } catch e is AtomicClosed { Stdout().writeln(e.text()); 0; } }"),
        concat!("module app; import \"async/atomic\" take Atomic; ",
            "main() is int { let a = Atomic(1); let b = Atomic(\"a\"); ",
            "a.replace(a.value() + 1); b.replace(b.value() + \"b\"); a.close(); b.close(); 0; }"),
        concat!("module app; import \"cell\" take Cell; ",
            "main() is int { let a = Cell(1); let b = Cell(\"a\"); ",
            "a.replace(a.value() + 1); b.replace(b.value() + \"b\"); a.value(); }"),
        concat!("module app; import \"io/stdout\" take Stdout; enum E is Error = (empty, text(s is string)); ",
            "main() is int { try { throw E.empty; } catch e is E { Stdout().writeln(e.text(), \"!\"); } 0; }"),
        concat!("module app; import \"io/stdout\" take Stdout; enum E is Error = (bad); ",
            "main() is int { Stdout().writeln(); try { throw E.bad; } catch e is E { ",
            "Stdout().writeln(e.text()); } 0; }"),
    ] {
        let module = lower_std(source).unwrap_or_else(|error| panic!("{source}\n{error}"));
        verify_high(&module, source);
        let catalog = super::native_summaries::P1NativeSummaries::for_module(&module).unwrap();
        let inferred = ir::own::elaborate_ownership(module, &catalog, &ir::own::OwnOptions::default())
            .unwrap_or_else(|error| panic!("{source}\n{error:#?}"));
        verify_high(&inferred.module, source);
    }
}

#[test]
fn oir2_std_rejects_other_intrinsics_resources_and_async() {
    for body in [
        "let a = Atomic(1); a.closed(); 0;",
        "let a = Atomic(1); a.compare_and_replace(1, 2); 0;",
        "let a = Atomic(R()); 0;",
        "let a = Cell(R()); 0;",
        "let action = () is int { 1; }; 0;",
    ] {
        let source = format!("module app; import \"async/atomic\" take Atomic; import \"cell\" take Cell; \
            object R {{ R(); destroy {{ }} }} main() is int {{ {body} }}");
        let error = lower_std(&source).expect_err("outside finite P1 boundary catalog");
        assert!(error.to_string().contains("unsupported in OIR-2 lowering"), "{error}");
    }
    let error = lower_std("module app; import \"async/atomic\" take Atomic; async main() is int { \
        let a = Atomic(1); a.value(); }").expect_err("async remains unsupported");
    assert!(error.to_string().contains("unsupported in OIR-2 lowering: async"), "{error}");
}

#[test]
fn oir2_std_boundary_catalog_is_finite_and_own_checks_entry_abi() {
    use ir::own::{ExternalBoundary, NativeSummaries, Substitution};
    let mut module = lower_std(include_str!("fixtures/std-log.ok")).unwrap();
    let catalog = super::native_summaries::P1NativeSummaries::for_module(&module).unwrap();
    for boundary in [
        ExternalBoundary::Intrinsic("unknown"),
        ExternalBoundary::Intrinsic("okkama.atomic.Atomic.closed$string"),
        ExternalBoundary::Native("runtime/stdio@1::stderr_write/1"),
        ExternalBoundary::Native("okkama.atomic.Atomic.value$string"),
        ExternalBoundary::Service(ir::TypeId::from_u32(u32::MAX)),
    ] {
        assert!(catalog.lookup(boundary, &Substitution::default()).is_err());
    }
    let substitution = Substitution { types: vec![ir::TypeId::from_u32(0)], descriptors: vec![] };
    assert!(catalog.lookup(ExternalBoundary::Intrinsic("okkama.atomic.Atomic.value$string"), &substitution).is_err());
    for (_, function) in module.functions.iter() {
        let Some(summary) = function.summary else {
            continue;
        };
        let summary = module.summaries.get(summary).unwrap();
        assert!(!summary.contract.suspends);
        assert!(!summary.contract.async_destroy);
        assert!(summary.parameters.iter().all(|param| param.retention.is_none() && param.deps.is_empty()));
        assert!(summary.results.iter().all(|result| result.aliases.is_empty() && result.external_deps.is_empty()));
    }
    module.profile = ir::Profile::Own;
    let context = ir::verify::VerifyContext { text_locations: None, external_contracts: &catalog };
    let report = ir::verify::verify_with(&module, &ir::verify::VerifyOptions::default(), context).unwrap_err();
    assert!(report.diagnostics.iter().any(|diagnostic| diagnostic.rule == ir::verify::RuleId::B51
        && diagnostic.message.contains("entry kind differs")), "{report:?}");
}

#[test]
fn oir2_std_p1_manifest_extractions_lower_with_destroy_log() {
    // #593 manifest references scopes checks 1-3 and branches checks 3-4.
    // Preserve those scopes/checkpoints here without importing general TAP.
    let prelude = concat!("module app; import \"async/atomic\" take Atomic; import \"io/stdout\" take Stdout; ",
        "object Res { has name is string; has log is Atomic<string>; ",
        "Res(name is string, log is Atomic<string>) { self.name = name; self.log = log; } ",
        "label() is string { name; } destroy { log.replace(log.value() + name); } } ");
    for body in [
        concat!("let log = Atomic(\"\"); defer log.close(); { let a = Res(\"A\", log); ",
            "{ let b = Res(\"B\", log); { let c = Res(\"C\", log); } } ",
            "confirm log.value() == \"CB\"; Stdout().writeln(log.value()); ",
            "{ let a = Res(\"S\", log); a.label(); } ",
            "confirm log.value() == \"CBS\"; Stdout().writeln(log.value()); a.label(); } ",
            "confirm log.value() == \"CBSA\"; Stdout().writeln(log.value()); 0;"),
        concat!("let log = Atomic(\"\"); defer log.close(); { ",
            "if false { Res(\"Z\", log); } else { Res(\"B\", log); }; ",
            "confirm log.value() == \"B\"; Stdout().writeln(log.value()); } ",
            "confirm log.value() == \"B\"; Stdout().writeln(log.value()); 0;"),
    ] {
        let source = format!("{prelude}main() is int {{ {body} }}");
        let module = lower_std(&source).unwrap_or_else(|error| panic!("{source}\n{error}"));
        verify_high(&module, &source);
        let catalog = super::native_summaries::P1NativeSummaries::for_module(&module).unwrap();
        let inferred = ir::own::elaborate_ownership(module, &catalog, &ir::own::OwnOptions::default())
            .unwrap_or_else(|error| panic!("{source}\n{error:#?}"));
        verify_high(&inferred.module, &source);
    }
}

#[test]
fn oir2_high_observation_requires_a_trusted_external() {
    with_analysis("module app; main() is int { 0; }", |analysis| {
        let mut module = lower_to_oir2(analysis).unwrap();
        let result = module.functions.get(module.entry.unwrap()).unwrap().signature.results[0];
        let summary = module.summaries.alloc(ir::Summary {
            entry_kind: ir::EntryKind::ObservationAdmissionStaysCaller,
            parameters: vec![],
            results: vec![ir::ResultRelation {
                fresh: vec![], aliases: vec![], external_deps: vec![],
                provenance: ir::AdmissionProvenance::UnboundResult,
            }],
            contract: ir::CallContract {
                results: vec![result], failures: vec![], suspends: false, async_destroy: false,
                publication: ir::Publication::Atomic, protocol: None,
            },
        }).unwrap();
        module.functions.get_mut(module.entry.unwrap()).unwrap().summary = Some(summary);
        let report = ir::verify::verify_high(&module).expect_err("source actions keep their call boundary");
        assert!(report.diagnostics.iter().any(|diagnostic| diagnostic.rule == ir::verify::RuleId::B51
            && diagnostic.message.contains("entry kind differs")), "{report:?}");
    });
    let mut module = lower_std(include_str!("fixtures/std-log.ok")).unwrap();
    assert!(ir::verify::verify_high(&module).is_err(), "external calls require the trusted catalog");
    let function = module.functions.iter().find_map(|(id, function)| {
        (function.symbol == "okkama.atomic.Atomic.value#string").then_some(id)
    }).unwrap();
    module.functions.get_mut(function).unwrap().signature.results.clear();
    assert!(super::native_summaries::P1NativeSummaries::for_module(&module).is_err());
}
