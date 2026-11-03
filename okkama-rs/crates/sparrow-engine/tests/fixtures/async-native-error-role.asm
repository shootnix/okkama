sparrow 9
module "async_native_error_role" "v1.0.0"
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
    handler 1 6 @6 r4
    handler 7 11 @17 r7
    handler 11 13 @17 r7
    handler 13 15 @17 r7
    0: jmp @1
    1: const r0, str "01"
    2: call r1, f1, [r0]
    3: await r2, r1
    4: const r3, int 0
    5: jmp @16
    6: catch r4, role, 0, r5, @7, @19
    7: call.role r0, r5, role0, 0, []
    8: const r6, str "Wait duration must be a canonical non-negative integer"
    9: str.cmp eq, r2, r0, r6
    10: br r2, @11, @13
    11: const r3, int 42
    12: jmp @15
    13: const r3, int 0
    14: jmp @15
    15: jmp @16
    16: ret r3
    17: fail.caused r8, r7, r4
    18: raise r8
    19: raise r4
}
function f1 "runtime.time::sleep_nanoseconds" async native "runtime/time@1::sleep_nanoseconds/1" {
    registers [t0]
    params [r0]
    result t1
}
entry f0
