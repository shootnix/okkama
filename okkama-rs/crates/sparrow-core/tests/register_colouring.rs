//! #540 baseline evidence. Large diagnostics are opt-in, Docker-only.
use okkama_compiler::{SourceFile, codegen, ir, lowering::lower_program, syntax};
use ir::{Block, BlockId, Constant, Function, Inst, InstKind as K, IntBinaryOp as B,
    IntUnaryOp, Target, TermKind as T, Terminator, Type, ValueId as V};

fn module(source: &str) -> ir::Module {
    let source = SourceFile::new("colouring.ok", source).unwrap();
    let ast = syntax::parse(&source, &syntax::ParseOptions::default()).unwrap();
    lower_program(&ast).unwrap()
}

fn template() -> ir::Module {
    let mut out = module("module app; main() is int { 0; }");
    out.functions[0].values.clear();
    out.functions[0].blocks.clear();
    out
}

fn value(f: &mut Function, ty: Type) -> V {
    let id = V(f.values.len());
    f.values.push(ty);
    id
}

fn block(f: &Function, params: Vec<V>, insts: Vec<Inst>, term: T) -> Block {
    Block { params, unwind: None, insts, term: Terminator { kind: term, span: f.span } }
}

fn inst(f: &Function, result: V, kind: K) -> Inst {
    Inst { result: Some(result), kind, span: f.span }
}

fn constant(f: &Function, result: V, n: usize) -> Inst {
    inst(f, result, K::Const(Constant::Int(ir::IntValue::from_i64(n as i64))))
}

/// Exactly n ordinary instructions, excluding terminators. Construction and
/// the independent oracle stay outside the codegen measurement.
fn generated(shape: &str, n: usize) -> (ir::Module, i64) {
    assert!((3..=100_000).contains(&n));
    let mut out = template();
    let f = &mut out.functions[0];
    match shape {
        "sparse" => {
            let mut instructions = Vec::with_capacity(n);
            let first = value(f, Type::Int);
            instructions.push(constant(f, first, 1));
            let mut last = first;
            for _ in 1..n {
                let result = value(f, Type::Int);
                instructions.push(inst(f, result, K::IntUnary { op: IntUnaryOp::Neg, operand: last }));
                last = result;
            }
            f.blocks.push(block(f, vec![], instructions, T::Ret(Some(last))));
        }
        "dense" => {
            let mut instructions = Vec::with_capacity(n);
            let mut items = Vec::with_capacity(n - 2);
            for i in 0..n - 2 {
                let result = value(f, Type::Int);
                items.push(result);
                instructions.push(constant(f, result, i));
            }
            let array = value(f, Type::Array(Box::new(Type::Int)));
            instructions.push(inst(f, array, K::ArrayNew(items)));
            let len = value(f, Type::Int);
            instructions.push(inst(f, len, K::ArrayLen(array)));
            f.blocks.push(block(f, vec![], instructions, T::Ret(Some(len))));
        }
        "blocks" => {
            for i in 0..n {
                let params = if i == 0 { vec![] } else { vec![value(f, Type::Int)] };
                let result = value(f, Type::Int);
                let instruction = if i == 0 { constant(f, result, 1) } else {
                    inst(f, result, K::IntUnary { op: IntUnaryOp::Neg, operand: params[0] })
                };
                let term = if i + 1 == n { T::Ret(Some(result)) } else {
                    T::Br(Target { block: BlockId(i + 1), args: vec![result] })
                };
                f.blocks.push(block(f, params, vec![instruction], term));
            }
        }
        _ => panic!("unknown shape: {shape}"),
    }
    let expected = if shape == "dense" { (n - 2) as i64 } else if n % 2 == 1 { 1 } else { -1 };
    (out, expected)
}

fn encoded(module: &ir::Module) -> Vec<u8> {
    let saved = module.clone();
    let program = codegen::sparrow::generate(module).unwrap();
    assert_eq!(*module, saved, "codegen must not mutate shared input");
    let bytes = okkama_sparrow::encode(&program).unwrap();
    let decoded = okkama_sparrow::decode(&bytes).unwrap();
    decoded.verify().unwrap();
    assert_eq!(bytes, okkama_sparrow::encode(&decoded).unwrap());
    assert_eq!(bytes, okkama_sparrow::encode(&codegen::sparrow::generate(module).unwrap()).unwrap());
    bytes
}

#[test]
fn register_colouring_generated_semantics_and_overlap() {
    for shape in ["sparse", "dense", "blocks"] {
        let (module, expected) = generated(shape, 16);
        let bytes = encoded(&module);
        let program = okkama_sparrow::decode(&bytes).unwrap();
        // A destination cannot overwrite its last-use operand.
        for instruction in &program.functions[0].code {
            if let okkama_sparrow::Instruction::IntNeg { dst, operand } = instruction {
                assert_ne!(dst.0, *operand);
            }
        }
        assert_eq!(sparrow_core::run(bytes).unwrap(), expected, "{shape}");
    }
}

