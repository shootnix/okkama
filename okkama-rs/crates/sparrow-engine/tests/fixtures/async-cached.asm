sparrow 9
module "async_cached" "v1.0.0"
type t0 int
type t1 object obj0
type t2 object obj1
type t3 str
type t4 future t0
object obj0 "Job" {
    field "result" t0
}
object obj1 "Dormant" {
}
function f0 "Job.new#0" {
    registers [t0, t1]
    params [r0]
    result t1
    0: obj.new r1, obj0, [r0]
    1: ret r1
}
function f1 "Job.run" async {
    registers [t1, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 0
    1: ret r1
}
function f2 "Dormant.new#0" {
    registers [t2]
    params []
    result t2
    0: obj.new r0, obj1, []
    1: ret r0
}
function f3 "Dormant.run" async {
    registers [t2, t3]
    params [r0]
    result t0
    0: const r1, str "cold body must not run"
    1: panic r1
}
function f4 "main" async {
    registers [t2, t4, t0, t1, t0, t0]
    params []
    result t0
    0: call r0, f2, []
    1: call r1, f3, [r0]
    2: const r2, int 21
    3: call r3, f0, [r2]
    4: call r1, f1, [r3]
    5: await r2, r1
    6: await r4, r1
    7: int.add r5, r2, r4
    8: ret r5
}
entry f4
