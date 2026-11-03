sparrow 9
module "async_self_cancel_ready" "v1.0.0"
type t0 int
type t1 future t0
type t2 object obj0
type t3 object obj1
type t4 object obj2
type t5 bool
type t6 str
object obj0 "Ready" {
}
object obj1 "Maker" {
}
object obj2 "Job" {
    field "future" t1
}
function f0 "Ready.new#0" {
    registers [t2]
    params []
    result t2
    0: obj.new r0, obj0, []
    1: ret r0
}
function f1 "Ready.value" async {
    registers [t2, t0]
    params [r0]
    result t0
    0: const r1, int 42
    1: ret r1
}
function f2 "Maker.new#0" {
    registers [t3]
    params []
    result t3
    0: obj.new r0, obj1, []
    1: ret r0
}
function f3 "Maker.make" {
    registers [t3, t4, t1]
    params [r0, r1]
    result t1
    0: call r2, f6, [r1]
    1: ret r2
}
function f4 "Job.new#0" {
    registers [t3, t1, t4]
    params []
    result t4
    0: call r0, f2, []
    1: undef r1
    2: obj.new r2, obj2, [r1]
    3: call r1, f3, [r0, r2]
    4: obj.init r2, 0, r1
    5: ret r2
}
function f5 "Job.pending" {
    registers [t4, t1]
    params [r0]
    result t1
    0: obj.get r1, r0, 0
    1: ret r1
}
function f6 "Job.run" async {
    registers [t4, t1, t2, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 0
    1: fut cancel, _, r1
    2: call r2, f0, []
    3: call r1, f1, [r2]
    4: await r3, r1
    5: ret r3
}
function f7 "main" async {
    registers [t4, t1, t0, t5, t5, t6]
    params []
    result t0
    0: call r0, f4, []
    1: call r1, f5, [r0]
    2: await r2, r1
    3: fut done, r3, r1
    4: br r3, @5, @8
    5: fut cancelled, r3, r1
    6: bool.not r4, r3
    7: jmp @10
    8: const r3, bool false
    9: jmp @10(r4 <- r3)
    10: br r4, @11, @12
    11: ret r2
    12: const r5, str "ready await must not deliver cancellation"
    13: panic r5
}
entry f7
