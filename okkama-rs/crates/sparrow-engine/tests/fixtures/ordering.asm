sparrow 9
module "ordering_fixture" "v1.0.0"
type t0 str
type t1 object obj1
type t2 enum enum0
type t3 object obj2
type t4 int
type t5 object obj0
type t6 bool
type t7 builder t1
type t8 array t1
type t9 array t3
type t10 failure
type t11 array t4
enum enum0 "compare::Order" ["less", "equal", "greater"]
role role0 "Error" builtin {
    method "text" [] -> t0
}
role role1 "compare::Comparing<Item>" {
    method "order" [t1] -> t2
}
role role2 "compare::Comparing<Bad>" {
    method "order" [t3] -> t2
}
object obj0 "Problem" {
    does role0 [f0]
}
object obj1 "Item" {
    field "key" t4
    field "serial" t4
    does role1 [f2]
}
object obj2 "Bad" {
    does role2 [f6]
}
function f0 "Problem.text" {
    registers [t5, t0]
    params [r0]
    result t0
    0: const r1, str "comparison failed"
    1: ret r1
}
function f1 "Problem.new#0" {
    registers [t5]
    params []
    result t5
    0: obj.new r0, obj0, []
    1: ret r0
}
function f2 "Item.order" {
    registers [t1, t1, t4, t4, t6, t2]
    params [r0, r1]
    result t2
    0: obj.get r2, r0, 0
    1: call r3, f4, [r1]
    2: int.cmp lt, r4, r2, r3
    3: br r4, @4, @6
    4: enum.case r5, enum0, 0
    5: jmp @12
    6: int.cmp eq, r4, r2, r3
    7: br r4, @8, @10
    8: enum.case r5, enum0, 1
    9: jmp @12
    10: enum.case r5, enum0, 2
    11: jmp @12
    12: ret r5
}
function f3 "Item.new#0" {
    registers [t4, t4, t1]
    params [r0, r1]
    result t1
    0: obj.new r2, obj1, [r0, r1]
    1: ret r2
}
function f4 "Item.value" {
    registers [t1, t4]
    params [r0]
    result t4
    0: obj.get r1, r0, 0
    1: ret r1
}
function f5 "Item.identity" {
    registers [t1, t4]
    params [r0]
    result t4
    0: obj.get r1, r0, 1
    1: ret r1
}
function f6 "Bad.order" {
    registers [t3, t3, t5]
    params [r0, r1]
    result t2
    0: call r2, f1, []
    1: throw r2
}
function f7 "Bad.new#0" {
    registers [t3]
    params []
    result t3
    0: obj.new r0, obj2, []
    1: ret r0
}
function f8 "main" {
    registers [t4, t4, t6, t7, t4, t4, t1, t7, t4, t8, t8, t4, t4, t1, t1, t1, t3, t3, t9, t9, t10, t5, t11, t11, t10, t10]
    params []
    result t4
    handler 108 114 @114 r20
    handler 115 117 @156 r24
    0: const r0, int 0
    1: const r1, int 201
    2: int.cmp le, r2, r0, r1
    3: const r4, int 200
    4: const r8, int 1
    5: const r11, int 1
    6: const r12, int 1
    7: br r2, @8, @162
    8: builder.new r3
    9: jmp @10
    10: int.cmp lt, r2, r0, r1
    11: br r2, @12, @18
    12: int.sub r5, r4, r0
    13: call r6, f3, [r5, r0]
    14: builder.push r7, r3, r6
    15: jmp @16
    16: int.add r5, r0, r8
    17: jmp @10(r0 <- r5, r3 <- r7)
    18: builder.finish r9, r3
    19: call r10, f9, [r9]
    20: const r0, int 0
    21: arr.get r6, r9, r0
    22: call r0, f4, [r6]
    23: const r1, int 200
    24: int.cmp eq, r2, r0, r1
    25: br r2, @26, @32
    26: const r0, int 0
    27: arr.get r6, r10, r0
    28: call r0, f4, [r6]
    29: const r1, int 0
    30: int.cmp eq, r2, r0, r1
    31: jmp @34
    32: const r2, bool false
    33: jmp @34
    34: br r2, @35, @41
    35: const r0, int 200
    36: arr.get r6, r10, r0
    37: call r0, f4, [r6]
    38: const r1, int 200
    39: int.cmp eq, r2, r0, r1
    40: jmp @43
    41: const r2, bool false
    42: jmp @43
    43: br r2, @44, @161
    44: const r0, int 1
    45: const r1, int 201
    46: int.cmp le, r2, r0, r1
    47: br r2, @48, @160
    48: jmp @49
    49: int.cmp lt, r2, r0, r1
    50: br r2, @51, @61
    51: int.sub r4, r0, r11
    52: arr.get r6, r10, r4
    53: call r4, f4, [r6]
    54: arr.get r6, r10, r0
    55: call r5, f4, [r6]
    56: int.cmp lt, r2, r4, r5
    57: br r2, @58, @60
    58: int.add r4, r0, r12
    59: jmp @49(r0 <- r4)
    60: trap confirm, "confirm failed"
    61: const r0, int 2
    62: const r1, int 0
    63: call r6, f3, [r0, r1]
    64: const r0, int 1
    65: const r1, int 1
    66: call r13, f3, [r0, r1]
    67: const r0, int 2
    68: const r1, int 2
    69: call r14, f3, [r0, r1]
    70: const r0, int 1
    71: const r1, int 3
    72: call r15, f3, [r0, r1]
    73: arr.new r9, [r6, r13, r14, r15]
    74: call r10, f9, [r9]
    75: const r0, int 0
    76: arr.get r6, r10, r0
    77: call r0, f5, [r6]
    78: const r1, int 1
    79: int.cmp eq, r2, r0, r1
    80: br r2, @81, @87
    81: const r0, int 1
    82: arr.get r6, r10, r0
    83: call r0, f5, [r6]
    84: const r1, int 3
    85: int.cmp eq, r2, r0, r1
    86: jmp @89
    87: const r2, bool false
    88: jmp @89
    89: br r2, @90, @96
    90: const r0, int 2
    91: arr.get r6, r10, r0
    92: call r0, f5, [r6]
    93: const r1, int 0
    94: int.cmp eq, r2, r0, r1
    95: jmp @98
    96: const r2, bool false
    97: jmp @98
    98: br r2, @99, @105
    99: const r0, int 3
    100: arr.get r6, r10, r0
    101: call r0, f5, [r6]
    102: const r1, int 2
    103: int.cmp eq, r2, r0, r1
    104: jmp @107
    105: const r2, bool false
    106: jmp @107
    107: br r2, @108, @159
    108: call r16, f7, []
    109: call r17, f7, []
    110: arr.new r18, [r16, r17]
    111: call r19, f10, [r18]
    112: const r2, bool false
    113: jmp @117
    114: catch r20, object, 0, r21, @115, @158
    115: const r2, bool true
    116: jmp @117
    117: br r2, @118, @155
    118: const r0, int 3
    119: const r1, int -1
    120: const r4, int 3
    121: const r5, int 0
    122: const r8, int 9223372036854775808
    123: arr.new r22, [r0, r1, r4, r5, r8]
    124: call r23, f11, [r22]
    125: const r0, int -1
    126: const r1, int 0
    127: const r4, int 3
    128: const r5, int 3
    129: const r8, int 9223372036854775808
    130: arr.new r22, [r0, r1, r4, r5, r8]
    131: val.cmp eq, r2, r23, r22
    132: br r2, @133, @154
    133: arr.new r9, []
    134: call r10, f9, [r9]
    135: arr.len r0, r10
    136: const r1, int 0
    137: int.cmp eq, r2, r0, r1
    138: br r2, @139, @153
    139: const r0, int 7
    140: const r1, int 9
    141: call r6, f3, [r0, r1]
    142: arr.new r9, [r6]
    143: call r10, f9, [r9]
    144: const r0, int 0
    145: arr.get r6, r10, r0
    146: call r0, f5, [r6]
    147: const r1, int 9
    148: int.cmp eq, r2, r0, r1
    149: br r2, @150, @152
    150: const r0, int 42
    151: ret r0
    152: trap confirm, "confirm failed"
    153: trap confirm, "confirm failed"
    154: trap confirm, "confirm failed"
    155: trap confirm, "confirm failed"
    156: fail.caused r25, r24, r20
    157: raise r25
    158: raise r20
    159: trap confirm, "confirm failed"
    160: trap range, "Range start must be less than or equal to range end"
    161: trap confirm, "confirm failed"
    162: trap range, "Range start must be less than or equal to range end"
}
function f9 "okkama.array.ordering#9" intrinsic "okkama.array.ordering" [] [] {
    registers [t8]
    params [r0]
    result t8
}
function f10 "okkama.array.ordering#10" intrinsic "okkama.array.ordering" [] [] {
    registers [t9]
    params [r0]
    result t9
}
function f11 "okkama.array.ordering#11" intrinsic "okkama.array.ordering" [] [] {
    registers [t11]
    params [r0]
    result t11
}
entry f8
