sparrow 9
module "async_self_cancel_cleanup" "v1.0.0"
type t0 str
type t1 int
type t2 future t1
type t3 object obj0
type t4 object obj1
type t5 object obj2
type t6 bool
type t7 future t6
type t8 object obj3
type t9 object obj4
type t10 region
type t11 failure
role role0 "Error" builtin {
    method "text" [] -> t0
}
object obj0 "CleanupFailure" {
    does role0 [f0]
}
object obj1 "Resource" {
    destroy f2
}
object obj2 "Waiter" {
}
object obj3 "Maker" {
}
object obj4 "Job" {
    field "future" t2
}
function f0 "CleanupFailure.text" {
    registers [t3, t0]
    params [r0]
    result t0
    0: jmp @1
    1: jmp @2
    2: const r1, str "cleanup"
    3: jmp @4
    4: ret r1
}
function f1 "CleanupFailure.new#0" {
    registers [t3]
    params []
    result t3
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj0, []
    3: jmp @4
    4: ret r0
}
function f2 "Resource.destroy" {
    registers [t4, t3]
    params [r0]
    result void
    0: jmp @1
    1: jmp @2
    2: call r1, f1, []
    3: throw r1
}
function f3 "Resource.new#0" {
    registers [t4]
    params []
    result t4
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj1, []
    3: jmp @4
    4: ret r0
}
function f4 "Waiter.new#0" {
    registers [t5]
    params []
    result t5
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj2, []
    3: jmp @4
    4: ret r0
}
function f5 "Waiter.value" async {
    registers [t5, t0, t7, t6, t1]
    params [r0]
    result t1
    0: jmp @1
    1: jmp @2
    2: const r1, str "20000000"
    3: call r2, f12, [r1]
    4: await r3, r2
    5: const r4, int 42
    6: jmp @7
    7: ret r4
}
function f6 "Maker.new#0" {
    registers [t8]
    params []
    result t8
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj3, []
    3: jmp @4
    4: ret r0
}
function f7 "Maker.make" {
    registers [t8, t9, t2]
    params [r0, r1]
    result t2
    0: jmp @1
    1: jmp @2
    2: call r2, f10, [r1]
    3: jmp @4
    4: ret r2
}
function f8 "Job.new#0" {
    registers [t8, t2, t9]
    params []
    result t9
    0: jmp @1
    1: jmp @2
    2: call r0, f6, []
    3: undef r1
    4: obj.new r2, obj4, [r1]
    5: call r1, f7, [r0, r2]
    6: obj.init r2, 0, r1
    7: jmp @8
    8: ret r2
}
function f9 "Job.pending" {
    registers [t9, t2]
    params [r0]
    result t2
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: jmp @4
    4: ret r1
}
function f10 "Job.run" async {
    registers [t9, t10, t4, t2, t5, t1, t11, t11]
    params [r0]
    result t1
    handler 3 12 @15 r6
    0: jmp @1
    1: region.enter r1
    2: jmp @3
    3: call r2, f3, []
    4: region.track r1, r2
    5: region.adopt r1, r2
    6: obj.get r3, r0, 0
    7: fut cancel, _, r3
    8: call r4, f4, []
    9: call r3, f5, [r4]
    10: await r5, r3
    11: jmp @12
    12: region.leave r1, _
    13: jmp @14
    14: ret r5
    15: region.unwind r7, r1, r6
    16: raise r7
}
function f11 "main" async {
    registers [t9, t2, t1]
    params []
    result t1
    0: jmp @1
    1: jmp @2
    2: call r0, f8, []
    3: call r1, f9, [r0]
    4: await r2, r1
    5: jmp @6
    6: ret r2
}
function f12 "runtime.time::sleep_nanoseconds" async native "runtime/time@1::sleep_nanoseconds/1" {
    registers [t0]
    params [r0]
    result t6
}
entry f11
