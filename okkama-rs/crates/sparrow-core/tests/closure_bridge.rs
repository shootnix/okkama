//! GH-02 Phase A: in-process SPB fixtures, with independent expected results.
//! Run this file again with GC stress and the forced reference bridge.

fn bytes(source: &str) -> Vec<u8> {
    okkama_sparrow::encode(&okkama_sparrow::assemble(source).unwrap()).unwrap()
}

fn run(source: &str) -> Result<i64, sparrow_core::Error> {
    sparrow_core::run(bytes(source))
}

/// Sum 1..=params twice, once via a newly compiled closure and once warm.
fn sum(params: usize, captures: usize) -> String {
    let registers = (0..params).map(|i| format!("r{i}")).collect::<Vec<_>>();
    let types = vec!["t0"; params + 1].join(", ");
    let mut body = vec![format!("const r{params}, int 0")];
    for i in 0..params {
        body.push(format!("int.add r{params}, r{params}, r{i}"));
    }
    body.push(format!("ret r{params}"));
    let mut main = (0..params).map(|i| format!("const r{i}, int {}", i + 1)).collect::<Vec<_>>();
    main.push(format!("fn.closure r{params}, f0, [{}]", registers[..captures].join(", ")));
    main.push(format!("call.fn r{}, r{params}, [{}]", params + 1, registers[captures..].join(", ")));
    main.push(format!("call.fn r{}, r{params}, [{}]", params + 2, registers[captures..].join(", ")));
    main.push(format!("int.add r{}, r{}, r{}", params + 1, params + 1, params + 2));
    main.push(format!("ret r{}", params + 1));
    let numbered = |lines: Vec<String>| lines.iter().enumerate().map(|(pc, line)| format!(" {pc}: {line}\n")).collect::<String>();
    format!(
        "sparrow 16\nmodule \"bridge\" \"1.0.0\"\ntype t0 int\ntype t1 fn [{}] -> t0\n\
         function f0 \"sum\" {{\n registers [{types}]\n params [{}]\n result t0\n{}}}\n\
         function f1 \"main\" {{\n registers [{}]\n params []\n result t0\n{}}}\nentry f1\n",
        vec!["t0"; params - captures].join(", "), registers.join(", "), numbered(body),
        [vec!["t0"; params], vec!["t1", "t0", "t0"]].concat().join(", "), numbered(main)
    )
}

#[test]
fn closure_bridge_zero_one_two_and_eight_words_first_and_warm() {
    for params in [0, 1, 2, 8] {
        for captures in 0..=params {
            assert_eq!(run(&sum(params, captures)).unwrap(), (params * (params + 1)) as i64,
                "params={params}, captures={captures}");
        }
    }
}

#[test]
fn closure_bridge_verified_over_limit_is_refused() {
    // SPB permits this signature; the bridge must fail before copying nine words.
    for captures in [0, 1, 9] {
        let error = run(&sum(9, captures)).unwrap_err();
        assert!(error.0.contains("closure with more than 8 parameters"), "{}", error.0);
    }
}

#[test]
fn closure_bridge_invalid_counts_are_rejected_by_verification() {
    let source = sum(2, 1);
    for (from, to) in [("f0, [r0]", "f0, []"), ("r2, [r1]", "r2, []"), ("r2, [r1]", "r2, [r0, r1]")] {
        assert!(okkama_sparrow::assemble(&source.replace(from, to)).is_err(), "{from} -> {to}");
    }
}

#[test]
fn closure_bridge_mixed_roots_survive_allocating_callee() {
    // Both captured and passed bigint/string values are read AFTER allocations.
    // main does not read them after call.fn: call-site roots must keep inputs.
    let source = r#"sparrow 16
module "mixed" "1.0.0"
type t0 int
type t1 str
type t2 fn [t0, t1] -> t0
function f0 "mixed" {
 registers [t0, t1, t0, t1, t1, t0, t0]
 params [r0, r1, r2, r3]
 result t0
 0: str.concat r4, r1, r3
 1: int.add r5, r0, r2
 2: int.sub r5, r5, r0
 3: int.sub r5, r5, r2
 4: str.len r6, r4
 5: int.add r5, r5, r6
 6: ret r5
}
function f1 "main" {
 registers [t0, t1, t0, t1, t2, t0]
 params []
 result t0
 0: const r0, int 123456789012345678901234567890
 1: int.add r0, r0, r0
 2: const r1, str "captured"
 3: str.concat r1, r1, r1
 4: const r2, int 987654321098765432109876543210
 5: int.add r2, r2, r2
 6: const r3, str "argument"
 7: str.concat r3, r3, r3
 8: fn.closure r4, f0, [r0, r1]
 9: call.fn r5, r4, [r2, r3]
 10: ret r5
}
entry f1
"#;
    assert_eq!(run(source).unwrap(), 32);
}

