//! Generated-code evidence for #542. Inspect CLIF before optimization, and
//! execute the same verified SPB with both dispatch policies, without changing
//! process-wide environment variables in parallel tests.
use super::*;
use cranelift_codegen::ir::{ExternalName, InstructionData, Opcode};

const PAIR: &str = r#"
sparrow 15
module "ready-callees" "1.0.0"
type t0 int
function f0 "identity" {
 registers [t0]
 params [r0]
 result t0
 0: ret r0
}
function f1 "caller" {
 registers [t0, t0]
 params [r0]
 result t0
 0: call r1, f0, [r0]
 1: ret r1
}
"#;

fn runtime(text: &str, old: bool) -> Box<Runtime> {
    let program = okkama_sparrow::assemble(text).unwrap();
    let mut runtime = Box::new(Runtime::new(crate::program::Loaded::new(program)).unwrap());
    runtime.jit.old_dispatch = Chosen { all: old, ..Chosen::default() };
    runtime
}

fn helper_calls(runtime: &Runtime, caller: usize, helper: FuncId) -> usize {
    let func = &runtime.jit.emitted[&caller];
    func.layout.blocks().flat_map(|block| func.layout.block_insts(block)).filter(|&inst| {
        let InstructionData::Call { func_ref, .. } = func.dfg.insts[inst] else { return false };
        let ExternalName::User(name) = func.dfg.ext_funcs[func_ref].name else { return false };
        func.params.user_named_funcs()[name].index == helper.as_u32()
    }).count()
}

fn compiles(runtime: &Runtime, caller: usize) -> usize {
    helper_calls(runtime, caller, runtime.jit.helpers.compile)
}

fn opcodes(runtime: &Runtime, caller: usize, opcode: Opcode) -> usize {
    let func = &runtime.jit.emitted[&caller];
    func.layout.blocks().flat_map(|block| func.layout.block_insts(block))
        .filter(|&inst| func.dfg.insts[inst].opcode() == opcode).count()
}

#[test]
fn ready_static_target_elides_dispatch_but_keeps_the_call_and_status_check() {
    let mut fast = runtime(PAIR, false);
    let target = compile(&mut fast, 0).unwrap();
    compile(&mut fast, 1).unwrap();
    assert_eq!(compiles(&fast, 1), 0);
    assert_eq!(opcodes(&fast, 1, Opcode::CallIndirect), 1);
    // The embedded value is the published code address, not a function index.
    let func = &fast.jit.emitted[&1];
    assert!(func.layout.blocks().flat_map(|block| func.layout.block_insts(block)).any(|inst| {
        matches!(func.dfg.insts[inst], InstructionData::UnaryImm { opcode: Opcode::Iconst, imm }
            if i64::from(imm) == target as i64)
    }));
    let mut old = runtime(PAIR, true);
    compile(&mut old, 0).unwrap();
    compile(&mut old, 1).unwrap();
    assert_eq!(compiles(&old, 1), 1);
    // Table/status loads and frontend root reloads belong to the fallback;
    // count branches exactly, but leave reload placement to Cranelift.
    assert!(opcodes(&old, 1, Opcode::Load) >= opcodes(&fast, 1, Opcode::Load) + 2);
    assert_eq!(opcodes(&old, 1, Opcode::Brif), opcodes(&fast, 1, Opcode::Brif) + 3);
    assert!(opcodes(&fast, 1, Opcode::Brif) > 0);
    assert_eq!(fast.run_function_with(1, &[word::small(42).unwrap()], 2 << 20).unwrap(), word::small(42).unwrap());
}

