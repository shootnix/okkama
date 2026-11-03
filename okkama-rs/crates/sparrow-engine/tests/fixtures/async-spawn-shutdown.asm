sparrow 9
module "async_spawn_shutdown" "v1.0.0"
type t0 str
type t1 object obj0
type t2 object obj1
type t3 object obj2
type t4 region
type t5 bool
type t6 future t5
type t7 int
type t8 failure
type t9 future t7
role role0 "Error" builtin {
    method "text" [] -> t0
}
object obj0 "CleanupFailure" {
    does role0 [f0]
}
object obj1 "Resource" {
    destroy f2
}
object obj2 "Job" {
}
function f0 "CleanupFailure.text" {
    registers [t1, t0]
    params [r0]
    result t0
    0: jmp @1
    1: jmp @2
    2: const r1, str "cleanup"
    3: jmp @4
    4: ret r1
}
function f1 "CleanupFailure.new#0" {
    registers [t1]
    params []
    result t1
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj0, []
    3: jmp @4
    4: ret r0
}
function f2 "Resource.destroy" {
    registers [t2, t1]
    params [r0]
    result void
    0: jmp @1
    1: jmp @2
    2: call r1, f1, []
    3: throw r1
}
function f3 "Resource.new#0" {
    registers [t2]
    params []
    result t2
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj1, []
    3: jmp @4
    4: ret r0
}
function f4 "Job.new#0" {
    registers [t3]
    params []
    result t3
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj2, []
    3: jmp @4
    4: ret r0
}
function f5 "Job.run" async {
    registers [t3, t4, t2, t0, t6, t5, t7, t8, t8]
    params [r0]
    result t7
    handler 3 11 @14 r7
    0: jmp @1
    1: region.enter r1
    2: jmp @3
    3: call r2, f3, []
    4: region.track r1, r2
    5: region.adopt r1, r2
    6: const r3, str "9223372036854775807"
    7: call r4, f7, [r3]
    8: await r5, r4
    9: const r6, int 0
    10: jmp @11
    11: region.leave r1, _
    12: jmp @13
    13: ret r6
    14: region.unwind r8, r1, r7
    15: raise r8
}
function f6 "main" async {
    registers [t3, t9, t9, t7]
    params []
    result t7
    0: jmp @1
    1: jmp @2
    2: call r0, f4, []
    3: call r1, f5, [r0]
    4: fut.spawn r2, r1
    5: const r3, int 42
    6: jmp @7
    7: ret r3
}
function f7 "runtime.time::sleep_nanoseconds" async native "runtime/time@1::sleep_nanoseconds/1" {
    registers [t0]
    params [r0]
    result t5
}
entry f6
