sparrow 9
module "async_spawn_observed" "v1.0.0"
type t0 str
type t1 object obj0
type t2 object obj1
type t3 int
type t4 future t3
type t5 failure
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
    registers [t2, t4, t4, t3, t5, t1, t3, t3, t5, t5]
    params []
    result t3
    handler 4 6 @6 r4
    handler 7 9 @20 r8
    handler 10 12 @12 r4
    handler 13 15 @17 r8
    0: call r0, f2, []
    1: call r1, f3, [r0]
    2: fut.spawn r2, r1
    3: jmp @4
    4: await r3, r2
    5: jmp @9
    6: catch r4, object, 0, r5, @7, @22
    7: const r3, int 21
    8: jmp @9
    9: jmp @10
    10: await r6, r2
    11: jmp @15
    12: catch r4, object, 0, r5, @13, @19
    13: const r6, int 21
    14: jmp @15
    15: int.add r7, r3, r6
    16: ret r7
    17: fail.caused r9, r8, r4
    18: raise r9
    19: raise r4
    20: fail.caused r9, r8, r4
    21: raise r9
    22: raise r4
}
entry f4