#[test]
fn closure_bridge_native_and_role_entries_share_publication() {
    let source = r#"sparrow 16
module "entries" "1.0.0"
type t0 int
type t1 str
type t2 fn [t1] -> t0
type t3 object obj0
type t4 role role0
type t5 fn [] -> t0
role role0 "Value" { method "get" [] -> t0 }
object obj0 "Box" { field "value" t0 does role0 [f1] }
function f0 "hex.valid" native "runtime/hex@1::hex_valid_prefix/1" {
 registers [t1]
 params [r0]
 result t0
}
function f1 "Box.get" {
 registers [t3, t0]
 params [r0]
 result t0
 0: obj.get r1, r0, 0
 1: ret r1
}
function f2 "main" {
 registers [t1, t2, t0, t3, t4, t5, t0, t0]
 params []
 result t0
 0: const r0, str "cafe!"
 1: fn.closure r1, f0, []
 2: call.fn r2, r1, [r0]
 3: obj.new r3, obj0, [r2]
 4: role.wrap r4, r3, role0
 5: call.role r6, r4, role0, 0, []
 6: fn.closure r5, f1, [r3]
 7: call.fn r7, r5, []
 8: int.add r6, r6, r7
 9: ret r6
}
entry f2
"#;
    assert_eq!(run(source).unwrap(), 8);
}

#[test]
fn closure_bridge_async_builds_a_cold_future() {
    let source = r#"sparrow 16
module "cold" "1.0.0"
type t0 int
type t1 future t0
type t2 fn [t0] -> t1
function f0 "cold" async {
 registers [t0, t0]
 params [r0]
 result t0
 0: trap confirm, "cold body must not run"
}
function f1 "main" {
 registers [t0, t2, t1]
 params []
 result t0
 0: const r0, int 7
 1: fn.closure r1, f0, []
 2: call.fn r2, r1, [r0]
 3: ret r0
}
entry f1
"#;
    assert_eq!(run(source).unwrap(), 7);
}

#[test]
fn closure_bridge_failure_keeps_the_callee_and_call_site_trace() {
    let source = sum(1, 0).replace("0: const r1, int 0", "0: trap confirm, \"bridge failure\"");
    let (error, uncaught) = sparrow_core::run_reported(bytes(&source), Vec::new(), Vec::new()).unwrap().unwrap_err();
    assert!(error.0.contains("bridge failure"), "{}", error.0);
    let trace = uncaught.unwrap().trace;
    assert_eq!(trace.iter().map(|site| (site.function, site.pc)).collect::<Vec<_>>(), [(0, 0), (1, 2)]);
}

#[test]
fn closure_bridge_one_function_supports_static_and_different_capture_prefixes() {
    let source = r#"sparrow 16
module "prefixes" "1.0.0"
type t0 int
type t1 fn [t0, t0] -> t0
type t2 fn [t0] -> t0
function f0 "add" {
 registers [t0, t0, t0]
 params [r0, r1]
 result t0
 0: int.add r2, r0, r1
 1: ret r2
}
function f1 "main" {
 registers [t0, t0, t1, t2, t0, t0, t0]
 params []
 result t0
 0: const r0, int 40
 1: const r1, int 2
 2: fn.closure r2, f0, []
 3: fn.closure r3, f0, [r0]
 4: call.fn r4, r2, [r0, r1]
 5: call r5, f0, [r0, r1]
 6: call.fn r6, r3, [r1]
 7: int.add r4, r4, r5
 8: int.add r4, r4, r6
 9: ret r4
}
entry f1
"#;
    assert_eq!(run(source).unwrap(), 126);
}

#[test]
fn closure_bridge_recursive_calls_reenter_the_published_entry() {
    let source = r#"sparrow 16
module "recursion" "1.0.0"
type t0 int
type t1 bool
type t2 fn [t0] -> t0
function f0 "sum" {
 registers [t0, t0, t1, t2, t0, t0]
 params [r0]
 result t0
 0: const r1, int 0
 1: int.cmp eq, r2, r0, r1
 2: br r2, @3, @4
 3: ret r1
 4: const r1, int 1
 5: int.sub r4, r0, r1
 6: fn.closure r3, f0, []
 7: call.fn r5, r3, [r4]
 8: int.add r5, r5, r0
 9: ret r5
}
function f1 "main" {
 registers [t0, t2, t0]
 params []
 result t0
 0: const r0, int 4
 1: fn.closure r1, f0, []
 2: call.fn r2, r1, [r0]
 3: ret r2
}
entry f1
"#;
    assert_eq!(run(source).unwrap(), 10);
}

#[test]
fn closure_bridge_async_captures_survive_suspension_and_cancellation() {
    let source = include_str!("closure_bridge_async.spa");
    assert_eq!(run(source).unwrap(), 16);
    let cancelled = source.replace("call _, f2, [r0]", "fut cancel, _, r5");
    let error = run(&cancelled).unwrap_err();
    assert!(error.0.contains("Operation was cancelled"), "{}", error.0);
}
