sparrow 9
module "resource_return" "v1.0.0"
type t0 str
type t1 object obj0
type t2 object obj1
type t3 object obj2
type t4 region
type t5 int
type t6 failure
type t7 object obj3
type t8 bool
role role0 "Error" builtin {
    method "text" [] -> t0
}
object obj0 "Released" {
    does role0 [f0]
}
object obj1 "Early" {
    does role0 [f2]
}
object obj2 "Resource" {
    destroy f4
}
object obj3 "Factory" {
}
function f0 "Released.text" {
    registers [t1, t0]
    params [r0]
    result t0
    0: jmp @1
    1: jmp @2
    2: const r1, str "released too early"
    3: jmp @4
    4: ret r1
}
function f1 "Released.new#0" {
    registers [t1]
    params []
    result t1
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj0, []
    3: jmp @4
    4: ret r0
}
function f2 "Early.text" {
    registers [t2, t0]
    params [r0]
    result t0
    0: jmp @1
    1: jmp @2
    2: const r1, str "early cleanup"
    3: jmp @4
    4: ret r1
}
function f3 "Early.new#0" {
    registers [t2]
    params []
    result t2
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj1, []
    3: jmp @4
    4: ret r0
}
function f4 "Resource.destroy" {
    registers [t3, t1]
    params [r0]
    result void
    0: jmp @1
    1: jmp @2
    2: call r1, f1, []
    3: throw r1
}
function f5 "Resource.new#0" {
    registers [t3]
    params []
    result t3
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj2, []
    3: jmp @4
    4: ret r0
}
function f6 "Resource.value" {
    registers [t3, t4, t5, t6, t6]
    params [r0]
    result t5
    handler 2 4 @8 r3
    handler 4 6 @8 r3
    0: region.enter r1
    1: jmp @2
    2: region.adopt r1, r0
    3: jmp @4
    4: const r2, int 42
    5: jmp @6
    6: region.leave r1, _
    7: ret r2
    8: region.unwind r4, r1, r3
    9: raise r4
}
function f7 "Factory.new#0" {
    registers [t7]
    params []
    result t7
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj3, []
    3: jmp @4
    4: ret r0
}
function f8 "Factory.make" {
    registers [t7, t4, t4, t4, t3, t6, t6, t1, t2, t6]
    params [r0]
    result t3
    handler 2 4 @33 r5
    handler 4 6 @24 r5
    handler 6 10 @22 r5
    handler 10 12 @24 r5
    handler 12 14 @24 r5
    handler 14 16 @31 r5
    handler 16 18 @33 r5
    handler 18 20 @33 r5
    handler 22 24 @24 r5
    handler 25 26 @28 r6
    handler 26 28 @28 r6
    handler 28 30 @31 r5
    handler 30 31 @31 r5
    handler 31 33 @33 r5
    0: region.enter r1
    1: jmp @2
    2: region.enter r2
    3: jmp @4
    4: region.enter r3
    5: jmp @6
    6: call r4, f5, []
    7: region.track r3, r4
    8: region.adopt r3, r4
    9: jmp @10
    10: region.leave r3, r4
    11: jmp @12
    12: region.track r2, r4
    13: jmp @14
    14: region.adopt r2, r4
    15: jmp @16
    16: region.leave r2, r4
    17: jmp @18
    18: region.track r1, r4
    19: jmp @20
    20: region.leave r1, r4
    21: ret r4
    22: region.unwind r6, r3, r5
    23: raise r6
    24: catch r5, object, 0, r7, @25, @30
    25: jmp @26
    26: call r8, f3, []
    27: throw r8
    28: fail.caused r9, r6, r5
    29: raise r9
    30: raise r5
    31: region.unwind r6, r2, r5
    32: raise r6
    33: region.unwind r6, r1, r5
    34: raise r6
}
function f9 "main" {
    registers [t4, t7, t3, t5, t5, t8, t0, t6, t6, t1, t6, t2]
    params []
    result t5
    handler 2 4 @20 r7
    handler 4 12 @18 r7
    handler 12 14 @18 r7
    handler 14 16 @20 r7
    handler 16 18 @18 r7
    handler 18 20 @20 r7
    handler 21 22 @24 r8
    handler 22 24 @24 r8
    handler 27 28 @32 r8
    handler 28 30 @32 r8
    0: jmp @1
    1: jmp @2
    2: region.enter r0
    3: jmp @4
    4: call r1, f7, []
    5: call r2, f8, [r1]
    6: region.track r0, r2
    7: region.adopt r0, r2
    8: call r3, f6, [r2]
    9: const r4, int 42
    10: int.cmp eq, r5, r3, r4
    11: br r5, @12, @16
    12: const r3, int 42
    13: jmp @14
    14: region.leave r0, _
    15: jmp @30
    16: const r6, str "wrong value"
    17: panic r6
    18: region.unwind r8, r0, r7
    19: raise r8
    20: catch r7, object, 0, r9, @21, @26
    21: jmp @22
    22: const r3, int 42
    23: jmp @30
    24: fail.caused r10, r8, r7
    25: raise r10
    26: catch r7, object, 1, r11, @27, @34
    27: jmp @28
    28: const r3, int 0
    29: jmp @30
    30: jmp @31
    31: ret r3
    32: fail.caused r10, r8, r7
    33: raise r10
    34: raise r7
}
entry f9
