sparrow 9
module "async_native_failure" "v1.0.0"
type t0 str
type t1 bool
type t2 future t1
type t3 int
type t4 failure
type t5 object obj0
role role0 "Error" builtin {
    method "text" [] -> t0
}
object obj0 "NativeFailure" builtin {
    field "domain" t0
    field "code" t0
    field "operation" t0
    field "message" t0
    does role0 [f1]
}
function f0 "main" async {
    registers [t0, t2, t1, t3, t4, t5, t0, t4, t4]
    params []
    result t3
    handler 1 6 @6 r4
    handler 7 11 @34 r7
    handler 11 15 @34 r7
    handler 15 19 @34 r7
    handler 19 23 @34 r7
    handler 23 25 @34 r7
    handler 26 28 @34 r7
    handler 28 30 @34 r7
    handler 30 32 @34 r7
    handler 32 34 @34 r7
    0: jmp @1
    1: const r0, str "01"
    2: call r1, f2, [r0]
    3: await r2, r1
    4: const r3, int 0
    5: jmp @25
    6: catch r4, object, 0, r5, @7, @36
    7: call r0, f3, [r5]
    8: const r6, str "core.time"
    9: str.cmp eq, r2, r0, r6
    10: br r2, @11, @32
    11: call r0, f4, [r5]
    12: const r6, str "invalid_input"
    13: str.cmp eq, r2, r0, r6
    14: br r2, @15, @30
    15: call r0, f5, [r5]
    16: const r6, str "runtime/time@1::sleep_nanoseconds/1"
    17: str.cmp eq, r2, r0, r6
    18: br r2, @19, @28
    19: call r0, f6, [r5]
    20: const r6, str "Wait duration must be a canonical non-negative integer"
    21: str.cmp eq, r2, r0, r6
    22: br r2, @23, @26
    23: const r3, int 42
    24: jmp @25
    25: ret r3
    26: const r0, str "message"
    27: panic r0
    28: const r0, str "operation"
    29: panic r0
    30: const r0, str "code"
    31: panic r0
    32: const r0, str "domain"
    33: panic r0
    34: fail.caused r8, r7, r4
    35: raise r8
    36: raise r4
}
function f1 "NativeFailure.text" {
    registers [t5, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 3
    1: ret r1
}
function f2 "runtime.time::sleep_nanoseconds" async native "runtime/time@1::sleep_nanoseconds/1" {
    registers [t0]
    params [r0]
    result t1
}
function f3 "NativeFailure.domain" {
    registers [t5, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 0
    1: ret r1
}
function f4 "NativeFailure.code" {
    registers [t5, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 1
    1: ret r1
}
function f5 "NativeFailure.operation" {
    registers [t5, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 2
    1: ret r1
}
function f6 "NativeFailure.message" {
    registers [t5, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 3
    1: ret r1
}
entry f0
