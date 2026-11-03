sparrow 9
module "resource_cleanup" "v1.0.0"
type t0 str
type t1 int
type t2 object obj0
type t3 object obj1
type t4 object obj2
type t5 region
type t6 failure
role role0 "Error" builtin {
    method "text" [] -> t0
}
object obj0 "Released" {
    field "code" t1
    does role0 [f0]
}
object obj1 "Resource" {
    destroy f3
}
object obj2 "Scope" {
}
function f0 "Released.text" {
    registers [t2, t0]
    params [r0]
    result t0
    0: jmp @1
    1: jmp @2
    2: const r1, str "released"
    3: jmp @4
    4: ret r1
}
function f1 "Released.new#0" {
    registers [t1, t2]
    params [r0]
    result t2
    0: jmp @1
    1: jmp @2
    2: obj.new r1, obj0, [r0]
    3: jmp @4
    4: ret r1
}
function f2 "Released.value" {
    registers [t2, t1]
    params [r0]
    result t1
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: jmp @4
    4: ret r1
}
function f3 "Resource.destroy" {
    registers [t3, t1, t2]
    params [r0]
    result void
    0: jmp @1
    1: jmp @2
    2: const r1, int 42
    3: call r2, f1, [r1]
    4: throw r2
}
function f4 "Resource.new#0" {
    registers [t3]
    params []
    result t3
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj1, []
    3: jmp @4
    4: ret r0
}
function f5 "Scope.new#0" {
    registers [t4]
    params []
    result t4
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj2, []
    3: jmp @4
    4: ret r0
}
function f6 "Scope.close" {
    registers [t4, t5, t3, t1, t6, t6]
    params [r0]
    result t1
    handler 4 8 @13 r4
    0: jmp @1
    1: jmp @2
    2: region.enter r1
    3: jmp @4
    4: call r2, f4, []
    5: region.track r1, r2
    6: region.adopt r1, r2
    7: jmp @8
    8: region.leave r1, _
    9: jmp @10
    10: const r3, int 0
    11: jmp @12
    12: ret r3
    13: region.unwind r5, r1, r4
    14: raise r5
}
function f7 "main" {
    registers [t4, t1, t6, t2, t6, t6]
    params []
    result t1
    handler 2 3 @6 r2
    handler 3 6 @6 r2
    handler 7 8 @12 r4
    handler 8 10 @12 r4
    0: jmp @1
    1: jmp @2
    2: jmp @3
    3: call r0, f5, []
    4: call r1, f6, [r0]
    5: jmp @10
    6: catch r2, object, 0, r3, @7, @14
    7: jmp @8
    8: call r1, f2, [r3]
    9: jmp @10
    10: jmp @11
    11: ret r1
    12: fail.caused r5, r4, r2
    13: raise r5
    14: raise r2
}
entry f7