#[test]
fn unresolved_and_self_targets_stay_lazy_even_after_execution() {
    let mut cold = runtime(PAIR, false);
    let caller = compile(&mut cold, 1).unwrap();
    assert_eq!(cold.code[0], 0, "emitting a caller must not compile its callee");
    assert_eq!(compiles(&cold, 1), 1);
    for n in [17, 31] {
        assert_eq!(cold.run_function_with(1, &[word::small(n).unwrap()], 2 << 20).unwrap(), word::small(n).unwrap());
    }
    assert_eq!(compile(&mut cold, 1).unwrap(), caller, "no warm recompilation");
    assert_eq!(compiles(&cold, 1), 1);
    let text = PAIR.replace("call r1, f0", "call r1, f1");
    let mut recursive = runtime(&text, false);
    compile(&mut recursive, 1).unwrap();
    assert_eq!(compiles(&recursive, 1), 1);
    assert_eq!(recursive.code[0], 0);
}

#[test]
fn mutual_recursion_keeps_the_unresolved_edge_and_can_embed_the_ready_edge() {
    let mut program = okkama_sparrow::assemble(PAIR).unwrap();
    program.functions[0] = program.functions[1].clone();
    program.functions[0].symbol = "other".into();
    if let I::Call { function, .. } = &mut program.functions[0].code[0] {
        *function = okkama_sparrow::FuncRef(1);
    }
    program.verify().unwrap();
    let mut runtime = Box::new(Runtime::new(crate::program::Loaded::new(program)).unwrap());
    runtime.jit.old_dispatch = Chosen::default();
    let first = compile(&mut runtime, 0).unwrap();
    assert_eq!(runtime.code[1], 0);
    assert_eq!(compiles(&runtime, 0), 1);
    compile(&mut runtime, 1).unwrap();
    assert_eq!(compiles(&runtime, 1), 0);
    assert_eq!(runtime.code[0], first);
}

#[test]
fn forced_plain_and_unoptimized_callers_keep_reference_dispatch() {
    for plain in [false, true] {
        let mut runtime = runtime(PAIR, false);
        if plain {
            runtime.jit.plain = Chosen { all: true, ..Chosen::default() };
        } else {
            runtime.jit.unoptimized = Chosen { all: true, ..Chosen::default() };
            runtime.jit.unoptimized_isa = Some(target("none").unwrap());
        }
        compile(&mut runtime, 0).unwrap();
        compile(&mut runtime, 1).unwrap();
        assert_eq!(compiles(&runtime, 1), 1);
    }
}

#[test]
fn role_and_closure_targets_remain_dynamic_when_all_entries_are_ready() {
    for (text, caller, ready, closure) in [
        (include_str!("../../../../../docs/SparrowBook/examples/shapes.spa"), 2, vec![0, 1], false),
        (include_str!("../../../../../docs/SparrowBook/examples/closures.spa"), 1, vec![0], true),
    ] {
        let mut runtime = runtime(text, false);
        for function in ready { compile(&mut runtime, function).unwrap(); }
        compile(&mut runtime, caller).unwrap();
        if closure {
            assert_eq!(helper_calls(&runtime, caller, runtime.jit.helpers.call_closure), 1);
        } else {
            assert_eq!(compiles(&runtime, caller), 2);
        }
        let result = runtime.run_function(caller, 2 << 20).unwrap();
        assert_eq!(word::small_value(result), Some(if closure { 42 } else { 17 }));
    }
}

#[test]
fn runtimes_keep_their_own_addresses_and_survive_other_module_teardown() {
    for old in [false, true] {
        let mut outer = runtime(PAIR, old);
        let first = compile(&mut outer, 0).unwrap();
        compile(&mut outer, 1).unwrap();
        {
            let mut inner = runtime(PAIR, old);
            let second = compile(&mut inner, 0).unwrap();
            compile(&mut inner, 1).unwrap();
            assert_ne!(first, second);
            assert_eq!(inner.run_function_with(1, &[word::small(31).unwrap()], 2 << 20).unwrap(), word::small(31).unwrap());
        }
        // Move the Box, retain the outer module, then invoke its embedded entry.
        let mut moved = outer;
        assert_eq!(moved.run_function_with(1, &[word::small(17).unwrap()], 2 << 20).unwrap(), word::small(17).unwrap());
        assert_eq!(compile(&mut moved, 0).unwrap(), first);
    }
}

