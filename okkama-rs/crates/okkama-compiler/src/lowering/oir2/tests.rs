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
        let text = ir::text::print(&module).expect("printable high");
        let parsed = ir::text::parse(&text).expect("high round trip parses");
        assert!(ir::text::normalized_eq(&module, &parsed).expect("normalized comparison"));
        assert_eq!(ir::text::print(&parsed).unwrap(), text);
        Ok(text)
    })
}

fn golden(source: &str, expected: &str) {
    let actual = print_oir2(source).unwrap_or_else(|error| panic!("{error}"));
    if std::env::var_os("OKKAMA_OIR2_UPDATE_GOLDENS").is_some() {
        for name in ["allowlist", "declarations", "infinite"] {
            let path = format!("{}/src/lowering/oir2/goldens/{name}.oir", env!("CARGO_MANIFEST_DIR"));
            if expected == std::fs::read_to_string(&path).expect("existing golden") {
                std::fs::write(path, &actual).expect("regenerated golden");
                return;
            }
        }
    }
    let expected = ir::text::parse(expected).expect("generated golden parses");
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
        ("defer 0; 0;", "statement"),
        ("try { 1; } catch e is Error { 2; }", "try"),
        ("panic \"stop\";", "panic"),
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
        let expected = include_str!("goldens/rebind.oir");
        let expected = ir::text::parse(expected).unwrap();
        assert_eq!(text, ir::text::print(&expected).unwrap());
        assert!(ir::text::normalized_eq(&module, &ir::text::parse(&text).unwrap()).unwrap());
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
        ("module app; object O { O() { defer 0; } } main() is int { 0; }", "app.O.new#0", "statement"),
        ("module app; object O { O(); destroy { panic \"stop\"; } } main() is int { 0; }",
            "app.O.destroy", "destroy"),
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
fn oir2_self_check_rejects_missing_or_conflicting_function_forms() {
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
        assert!(super::structure::check(&broken).is_err());
    }
    for attr in [
        ir::FunctionAttr::Extern("ffi".into()),
        ir::FunctionAttr::Intrinsic { name: "intrinsic".into(), objects: vec![], helpers: vec![] },
    ] {
        let mut external = module.clone();
        external.functions.get_mut(id).unwrap().attrs.push(attr);
        assert!(super::structure::check(&external).is_err(), "body and ABI conflict");
        external.functions.get_mut(id).unwrap().body = None;
        super::structure::check(&external).unwrap();
    }
}

#[test]
fn oir2_self_check_rejects_nonempty_unreachable_blocks() {
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
        let error = super::structure::check(&broken).expect_err("nonempty tombstone");
        assert!(error.to_string().contains("empty tombstone"), "{error}");
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
    if std::env::var_os("OKKAMA_OIR2_UPDATE_GOLDENS").is_some() {
        let path = format!("{}/src/lowering/oir2/goldens/{name}.oir", env!("CARGO_MANIFEST_DIR"));
        std::fs::write(path, actual).unwrap();
    } else {
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
        ("module app; object O { O() { try { 1; } catch e is Error { 2; } } } main() is int { 0; }",
            "app.O.new#0"),
        ("module app; object O { O(); f() is int { defer 0; 0; } } main() is int { 0; }", "app.O.f#0"),
        ("module app; object O { has n is int = 1 & 2; O(); } main() is int { 0; }",
            "app.O.default#0"),
        ("module app; object O { O(); destroy { 0; } } main() is int { 0; }", "app.O.destroy"),
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
