sparrow 9
module "async_failure" "v1.0.0"
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
    registers [t2, t4, t3, t5, t1, t3, t3, t5, t5]
    params []
    result t3
    handler 3 5 @5 r3
    handler 6 8 @19 r7
    handler 9 11 @11 r3
    handler 12 14 @16 r7
    0: call r0, f2, []
    1: call r1, f3, [r0]
    2: jmp @3
    3: await r2, r1
    4: jmp @8
    5: catch r3, object, 0, r4, @6, @21
    6: const r2, int 21
    7: jmp @8
    8: jmp @9
    9: await r5, r1
    10: jmp @14
    11: catch r3, object, 0, r4, @12, @18
    12: const r5, int 21
    13: jmp @14
    14: int.add r6, r2, r5
    15: ret r6
    16: fail.caused r8, r7, r3
    17: raise r8
    18: raise r3
    19: fail.caused r8, r7, r3
    20: raise r8
    21: raise r3
}
entry f4