#[test]
fn register_colouring_loop_parallel_swap() {
    let mut module = template();
    let f = &mut module.functions[0];
    let ids: Vec<V> = (0..12).map(|_| value(f, Type::Int)).collect();
    let [a, b, n, zero, one, p, q, remaining, next, x, y, result] = ids[..] else { unreachable!() };
    let condition = value(f, Type::Bool);
    f.blocks = vec![
        block(f, vec![], vec![constant(f, a, 7), constant(f, b, 11), constant(f, n, 1),
            constant(f, zero, 0), constant(f, one, 1)],
            T::Br(Target { block: BlockId(1), args: vec![a, b, n] })),
        block(f, vec![p, q, remaining], vec![inst(f, condition,
            K::IntCompare { op: ir::CompareOp::Eq, left: remaining, right: zero })],
            T::Cond { condition, then_target: Target { block: BlockId(3), args: vec![p, q] },
                else_target: Target::new(BlockId(2)) }),
        block(f, vec![], vec![inst(f, next, K::IntBinary { op: B::Sub, left: remaining, right: one })],
            T::Br(Target { block: BlockId(1), args: vec![q, p, next] })),
        block(f, vec![x, y], vec![inst(f, result, K::IntBinary { op: B::Sub, left: x, right: y })],
            T::Ret(Some(result))),
    ];
    let bytes = encoded(&module);
    let decoded = okkama_sparrow::decode(&bytes).unwrap();
    assert!(decoded.functions[0].code.iter().any(|inst| {
        let okkama_sparrow::Instruction::Jump { target } = inst else { return false };
        target.moves.iter().any(|(dst, src)| dst != src && target.moves.contains(&(*src, *dst)))
    }), "fixture must exercise an actual parallel-move cycle");
    assert_eq!(sparrow_core::run(bytes).unwrap(), 4);
}

#[test]
fn register_colouring_joins_types_parameters_and_unwind() {
    let module = module("module app;
        enum Boom is Error = (boom(code is int));
        object Work {
            Work();
            fail(n is int) is int { if n > 2 { throw Boom.boom(n); } n; }
            guarded(x is int, y is int) is int {
                let kept = x * 7;
                let result = try { self.fail(y) * 2; } catch Boom.boom(code) { kept + code; };
                result + kept;
            }
            pick(flag is bool, x is int, y is int) is int {
                let p = if flag { x; } else { y; };
                let q = if flag { y; } else { x; }; p - q;
            }
        }
        main() is int { let w = Work(); w.guarded(1, 5) + w.guarded(1, 2) + w.pick(true, 3, 4); }");
    // 19 + 11 - 1: independent oracle, successful and failure paths.
    assert_eq!(sparrow_core::run(encoded(&module)).unwrap(), 29);
}

#[test]
fn register_colouring_bigint_live_across_call() {
    let module = module("module app;
        object Work {
            Work();
            touch() is int { 1; }
            hold(flag is bool, x is int) is int {
                let kept = if flag { x * x; } else { x; };
                let ping = self.touch();
                kept - kept + ping + 41;
            }
        }
        main() is int { let w = Work(); w.hold(true, 4294967296) + w.hold(false, 1); }");
    // A dynamically computed 2^64 is a real pointer, live across a collecting
    // call in GC-stress mode. The other branch carries a small integer.
    let bytes = encoded(&module);
    let decoded = okkama_sparrow::decode(&bytes).unwrap();
    let hold = decoded.functions.iter().find(|f| f.symbol == "Work.hold").unwrap();
    assert!(hold.code.iter().any(|i| matches!(i, okkama_sparrow::Instruction::IntMul { .. })));
    let call = hold.code.iter().position(|i| matches!(i, okkama_sparrow::Instruction::Call { .. })).unwrap();
    assert!(hold.code[call + 1..].iter().any(|i| matches!(i, okkama_sparrow::Instruction::IntSub { .. })));
    assert_eq!(sparrow_core::run(bytes).unwrap(), 84);
}

#[test]
fn register_colouring_empty_and_no_value() {
    for source in ["module library;", "module app; object Quiet { Quiet(); noop() is void {} } main() is int { Quiet().noop(); 42; }"] {
        let module = module(source);
        let bytes = encoded(&module);
        if module.entry.is_some() {
            assert_eq!(sparrow_core::run(bytes).unwrap(), 42);
        }
    }
}

#[test]
fn register_colouring_zero_values_and_bodyless_function() {
    let mut module = template();
    module.entry = None;
    let f = &mut module.functions[0];
    f.attrs.clear();
    f.symbol = "empty".into();
    f.result = ir::ResultType::Void;
    f.blocks.push(block(f, vec![], vec![], T::Ret(None)));
    encoded(&module);
    let f = &mut module.functions[0];
    f.blocks.clear();
    f.attrs.push(ir::Attr::Extern("fixture.empty".into()));
    let param = value(f, Type::Int);
    f.params.push(param);
    encoded(&module);
}

#[test]
#[ignore = "Docker-only dense EDGE_BUDGET fallback for #540"]
fn register_colouring_dense_budget_fallback() {
    let (module, _) = generated("dense", 3_000);
    let program = codegen::sparrow::generate(&module).unwrap();
    // This many simultaneous operands exceed the existing 2^22 pair budget.
    assert_eq!(program.functions[0].registers.len(), module.functions[0].values.len());
    program.verify().unwrap();
}

/// One bounded case per process; does not execute SPB or invoke the JIT.
/// Docker owns wall/CPU/output/memory/PID limits and peak RSS measurement.
#[test]
#[ignore = "Docker-only compile-time evidence for #540"]
fn register_colouring_bounded_diagnostic() {
    let n: usize = std::env::var("OKKAMA_COLOUR_N").expect("explicit N").parse().unwrap();
    assert!([1_000, 10_000, 100_000].contains(&n));
    let shape = std::env::var("OKKAMA_COLOUR_SHAPE").expect("explicit sparse/dense/blocks");
    if cfg!(debug_assertions) {
        panic!("large diagnostic requires a release build");
    }
    let (module, expected) = generated(&shape, n);
    let start = std::time::Instant::now();
    let program = codegen::sparrow::generate(&module).unwrap();
    let elapsed = start.elapsed();
    let f = &program.functions[0];
    eprintln!("colour-case shape={shape} n={n} expected={expected} codegen_ns={} registers={} instructions={}",
        elapsed.as_nanos(), f.registers.len(), f.code.len());
    program.verify().unwrap();
}
