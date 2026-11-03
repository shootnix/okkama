sparrow 9
module "fake_storage_fixture" "v1.0.0"
type t0 str
type t1 byte
type t2 array t1
type t3 bool
type t4 int
type t5 object obj0
role role0 "Storage" {
    method "put" [t0, t2] -> void
    method "get" [t0] -> t2
    method "has" [t0] -> t3
    method "remove" [t0] -> void
    method "clear" [] -> void
    method "size" [] -> t4
}
object obj0 "Storage$fake" {
    does role0 [f0, f1, f2, f3, f4, f5]
}
function f0 "Storage$fake.put" {
    registers [t5, t0, t2]
    params [r0, r1, r2]
    result void
    0: call _, f8, [r0, r1, r2]
    1: ret _
}
function f1 "Storage$fake.get" {
    registers [t5, t0, t2]
    params [r0, r1]
    result t2
    0: call r2, f9, [r0, r1]
    1: ret r2
}
function f2 "Storage$fake.has" {
    registers [t5, t0, t3]
    params [r0, r1]
    result t3
    0: call r2, f10, [r0, r1]
    1: ret r2
}
function f3 "Storage$fake.remove" {
    registers [t5, t0]
    params [r0, r1]
    result void
    0: call _, f11, [r0, r1]
    1: ret _
}
function f4 "Storage$fake.clear" {
    registers [t5]
    params [r0]
    result void
    0: call _, f12, [r0]
    1: ret _
}
function f5 "Storage$fake.size" {
    registers [t5, t4]
    params [r0]
    result t4
    0: call r1, f13, [r0]
    1: ret r1
}
function f6 "Storage$fake.new#0" {
    registers [t5]
    params []
    result t5
    0: obj.new r0, obj0, []
    1: ret r0
}
function f7 "test#0" {
    registers [t5, t5, t0, t1, t2, t4, t4, t3, t4, t4, t4, t2, t3]
    params []
    result void
    0: call r0, f6, []
    1: call r1, f6, []
    2: const r2, str "kept"
    3: const r3, byte 255
    4: arr.new r4, [r3]
    5: call _, f0, [r0, r2, r4]
    6: const r5, int 0
    7: const r6, int 300
    8: int.cmp le, r7, r5, r6
    9: const r2, str "loop"
    10: const r8, int 256
    11: const r10, int 1
    12: br r7, @13, @59
    13: jmp @14
    14: int.cmp lt, r7, r5, r6
    15: br r7, @16, @23
    16: int.mod r9, r5, r8
    17: byte.from r3, r9, _
    18: arr.new r4, [r3]
    19: call _, f0, [r0, r2, r4]
    20: jmp @21
    21: int.add r9, r5, r10
    22: jmp @14(r5 <- r9)
    23: const r2, str "kept"
    24: call r4, f1, [r0, r2]
    25: const r3, byte 255
    26: arr.new r11, [r3]
    27: call _, f14, [r4, r11]
    28: const r2, str "loop"
    29: call r4, f1, [r0, r2]
    30: const r3, byte 43
    31: arr.new r11, [r3]
    32: call _, f14, [r4, r11]
    33: call r5, f5, [r0]
    34: const r6, int 2
    35: call _, f15, [r5, r6]
    36: call r5, f5, [r1]
    37: const r6, int 0
    38: call _, f15, [r5, r6]
    39: const r2, str "kept"
    40: call r7, f2, [r1, r2]
    41: bool.not r12, r7
    42: call _, f16, [r12]
    43: const r2, str "kept"
    44: call r4, f1, [r0, r2]
    45: const r2, str "kept"
    46: call _, f3, [r0, r2]
    47: const r2, str "kept"
    48: call r7, f2, [r0, r2]
    49: bool.not r12, r7
    50: call _, f16, [r12]
    51: call _, f4, [r0]
    52: call r5, f5, [r0]
    53: const r6, int 0
    54: call _, f15, [r5, r6]
    55: const r3, byte 255
    56: arr.new r11, [r3]
    57: call _, f14, [r4, r11]
    58: ret _
    59: trap range, "Range start must be less than or equal to range end"
}
function f8 "okkama.fake.storage.set#8" intrinsic "okkama.fake.storage.set" [] [] {
    registers [t5, t0, t2]
    params [r0, r1, r2]
    result void
}
function f9 "okkama.fake.storage.get#9" intrinsic "okkama.fake.storage.get" [] [] {
    registers [t5, t0]
    params [r0, r1]
    result t2
}
function f10 "okkama.fake.storage.has#10" intrinsic "okkama.fake.storage.has" [] [] {
    registers [t5, t0]
    params [r0, r1]
    result t3
}
function f11 "okkama.fake.storage.remove#11" intrinsic "okkama.fake.storage.remove" [] [] {
    registers [t5, t0]
    params [r0, r1]
    result void
}
function f12 "okkama.fake.storage.clear#12" intrinsic "okkama.fake.storage.clear" [] [] {
    registers [t5]
    params [r0]
    result void
}
function f13 "okkama.fake.storage.size#13" intrinsic "okkama.fake.storage.size" [] [] {
    registers [t5]
    params [r0]
    result t4
}
function f14 "okkama.test.is#14" intrinsic "okkama.test.is" [] [] {
    registers [t2, t2]
    params [r0, r1]
    result void
}
function f15 "okkama.test.is#15" intrinsic "okkama.test.is" [] [] {
    registers [t4, t4]
    params [r0, r1]
    result void
}
function f16 "okkama.test.ok#16" intrinsic "okkama.test.ok" [] [] {
    registers [t3]
    params [r0]
    result void
}
tests {
    test "fake storage keeps values through GC and isolates receiver state" f7
}
