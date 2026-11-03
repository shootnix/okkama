sparrow 9
module "async_native_rethrow" "v1.0.0"
type t0 str
type t1 role role0
type t2 object obj0
type t3 int
type t4 bool
type t5 future t4
type t6 failure
type t7 object obj1
role role0 "Error" builtin {
    method "text" [] -> t0
}
object obj0 "Relay" {
    field "error" t1
}
object obj1 "NativeFailure" builtin {
    field "domain" t0
    field "code" t0
    field "operation" t0
    field "message" t0
    does role0 [f3]
}
function f0 "Relay.new#0" {
    registers [t1, t2]
    params [r0]
    result t2
    0: obj.new r1, obj0, [r0]
    1: ret r1
}
function f1 "Relay.fail" {
    registers [t2, t1]
    params [r0]
    result t3
    0: obj.get r1, r0, 0
    1: throw r1
}
function f2 "main" async {
    registers [t0, t5, t4, t3, t6, t7, t1, t2, t6, t6, t0]
    params []
    result t3
    handler 1 6 @6 r4
    handler 7 11 @12 r8
    handler 12 14 @15 r4
    handler 14 15 @15 r4
    handler 16 20 @26 r8
    handler 20 22 @26 r8
    handler 22 24 @26 r8
    0: jmp @1
    1: const r0, str "01"
    2: call r1, f4, [r0]
    3: await r2, r1
    4: const r3, int 0
    5: jmp @11
    6: catch r4, object, 1, r5, @7, @14
    7: role.wrap r6, r5, role0
    8: call r7, f0, [r6]
    9: call r3, f1, [r7]
    10: jmp @11
    11: jmp @25
    12: fail.caused r9, r8, r4
    13: raise r9
    14: raise r4
    15: catch r4, object, 1, r5, @16, @28
    16: call r0, f5, [r5]
    17: const r10, str "invalid_input"
    18: str.cmp eq, r2, r0, r10
    19: br r2, @20, @22
    20: const r3, int 42
    21: jmp @24
    22: const r3, int 0
    23: jmp @24
    24: jmp @25
    25: ret r3
    26: fail.caused r9, r8, r4
    27: raise r9
    28: raise r4
}
function f3 "NativeFailure.text" {
    registers [t7, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 3
    1: ret r1
}
function f4 "runtime.time::sleep_nanoseconds" async native "runtime/time@1::sleep_nanoseconds/1" {
    registers [t0]
    params [r0]
    result t4
}
function f5 "NativeFailure.code" {
    registers [t7, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 1
    1: ret r1
}
entry f2
