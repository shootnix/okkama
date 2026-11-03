sparrow 9
module "async_group_failures" "v1.0.0"
type t0 str
type t1 int
type t2 future t1
type t3 object obj0
type t4 object obj1
type t5 object obj2
type t6 future t5
type t7 object obj3
role role0 "Error" builtin {
    method "text" [] -> t0
}
role role1 "async.scope::Supervising<int>" {
    method "start" [t2] -> void
    method "active" [] -> t1
    method "cancel" [] -> void
}
object obj0 "Problem" {
    field "label" t0
    does role0 [f0]
}
object obj1 "Work" {
    field "label" t0
}
object obj2 "async.scope::FutureGroup<int>" {
    does role1 [f5, f6, f7]
}
object obj3 "async.scope::FinishedFutureGroup" {
    does role0 [f10]
}
function f0 "Problem.text" {
    registers [t3, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 0
    1: ret r1
}
function f1 "Problem.new#0" {
    registers [t0, t3]
    params [r0]
    result t3
    0: obj.new r1, obj0, [r0]
    1: ret r1
}
function f2 "Work.new#0" {
    registers [t0, t4]
    params [r0]
    result t4
    0: obj.new r1, obj1, [r0]
    1: ret r1
}
function f3 "Work.run" async {
    registers [t4, t0, t3]
    params [r0]
    result t1
    0: obj.get r1, r0, 0
    1: call r2, f1, [r1]
    2: throw r2
}
function f4 "main" async {
    registers [t5, t0, t4, t2, t6, t1]
    params []
    result t1
    0: call r0, f8, []
    1: const r1, str "first"
    2: call r2, f2, [r1]
    3: call r3, f3, [r2]
    4: call _, f5, [r0, r3]
    5: const r1, str "second"
    6: call r2, f2, [r1]
    7: call r3, f3, [r2]
    8: call _, f5, [r0, r3]
    9: call r4, f9, [r0]
    10: await r0, r4
    11: const r5, int 42
    12: ret r5
}
function f5 "async.scope::FutureGroup<int>.start" intrinsic "okkama.scope.FutureGroup.start" [obj3] [] {
    registers [t5, t2]
    params [r0, r1]
    result void
}
function f6 "async.scope::FutureGroup<int>.active" intrinsic "okkama.scope.FutureGroup.active" [] [] {
    registers [t5]
    params [r0]
    result t1
}
function f7 "async.scope::FutureGroup<int>.cancel" intrinsic "okkama.scope.FutureGroup.cancel" [] [] {
    registers [t5]
    params [r0]
    result void
}
function f8 "async.scope::FutureGroup<int>.new#0" intrinsic "okkama.scope.FutureGroup.new" [obj2] [] {
    registers []
    params []
    result t5
}
function f9 "async.scope::FutureGroup<int>.wait" async intrinsic "okkama.scope.FutureGroup.wait" [] [] {
    registers [t5]
    params [r0]
    result t5
}
function f10 "async.scope::FinishedFutureGroup.text" {
    registers [t7, t0]
    params [r0]
    result t0
    0: const r1, str "future group is already finishing"
    1: ret r1
}
entry f4
