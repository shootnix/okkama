sparrow 9
module "async_worker_cancel" "v1.0.0"
type t0 str
type t1 object obj0
type t2 int
type t3 fn [] -> t2
type t4 future t2
type t5 bool
type t6 future t5
type t7 failure
type t8 object obj1
role role0 "Error" builtin {
    method "text" [] -> t0
}
object obj0 "Endless" {
}
object obj1 "Cancelled" builtin {
    does role0 [f5]
}
function f0 "Endless.new#0" {
    registers [t1]
    params []
    result t1
    0: obj.new r0, obj0, []
    1: ret r0
}
function f1 "Endless.run" {
    registers [t1]
    params [r0]
    result t2
    0: jmp @1
    1: jmp @1
}
function f2 "main" async {
    registers [t3, t4, t0, t6, t5, t2, t7, t8, t7, t7]
    params []
    result t2
    handler 8 11 @11 r6
    handler 12 14 @30 r8
    0: fn.closure r0, f3, []
    1: worker.spawn r1, r0, [], true
    2: const r2, str "10000000"
    3: call r3, f4, [r2]
    4: await r4, r3
    5: fut cancel, _, r1
    6: fut cancel, _, r1
    7: jmp @8
    8: await r5, r1
    9: const r4, bool false
    10: jmp @14
    11: catch r6, object, 1, r7, @12, @32
    12: const r4, bool true
    13: jmp @14
    14: br r4, @15, @17
    15: fut cancelled, r4, r1
    16: jmp @19
    17: const r4, bool false
    18: jmp @19
    19: br r4, @20, @22
    20: fut done, r4, r1
    21: jmp @24
    22: const r4, bool false
    23: jmp @24
    24: br r4, @25, @29
    25: fn.closure r0, f3, []
    26: worker.spawn r1, r0, [], true
    27: const r5, int 42
    28: ret r5
    29: trap confirm, "confirm failed"
    30: fail.caused r9, r8, r6
    31: raise r9
    32: raise r6
}
function f3 "Endless.run$worker#3" {
    registers [t1, t2]
    params []
    result t2
    0: call r0, f0, []
    1: call r1, f1, [r0]
    2: ret r1
}
function f4 "runtime.time::sleep_nanoseconds" async native "runtime/time@1::sleep_nanoseconds/1" {
    registers [t0]
    params [r0]
    result t5
}
function f5 "Cancelled.text" {
    registers [t8, t0]
    params [r0]
    result t0
    0: const r1, str "Operation was cancelled"
    1: ret r1
}
entry f2
