sparrow 9
module "async_cancel_timer" "v1.0.0"
type t0 str
type t1 bool
type t2 future t1
type t3 int
type t4 failure
type t5 role role0
role role0 "Error" builtin {
    method "text" [] -> t0
}
function f0 "main" async {
    registers [t0, t2, t1, t3, t4, t5, t0, t4, t4]
    params []
    result t3
    handler 4 7 @7 r4
    handler 8 12 @26 r7
    handler 12 14 @26 r7
    handler 24 26 @26 r7
    0: const r0, str "9223372036854775807"
    1: call r1, f1, [r0]
    2: fut cancel, _, r1
    3: jmp @4
    4: await r2, r1
    5: const r3, int 0
    6: jmp @14
    7: catch r4, role, 0, r5, @8, @28
    8: call.role r0, r5, role0, 0, []
    9: const r6, str "Operation was cancelled"
    10: str.cmp eq, r2, r0, r6
    11: br r2, @12, @24
    12: const r3, int 42
    13: jmp @14
    14: fut done, r2, r1
    15: br r2, @16, @18
    16: fut cancelled, r2, r1
    17: jmp @20
    18: const r2, bool false
    19: jmp @20
    20: br r2, @21, @22
    21: ret r3
    22: const r0, str "cancel status"
    23: panic r0
    24: const r0, str "cancel text"
    25: panic r0
    26: fail.caused r8, r7, r4
    27: raise r8
    28: raise r4
}
function f1 "runtime.time::sleep_nanoseconds" async native "runtime/time@1::sleep_nanoseconds/1" {
    registers [t0]
    params [r0]
    result t1
}
entry f0
