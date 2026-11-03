sparrow 9
module "app" "v1.0.0"
type t0 str
type t1 int
type t2 future t1
type t3 object obj0
type t4 object obj1
type t5 object obj2
type t6 region
type t7 bool
type t8 future t7
type t9 failure
type t10 object obj3
type t11 future t10
type t12 object obj4
role role0 "Error" builtin {
    method "text" [] -> t0
}
role role1 "async.scope::Supervising<int>" {
    method "start" [t2] -> void
    method "active" [] -> t1
    method "cancel" [] -> void
}
object obj0 "CleanupFailure" {
    does role0 [f0]
}
object obj1 "Resource" {
    destroy f2
}
object obj2 "Job" {
}
object obj3 "async.scope::FutureGroup<int>" {
    does role1 [f8, f9, f10]
}
object obj4 "async.scope::FinishedFutureGroup" {
    does role0 [f13]
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
function f4 "Job.new#0" {
    registers [t5]
    params []
    result t5
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj2, []
    3: jmp @4
    4: ret r0
}
function f5 "Job.run" async {
    registers [t5, t6, t4, t0, t8, t7, t1, t9, t9]
    params [r0]
    result t1
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
    registers [t10, t5, t2, t11, t1]
    params []
    result t1
    0: jmp @1
    1: jmp @2
    2: call r0, f11, []
    3: call r1, f4, []
    4: call r2, f5, [r1]
    5: call _, f8, [r0, r2]
    6: call _, f10, [r0]
    7: call r3, f12, [r0]
    8: await r0, r3
    9: const r4, int 42
    10: jmp @11
    11: ret r4
}
function f7 "runtime.time::sleep_nanoseconds" async native "runtime/time@1::sleep_nanoseconds/1" {
    registers [t0]
    params [r0]
    result t7
}
function f8 "async.scope::FutureGroup<int>.start" intrinsic "okkama.scope.FutureGroup.start" [obj4] [] {
    registers [t10, t2]
    params [r0, r1]
    result void
}
function f9 "async.scope::FutureGroup<int>.active" intrinsic "okkama.scope.FutureGroup.active" [] [] {
    registers [t10]
    params [r0]
    result t1
}
function f10 "async.scope::FutureGroup<int>.cancel" intrinsic "okkama.scope.FutureGroup.cancel" [] [] {
    registers [t10]
    params [r0]
    result void
}
function f11 "async.scope::FutureGroup<int>.new#0" intrinsic "okkama.scope.FutureGroup.new" [obj3] [] {
    registers []
    params []
    result t10
}
function f12 "async.scope::FutureGroup<int>.wait" async intrinsic "okkama.scope.FutureGroup.wait" [] [] {
    registers [t10]
    params [r0]
    result t10
}
function f13 "async.scope::FinishedFutureGroup.text" {
    registers [t12, t0]
    params [r0]
    result t0
    0: jmp @1
    1: jmp @2
    2: const r1, str "future group is already finishing"
    3: jmp @4
    4: ret r1
}
entry f6
