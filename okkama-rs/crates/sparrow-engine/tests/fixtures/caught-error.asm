sparrow 9
module "caught_error" "v1.0.0"
type t0 str
type t1 object obj0
type t2 object obj1
type t3 int
type t4 failure
role role0 "Error" builtin {
    method "text" [] -> t0
}
object obj0 "Problem" {
    does role0 [f0]
}
object obj1 "Source" {
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
function f2 "Source.new#0" {
    registers [t2]
    params []
    result t2
    0: obj.new r0, obj1, []
    1: ret r0
}
function f3 "Source.value" {
    registers [t2, t1]
    params [r0]
    result t3
    0: call r1, f1, []
    1: throw r1
}
function f4 "main" {
    registers [t2, t3, t4, t1, t4, t4]
    params []
    result t3
    handler 1 4 @4 r2
    handler 5 7 @8 r4
    0: jmp @1
    1: call r0, f2, []
    2: call r1, f3, [r0]
    3: jmp @7
    4: catch r2, object, 0, r3, @5, @10
    5: const r1, int 42
    6: jmp @7
    7: ret r1
    8: fail.caused r5, r4, r2
    9: raise r5
    10: raise r2
}
entry f4
