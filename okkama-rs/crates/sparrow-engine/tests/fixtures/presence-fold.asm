sparrow 9
module "presence_fold" "v1.0.0"
type t0 int
type t1 array t0
type t2 object obj0
type t3 bool
type t4 object obj1
object obj0 "$variant_Presence_0" {
    field "$tag" t0
    field "$0_value" t1
}
object obj1 "$variant_Maybe_1" {
    field "$tag" t0
    field "$0_value" t1
}
function f0 "main" {
    registers [t0, t0, t1, t2, t3, t4]
    params []
    result t0
    0: const r0, int 0
    1: const r1, int 42
    2: arr.new r2, [r1]
    3: call r3, f1, [r0, r2]
    4: call r0, f2, [r3]
    5: const r1, int 0
    6: int.cmp eq, r4, r0, r1
    7: br r4, @8, @13
    8: const r0, int 0
    9: call r1, f3, [r3]
    10: arr.new r2, [r1]
    11: call r5, f4, [r0, r2]
    12: jmp @17
    13: const r0, int 1
    14: arr.new r2, []
    15: call r5, f4, [r0, r2]
    16: jmp @17
    17: call r0, f5, [r5]
    18: const r1, int 0
    19: int.cmp eq, r4, r0, r1
    20: br r4, @21, @23
    21: call r0, f6, [r5]
    22: jmp @25
    23: const r0, int 0
    24: jmp @25
    25: ret r0
}
function f1 "$variant_Presence_0.new" {
    registers [t0, t1, t2]
    params [r0, r1]
    result t2
    0: obj.new r2, obj0, [r0, r1]
    1: ret r2
}
function f2 "$variant_Presence_0.$field0" {
    registers [t2, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 0
    1: ret r1
}
function f3 "$variant_Presence_0.$field1" {
    registers [t2, t0, t0, t3, t1]
    params [r0]
    result t0
    0: obj.get r1, r0, 0
    1: const r2, int 0
    2: int.cmp eq, r3, r1, r2
    3: br r3, @4, @8
    4: obj.get r4, r0, 1
    5: const r1, int 0
    6: arr.get r2, r4, r1
    7: ret r2
    8: trap empty, "Cannot extract payload of an inactive enum variant"
}
function f4 "$variant_Maybe_1.new" {
    registers [t0, t1, t4]
    params [r0, r1]
    result t4
    0: obj.new r2, obj1, [r0, r1]
    1: ret r2
}
function f5 "$variant_Maybe_1.$field0" {
    registers [t4, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 0
    1: ret r1
}
function f6 "$variant_Maybe_1.$field1" {
    registers [t4, t0, t0, t3, t1]
    params [r0]
    result t0
    0: obj.get r1, r0, 0
    1: const r2, int 0
    2: int.cmp eq, r3, r1, r2
    3: br r3, @4, @8
    4: obj.get r4, r0, 1
    5: const r1, int 0
    6: arr.get r2, r4, r1
    7: ret r2
    8: trap empty, "Cannot extract payload of an inactive enum variant"
}
entry f0
