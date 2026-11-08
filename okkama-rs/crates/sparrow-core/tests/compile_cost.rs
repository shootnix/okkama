//! Compile cost (#73): a function with many values live across many fallible
//! instructions is refused before machine code generation takes memory
//! quadratic in their number; ordinary functions compile as before.

use sparrow_core::budget::{Limits, counted_bytes, set_memory_probe};

/// `deep(n)`: k floats computed in a chain, live across a recursive call,
/// summed after it. Every float operation is fallible, so each one emits
/// several Cranelift blocks in which all k values are live.
fn chain(k: usize) -> Vec<u8> {
    let mut registers = vec!["t0", "t0", "t1", "t0", "t0", "t0", "t2", "t2"];
    registers.extend(std::iter::repeat_n("t2", k));
    let mut code = vec![
        "const r1, int 0".to_string(),
        "int.cmp eq, r2, r0, r1".into(),
        "br r2, @3, @4".into(),
        "ret r1".into(),
        "const r3, int 1".into(),
        "const r6, float 0x3ff8000000000000".into(),
        "const r7, float 0x3fd0000000000000".into(),
        "float.add r8, r6, r7".into(),
    ];
    for i in 1..k {
        code.push(format!("float.add r{}, r{}, r7", 8 + i, 7 + i));
    }
    code.push("int.sub r4, r0, r3".into());
    code.push("call r5, f0, [r4]".into());
    for i in 0..k {
        code.push(format!("float.add r6, r6, r{}", 8 + i));
    }
    code.push("ret r5".into());
    let body: String = code.iter().enumerate().map(|(pc, line)| format!(" {pc}: {line}\n")).collect();
    let text = format!(
        "sparrow 12\nmodule \"cost\" \"v1.0.0\"\ntype t0 int\ntype t1 bool\ntype t2 float\n\
         function f0 \"deep\" {{\n registers [{}]\n params [r0]\n result t0\n{body}}}\n\
         function f1 \"main\" {{\n registers [t0, t0]\n params []\n result t0\n 0: const r0, int 2\n 1: call r1, f0, [r0]\n 2: const r0, int 0\n 3: ret r0\n}}\nentry f1\n",
        registers.join(", ")
    );
    okkama_sparrow::encode(&okkama_sparrow::assemble(&text).unwrap()).unwrap()
}

fn run(bytes: Vec<u8>, memory: u64) -> Result<i64, sparrow_core::Error> {
    set_memory_probe(counted_bytes);
    let limits = Limits { memory: Some(memory), ..Limits::default() };
    sparrow_core::run_limited(bytes, Vec::new(), false, Vec::new(), limits)
}

#[test]
fn many_values_live_across_fallible_instructions_are_refused_before_compiling() {
    // About 6 blocks × k² / 2 live registers at 64 bytes: k = 300 needs some
    // 34 MiB, over a 16 MiB budget; k = 100 needs about 4 MiB.
    let error = run(chain(300), 16 << 20).unwrap_err();
    assert!(error.0.starts_with("deep: compiling it would take more than 16 MiB"), "{}", error.0);
    assert_eq!(run(chain(100), 16 << 20).unwrap(), 0);
}

#[test]
fn the_cost_check_skips_small_functions() {
    // Without a budget the limit is 256 MiB; k = 200 is far below it.
    let limits = Limits::default();
    assert_eq!(sparrow_core::run_limited(chain(200), Vec::new(), false, Vec::new(), limits).unwrap(), 0);
}
