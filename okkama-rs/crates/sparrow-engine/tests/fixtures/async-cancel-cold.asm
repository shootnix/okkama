sparrow 9
module "async_cancel_cold" "v1.0.0"
type t0 str
type t1 object obj0
type t2 int
type t3 future t2
type t4 bool
type t5 failure
type t6 object obj1
role role0 "Error" builtin {
    method "text" [] -> t0
}
object obj0 "Job" {
}
object obj1 "Cancelled" builtin {
    does role0 [f3]
}
function f0 "Job.new#0" {
    registers [t1]
    params []
    result t1
    0: obj.new r0, obj0, []
    1: ret r0
}
function f1 "Job.run" async {
    registers [t1, t0]
    params [r0]
    result t2
    0: const r1, str "cancelled cold body must not run"
    1: panic r1
}
function f2 "main" async {
    registers [t1, t3, t4, t4, t2, t5, t6, t0, t5, t5]
    params []
    result t2
    handler 8 10 @10 r5
    handler 11 13 @23 r8
    0: call r0, f0, []
    1: call r1, f1, [r0]
    2: fut done, r2, r1
    3: bool.not r3, r2
    4: br r3, @5, @26
    5: fut cancel, _, r1
    6: fut cancel, _, r1
    7: jmp @8
    8: await r4, r1
    9: jmp @13
    10: catch r5, object, 1, r6, @11, @25
    11: const r4, int 42
    12: jmp @13
    13: fut done, r2, r1
    14: br r2, @15, @17
    15: fut cancelled, r2, r1
    16: jmp @19
    17: const r2, bool false
    18: jmp @19
    19: br r2, @20, @21
    20: ret r4
    21: const r7, str "cancel status"
    22: panic r7
    23: fail.caused r9, r8, r5
    24: raise r9
    25: raise r5
    26: const r7, str "not cold"
    27: panic r7
}
function f3 "Cancelled.text" {
    registers [t6, t0]
    params [r0]
    result t0
    0: const r1, str "Operation was cancelled"
    1: ret r1
}
entry f2