#[test]
fn ordinary_signatures_keep_register_and_stack_arguments() {
    for old in [false, true] {
        for arity in [0, 1, 2, 8, 9, 16] {
            let params = (0..arity).map(|i| format!("r{i}")).collect::<Vec<_>>().join(", ");
            let registers = vec!["t0"; arity.max(1)].join(", ");
            let body = if arity == 0 { "0: const r0, int 42\n 1: ret r0".into() }
                else { format!("0: ret r{}", arity - 1) };
            let mut init = (0..arity).map(|i| format!(" {i}: const r{i}, int {}\n", 43 - arity + i)).collect::<String>();
            init.push_str(&format!(" {arity}: call r{arity}, f0, [{params}]\n {}: ret r{arity}", arity + 1));
            let text = format!(r#"
sparrow 15
module "arity" "1.0.0"
type t0 int
function f0 "callee" {{
 registers [{registers}]
 params [{params}]
 result t0
 {body}
}}
function f1 "caller" {{
 registers [{}]
 params []
 result t0
 {init}
}}
"#, vec!["t0"; arity + 1].join(", "));
            let mut runtime = runtime(&text, old);
            compile(&mut runtime, 0).unwrap();
            assert_eq!(runtime.run_function(1, 2 << 20).unwrap(), word::small(42).unwrap());
            assert_eq!(compiles(&runtime, 1), usize::from(old));
        }
    }
}

#[test]
fn native_trampolines_are_not_used_as_ready_static_ordinary_entries() {
    let text = r#"
sparrow 15
module "native" "1.0.0"
type t0 int
function f0 "clock" native "runtime/time@1::wall_clock/0" {
 registers []
 params []
 result t0
}
function f1 "caller" {
 registers [t0]
 params []
 result t0
 0: call r0, f0, []
 1: ret r0
}
"#;
    for old in [false, true] {
        let mut runtime = runtime(text, old);
        compile(&mut runtime, 0).unwrap();
        compile(&mut runtime, 1).unwrap();
        assert_eq!(helper_calls(&runtime, 1, runtime.jit.helpers.native), 1);
        assert_eq!(opcodes(&runtime, 1, Opcode::CallIndirect), 0);
        assert_eq!(compiles(&runtime, 1), 0);
        assert!(runtime.run_function(1, 2 << 20).is_ok());
    }
}

#[test]
fn ready_async_body_calls_use_the_same_explicit_roots_at_the_final_call() {
    let text = include_str!("../../tests/ready_callees.spa");
    for old in [false, true] {
        for frontend in [false, true] {
            let mut program = okkama_sparrow::assemble(text).unwrap();
            program.functions[1].is_async = true;
            program.verify().unwrap();
            let mut runtime = Box::new(Runtime::new(crate::program::Loaded::new(program)).unwrap());
            runtime.jit.old_dispatch = Chosen { all: old, ..Chosen::default() };
            runtime.jit.frontend_maps = Chosen { all: frontend, ..Chosen::default() };
            runtime.heap.set_stress(true);
            compile(&mut runtime, 0).unwrap();
            let big = runtime.heap.big("123456789012345678901234567890".parse().unwrap());
            let text = runtime.heap.allocate_text("heap");
            let label = runtime.heap.allocate_text("object");
            let object = runtime.heap.object_new(0, &[label]);
            let result = runtime.run_function_with(1, &[big, text, object], 2 << 20).unwrap();
            let number = runtime.heap.field(result, 0);
            assert_eq!(runtime.heap.integer(number).to_string(), "246913578024691357802469135780");
            assert_eq!(runtime.heap.text(runtime.heap.field(result, 1)), "heapheapobjectheap");
            assert_eq!(compiles(&runtime, 1), usize::from(old));
        }
    }
}

#[test]
fn failures_and_trace_sites_match_reference_dispatch() {
    let text = include_str!("../../tests/ready_callees.spa");
    let mut reports = Vec::new();
    for old in [false, true] {
        let mut runtime = runtime(text, old);
        runtime.heap.set_stress(true);
        compile(&mut runtime, 2).unwrap();
        let error = runtime.run_function(4, 2 << 20).unwrap_err();
        reports.push((error.0, runtime.uncaught.take().unwrap()));
    }
    assert_eq!(reports[0], reports[1]);
    assert_eq!(reports[0].1.trace.len(), 2);
}

#[test]
fn lazy_compilation_failure_propagates_and_never_publishes_a_zero_entry() {
    for old in [false, true] {
        let mut runtime = runtime(PAIR, old);
        compile(&mut runtime, 1).unwrap();
        runtime.limits.memory = Some(1);
        runtime.limits.memory_scope = crate::budget::MemoryScope::Runtime;
        let error = runtime.run_function_with(1, &[word::small(17).unwrap()], 2 << 20).unwrap_err();
        assert!(error.0.starts_with("identity: compiling it would take more than 0 MiB"), "{}", error.0);
        assert_eq!(runtime.code[0], 0);
        assert_eq!(compiles(&runtime, 1), 1);
        runtime.limits.memory = None;
        assert_eq!(runtime.run_function_with(1, &[word::small(31).unwrap()], 2 << 20).unwrap(), word::small(31).unwrap());
        assert_ne!(runtime.code[0], 0);
    }
}

#[test]
fn bounded_self_and_mutual_recursion_keep_stack_and_step_checks() {
    let recursive = r#"
sparrow 15
module "recursion" "1.0.0"
type t0 int
type t1 bool
function f0 "recurse" {
 registers [t0, t0, t1, t0, t0]
 params [r0]
 result t0
 0: const r1, int 0
 1: int.cmp eq, r2, r0, r1
 2: br r2, @3, @4
 3: ret r1
 4: const r1, int 1
 5: int.sub r3, r0, r1
 6: call r4, f0, [r3]
 7: int.add r4, r4, r1
 8: ret r4
}
function f1 "caller" {
 registers [t0, t0]
 params [r0]
 result t0
 0: call r1, f0, [r0]
 1: ret r1
}
"#;
    for old in [false, true] {
        for mutual in [false, true] {
            let mut program = okkama_sparrow::assemble(recursive).unwrap();
            if mutual {
                let mut other = program.functions[0].clone();
                other.symbol = "other".into();
                program.functions.push(other);
                if let I::Call { function, .. } = &mut program.functions[0].code[6] {
                    *function = okkama_sparrow::FuncRef(2);
                }
            }
            program.verify().unwrap();
            let mut runtime = Box::new(Runtime::new(crate::program::Loaded::new(program)).unwrap());
            runtime.jit.old_dispatch = Chosen { all: old, ..Chosen::default() };
            runtime.limits.steps = Some(100);
            compile(&mut runtime, 0).unwrap();
            for n in [0, 7, 7] {
                assert_eq!(runtime.run_function_with(1, &[word::small(n).unwrap()], 2 << 20).unwrap(), word::small(n).unwrap());
            }
            let error = runtime.run_function_with(1, &[word::small(7).unwrap()], runtime::STACK_HEADROOM + 128).unwrap_err();
            assert!(error.0.contains("Stack overflow"), "{}", error.0);
            runtime.limits.steps = Some(2);
            let error = runtime.run_function_with(1, &[word::small(7).unwrap()], 2 << 20).unwrap_err();
            assert_eq!(error.0, "Execution budget exhausted: steps");
        }
    }
}

#[test]
fn ready_async_entries_still_construct_cold_futures() {
    let text = r#"
sparrow 15
module "async-entry" "1.0.0"
type t0 int
type t1 future t0
function f0 "async" async {
 registers [t0]
 params []
 result t0
 0: const r0, int 42
 1: ret r0
}
function f1 "caller" {
 registers [t1]
 params []
 result t1
 0: call r0, f0, []
 1: ret r0
}
"#;
    let mut runtime = runtime(text, false);
    compile(&mut runtime, 0).unwrap();
    compile(&mut runtime, 1).unwrap();
    assert_eq!(opcodes(&runtime, 1, Opcode::CallIndirect), 0);
    assert_eq!(helper_calls(&runtime, 1, runtime.jit.helpers.future_new), 1);
    assert_eq!(compiles(&runtime, 1), 0);
}

#[test]
fn cap_fields_prove_the_target_and_use_the_ordinary_receiver_abi() {
    let text = r#"
sparrow 15
module "cap-entry" "1.0.0"
type t0 int
type t1 object obj0
type t2 cap cap0
object obj0 "Owner" { field "value" t0 }
cap cap0 "Value" obj0 { field "value" f0 }
function f0 "get" {
 registers [t1, t0]
 params [r0]
 result t0
 0: obj.get r1, r0, 0
 1: ret r1
}

function f1 "caller" {
 registers [t0, t1, t2, t0]
 params []
 result t0
 0: const r0, int 42
 1: obj.new r1, obj0, [r0]
 2: move r2, r1
 3: call.cap r3, r2, cap0, 0
 4: ret r3
}
"#;
    for old in [false, true] {
        let mut runtime = runtime(text, old);
        compile(&mut runtime, 0).unwrap();
        assert_eq!(runtime.run_function(1, 2 << 20).unwrap(), word::small(42).unwrap());
        assert_eq!(compiles(&runtime, 1), usize::from(old));
    }
}

#[test]
fn mixed_closure_captures_keep_the_existing_bridge_and_roots() {
    let text = r#"
sparrow 15
module "captures" "1.0.0"
type t0 int
type t1 str
type t2 object obj0
type t3 fn [t1] -> t2
object obj0 "Result" {
 field "number" t0
 field "text" t1
}
function f0 "closure-body" {
 registers [t0, t1, t1, t1, t2]
 params [r0, r1, r2]
 result t2
 0: str.concat r3, r1, r2
 1: obj.new r4, obj0, [r0, r3]
 2: ret r4
}
function f1 "caller" {
 registers [t0, t1, t1, t3, t2]
 params [r0, r1, r2]
 result t2
 0: fn.closure r3, f0, [r0, r1]
 1: call.fn r4, r3, [r2]
 2: ret r4
}
"#;
    for old in [false, true] {
        for ready in [false, true] {
            let mut runtime = runtime(text, old);
            runtime.heap.set_stress(true);
            if ready { compile(&mut runtime, 0).unwrap(); }
            let big = runtime.heap.big("123456789012345678901234567890".parse().unwrap());
            let capture = runtime.heap.allocate_text("capture");
            let argument = runtime.heap.allocate_text("argument");
            let result = runtime.run_function_with(1, &[big, capture, argument], 2 << 20).unwrap();
            assert_eq!(runtime.heap.integer(runtime.heap.field(result, 0)).to_string(), "123456789012345678901234567890");
            assert_eq!(runtime.heap.text(runtime.heap.field(result, 1)), "captureargument");
            assert_eq!(helper_calls(&runtime, 1, runtime.jit.helpers.call_closure), 1);
            assert_eq!(opcodes(&runtime, 1, Opcode::CallIndirect), 0);
        }
    }
}

#[test]
fn float_bits_use_the_same_word_entry_abi() {
    let text = PAIR.replace("type t0 int", "type t0 float");
    for old in [false, true] {
        let mut runtime = runtime(&text, old);
        compile(&mut runtime, 0).unwrap();
        for value in [0.0f64, -0.0, 1.25, -12345.5] {
            assert_eq!(runtime.run_function_with(1, &[value.to_bits()], 2 << 20).unwrap(), value.to_bits());
        }
        assert_eq!(compiles(&runtime, 1), usize::from(old));
    }
}