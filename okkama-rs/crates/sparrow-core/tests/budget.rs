//! Execution budgets (H-04, #21): runaway programs stop with a report of what
//! ran out, and a later run in the same process starts from the base memory.
//! One test: the allocation counter is process-wide.
use okkama_sparrow::{Constant, Dst, FuncRef, Function, Instruction as I, Program, Register, Target, Type, TypeRef};
use sparrow_core::budget::{Counted, Limits, Meter, counted_bytes};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[global_allocator]
static ALLOCATOR: Counted<std::alloc::System> = Counted(std::alloc::System);

/// `main` over registers of `types` (register i has type i): `code` after
/// the constants `init`, looping back to its first instruction.
fn looping(types: Vec<Type>, init: Vec<(u32, Constant)>, body: Vec<I>) -> Vec<u8> {
    let registers = (0..types.len() as u32).map(TypeRef).collect();
    let start = init.len() as u32;
    let mut code: Vec<I> = init.into_iter().map(|(r, value)| I::Const { dst: Dst(Register(r)), value }).collect();
    code.extend(body);
    let int = types.iter().position(|t| *t == Type::Int).expect("int register") as u32;
    code.push(I::Jump { target: Target { pc: start, moves: Vec::new() } });
    code.push(I::Return { value: Some(Register(int)) });
    okkama_sparrow::encode(&Program {
        module: "app".into(),
        version: "1.0.0".into(),
        types,
        functions: vec![Function {
            symbol: "main".into(),
            is_async: false,
            native: None,
            intrinsic: None,
            registers,
            params: Vec::new(),
            result: Some(TypeRef(int)),
            code,
            handlers: Vec::new(),
        }],
        entry: Some(FuncRef(0)),
        ..Program::default()
    })
    .unwrap()
}

fn spin() -> Vec<u8> {
    looping(vec![Type::Int], vec![(0, Constant::Int("0".into()))], Vec::new())
}

/// n = n * n forever: BigInt growth.
fn squares() -> Vec<u8> {
    let r = Register(0);
    looping(vec![Type::Int], vec![(0, Constant::Int("3".into()))], vec![I::IntMul { dst: Dst(r), left: r, right: r }])
}

/// s = s + s forever: string growth.
fn doubles() -> Vec<u8> {
    let s = Register(1);
    looping(
        vec![Type::Int, Type::Str],
        vec![(0, Constant::Int("0".into())), (1, Constant::Str("x".into()))],
        vec![I::StrConcat { dst: Dst(s), left: s, right: s }],
    )
}

fn run(bytes: Vec<u8>, limits: Limits) -> String {
    match sparrow_core::run_limited(bytes, Vec::new(), false, Vec::new(), limits) {
        Ok(code) => format!("exit {code}"),
        Err(error) => error.0,
    }
}

#[test]
fn runaway_programs_stop_within_their_budgets() {
    sparrow_core::budget::set_memory_probe(counted_bytes);
    let steps = Limits { steps: Some(1_000_000), ..Limits::default() };
    assert_eq!(run(spin(), steps), "Execution budget exhausted: steps");

    let flag = Arc::new(AtomicBool::new(false));
    let setter = flag.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(50));
        setter.store(true, Ordering::Relaxed);
    });
    let interrupt = Limits { interrupt: Some(flag), ..Limits::default() };
    assert_eq!(run(spin(), interrupt), "Execution interrupted");

    let memory = || Limits { memory: Some(8 << 20), ..Limits::default() };
    assert_eq!(run(squares(), memory()), "Execution budget exhausted: memory");
    assert_eq!(run(doubles(), memory()), "Execution budget exhausted: memory");

    // Each exhausted run releases what it held: the process stays at its base.
    let meter = Meter::start(counted_bytes);
    for _ in 0..3 {
        assert_eq!(run(doubles(), memory()), "Execution budget exhausted: memory");
        assert_eq!(run(squares(), memory()), "Execution budget exhausted: memory");
        assert!(meter.used() < 2 << 20, "held after exhausted runs: {} bytes", meter.used());
    }
}
