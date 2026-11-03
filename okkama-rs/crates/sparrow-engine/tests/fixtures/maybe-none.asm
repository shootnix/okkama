sparrow 9
module "maybe_none" "v1.0.0"
type t0 int
type t1 array t0
type t2 object obj0
type t3 object obj1
type t4 bool
object obj0 "Source" {
}
object obj1 "$variant_Maybe_1" {
    field "$tag" t0
    field "$0_value" t1
}
function f0 "Source.new#0" {
    registers [t2]
    params []
    result t2
    0: obj.new r0, obj0, []
    1: ret r0
}
function f1 "Source.absent" {
    registers [t2, t0, t1, t3]
    params [r0]
    result t3
    0: const r1, int 1
    1: arr.new r2, []
    2: call r3, f3, [r1, r2]
    3: ret r3
}
function f2 "main" {
    registers [t2, t3, t0, t0, t4]
    params []
    result t0
    0: call r0, f0, []
    1: call r1, f1, [r0]
    2: call r2, f4, [r1]
    3: const r3, int 0
    4: int.cmp eq, r4, r2, r3
    5: br r4, @6, @8
    6: call r2, f5, [r1]
    7: jmp @10
    8: const r2, int 42
    9: jmp @10
    10: ret r2
}
function f3 "$variant_Maybe_1.new" {
    registers [t0, t1, t3]
    params [r0, r1]
    result t3
    0: obj.new r2, obj1, [r0, r1]
    1: ret r2
}
function f4 "$variant_Maybe_1.$field0" {
    registers [t3, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 0
    1: ret r1
}
function f5 "$variant_Maybe_1.$field1" {
    registers [t3, t0, t0, t4, t1]
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
entry f2
