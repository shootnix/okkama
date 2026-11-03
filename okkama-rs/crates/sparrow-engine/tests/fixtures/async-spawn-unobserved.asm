sparrow 9
module "async_spawn_unobserved" "v1.0.0"
type t0 str
type t1 object obj0
type t2 object obj1
type t3 int
type t4 future t3
role role0 "Error" builtin {
    method "text" [] -> t0
}
object obj0 "Problem" {
    does role0 [f0]
}
object obj1 "Job" {
}
function f0 "Problem.text" {
    registers [t1, t0]
    params [r0]
    result t0
    0: const r1, str "problem"
    1: ret r1
}
function f1 "Problem.new#0" {
    registers [t1]
    params []
    result t1
    0: obj.new r0, obj0, []
    1: ret r0
}
function f2 "Job.new#0" {
    registers [t2]
    params []
    result t2
    0: obj.new r0, obj1, []
    1: ret r0
}
function f3 "Job.run" async {
    registers [t2, t1]
    params [r0]
    result t3
    0: call r1, f1, []
    1: throw r1
}
function f4 "main" async {
    registers [t2, t4, t4, t3]
    params []
    result t3
    0: call r0, f2, []
    1: call r1, f3, [r0]
    2: fut.spawn r2, r1
    3: const r3, int 42
    4: ret r3
}
entry f4
