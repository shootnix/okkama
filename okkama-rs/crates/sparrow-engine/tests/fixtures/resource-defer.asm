sparrow 9
module "resource_defer" "v1.0.0"
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
object obj0 "Failure" {
    field "code" t1
    does role0 [f0]
}
object obj1 "Resource" {
    destroy f3
}
object obj2 "Action" {
}
function f0 "Failure.text" {
    registers [t2, t0]
    params [r0]
    result t0
    0: jmp @1
    1: jmp @2
    2: const r1, str "failure"
    3: jmp @4
    4: ret r1
}
function f1 "Failure.new#0" {
    registers [t1, t2]
    params [r0]
    result t2
    0: jmp @1
    1: jmp @2
    2: obj.new r1, obj0, [r0]
    3: jmp @4
    4: ret r1
}
function f2 "Failure.value" {
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
    2: const r1, int 3
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
function f5 "Action.new#0" {
    registers [t4]
    params []
    result t4
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj2, []
    3: jmp @4
    4: ret r0
}
function f6 "Action.fail" {
    registers [t4, t1, t2]
    params [r0]
    result void
    0: jmp @1
    1: jmp @2
    2: const r1, int 42
    3: call r2, f1, [r1]
    4: throw r2
}
function f7 "main" {
    registers [t5, t3, t1, t4, t6, t6, t6, t2]
    params []
    result t1
    handler 2 4 @22 r4
    handler 4 8 @20 r4
    handler 8 10 @15 r4
    handler 10 13 @20 r4
    handler 13 15 @22 r4
    handler 15 18 @18 r5
    handler 18 20 @22 r4
    handler 20 22 @22 r4
    handler 23 24 @28 r5
    handler 24 26 @28 r5
    0: jmp @1
    1: jmp @2
    2: region.enter r0
    3: jmp @4
    4: call r1, f4, []
    5: region.track r0, r1
    6: region.adopt r0, r1
    7: jmp @8
    8: const r2, int 0
    9: jmp @10
    10: call r3, f5, []
    11: call _, f6, [r3]
    12: jmp @13
    13: region.leave r0, _
    14: jmp @26
    15: call r3, f5, []
    16: call _, f6, [r3]
    17: jmp @20
    18: fail.secondary r6, r4, r5
    19: jmp @20(r4 <- r6)
    20: region.unwind r5, r0, r4
    21: raise r5
    22: catch r4, object, 0, r7, @23, @30
    23: jmp @24
    24: call r2, f2, [r7]
    25: jmp @26
    26: jmp @27
    27: ret r2
    28: fail.caused r6, r5, r4
    29: raise r6
    30: raise r4
}
entry f7
