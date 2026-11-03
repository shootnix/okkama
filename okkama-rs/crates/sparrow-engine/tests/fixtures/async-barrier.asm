sparrow 9
module "async_barrier" "v1.0.0"
type t0 str
type t1 object obj1
type t2 int
type t3 object obj0
type t4 future t2
type t5 object obj2
type t6 bool
type t7 failure
type t8 object obj3
type t9 object obj4
type t10 object obj5
role role0 "Error" builtin {
    method "text" [] -> t0
}
object obj0 "Job" {
    field "barrier" t1
}
object obj1 "async.barrier::Barrier" {
    field "amount" t2
}
object obj2 "async.barrier::BarrierState" {
}
object obj3 "Cancelled" builtin {
    does role0 [f10]
}
object obj4 "async.barrier::BrokenBarrier" {
    does role0 [f11]
}
object obj5 "async.barrier::InvalidPartyCount" {
    field "amount" t2
    does role0 [f14]
}
function f0 "Job.new#0" {
    registers [t1, t3]
    params [r0]
    result t3
    0: obj.new r1, obj0, [r0]
    1: ret r1
}
function f1 "Job.run" async {
    registers [t3, t1, t4, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: call r2, f3, [r1]
    2: await r3, r2
    3: ret r3
}
function f2 "main" async {
    registers [t2, t1, t3, t4, t4, t4, t5, t2, t6, t6, t5, t7, t8, t9, t10, t7, t7]
    params []
    result t2
    handler 81 84 @84 r11
    handler 85 87 @226 r15
    handler 88 91 @91 r11
    handler 92 94 @223 r15
    handler 105 109 @109 r11
    handler 110 112 @219 r15
    handler 131 134 @134 r11
    handler 135 137 @214 r15
    handler 194 198 @198 r11
    handler 199 201 @205 r15
    0: const r0, int 3
    1: call r1, f4, [r0]
    2: call r2, f0, [r1]
    3: call r3, f1, [r2]
    4: fut.spawn r4, r3
    5: call r2, f0, [r1]
    6: call r3, f1, [r2]
    7: fut.spawn r5, r3
    8: call r6, f5, [r1]
    9: call r0, f6, [r6]
    10: const r7, int 3
    11: int.cmp eq, r8, r0, r7
    12: br r8, @13, @17
    13: call r0, f7, [r6]
    14: const r7, int 2
    15: int.cmp eq, r8, r0, r7
    16: jmp @19
    17: const r8, bool false
    18: jmp @19
    19: br r8, @20, @24
    20: call r0, f8, [r6]
    21: const r7, int 0
    22: int.cmp eq, r8, r0, r7
    23: jmp @26
    24: const r8, bool false
    25: jmp @26
    26: br r8, @27, @30
    27: call r8, f9, [r6]
    28: bool.not r9, r8
    29: jmp @32
    30: const r8, bool false
    31: jmp @32(r9 <- r8)
    32: br r9, @33, @233
    33: call r3, f3, [r1]
    34: await r0, r3
    35: const r7, int 2
    36: int.cmp eq, r8, r0, r7
    37: br r8, @38, @232
    38: await r0, r4
    39: const r7, int 0
    40: int.cmp eq, r8, r0, r7
    41: br r8, @42, @46
    42: await r0, r5
    43: const r7, int 1
    44: int.cmp eq, r8, r0, r7
    45: jmp @48
    46: const r8, bool false
    47: jmp @48
    48: br r8, @49, @231
    49: call r10, f5, [r1]
    50: call r0, f8, [r10]
    51: const r7, int 1
    52: int.cmp eq, r8, r0, r7
    53: br r8, @54, @59
    54: call r10, f5, [r1]
    55: call r0, f7, [r10]
    56: const r7, int 0
    57: int.cmp eq, r8, r0, r7
    58: jmp @61
    59: const r8, bool false
    60: jmp @61
    61: br r8, @62, @230
    62: call r0, f7, [r6]
    63: const r7, int 2
    64: int.cmp eq, r8, r0, r7
    65: br r8, @66, @70
    66: call r0, f8, [r6]
    67: const r7, int 0
    68: int.cmp eq, r8, r0, r7
    69: jmp @72
    70: const r8, bool false
    71: jmp @72
    72: br r8, @73, @229
    73: call r2, f0, [r1]
    74: call r3, f1, [r2]
    75: fut.spawn r4, r3
    76: call r2, f0, [r1]
    77: call r3, f1, [r2]
    78: fut.spawn r5, r3
    79: fut cancel, _, r4
    80: jmp @81
    81: await r0, r4
    82: const r8, bool false
    83: jmp @87
    84: catch r11, object, 3, r12, @85, @228
    85: const r8, bool true
    86: jmp @87
    87: jmp @88
    88: await r0, r5
    89: const r9, bool false
    90: jmp @94
    91: catch r11, object, 4, r13, @92, @225
    92: const r9, bool true
    93: jmp @94
    94: br r8, @95, @96
    95: jmp @98
    96: const r8, bool false
    97: jmp @98(r9 <- r8)
    98: br r9, @99, @102
    99: call r6, f5, [r1]
    100: call r8, f9, [r6]
    101: jmp @104
    102: const r8, bool false
    103: jmp @104
    104: br r8, @105, @222
    105: call r3, f3, [r1]
    106: await r0, r3
    107: const r8, bool false
    108: jmp @112
    109: catch r11, object, 4, r13, @110, @221
    110: const r8, bool true
    111: jmp @112
    112: br r8, @113, @218
    113: call _, f12, [r1]
    114: call r6, f5, [r1]
    115: call r8, f9, [r6]
    116: bool.not r9, r8
    117: br r9, @118, @123
    118: call r6, f5, [r1]
    119: call r0, f8, [r6]
    120: const r7, int 2
    121: int.cmp eq, r8, r0, r7
    122: jmp @125
    123: const r8, bool false
    124: jmp @125
    125: br r8, @126, @217
    126: call r2, f0, [r1]
    127: call r3, f1, [r2]
    128: fut.spawn r4, r3
    129: call _, f12, [r1]
    130: jmp @131
    131: await r0, r4
    132: const r8, bool false
    133: jmp @137
    134: catch r11, object, 4, r13, @135, @216
    135: const r8, bool true
    136: jmp @137
    137: br r8, @138, @143
    138: call r6, f5, [r1]
    139: call r0, f8, [r6]
    140: const r7, int 3
    141: int.cmp eq, r8, r0, r7
    142: jmp @145
    143: const r8, bool false
    144: jmp @145
    145: br r8, @146, @213
    146: call r2, f0, [r1]
    147: call r3, f1, [r2]
    148: fut.spawn r4, r3
    149: call r2, f0, [r1]
    150: call r3, f1, [r2]
    151: fut.spawn r5, r3
    152: call r3, f3, [r1]
    153: await r0, r3
    154: const r7, int 2
    155: int.cmp eq, r8, r0, r7
    156: br r8, @157, @212
    157: await r0, r4
    158: const r7, int 0
    159: int.cmp eq, r8, r0, r7
    160: br r8, @161, @165
    161: await r0, r5
    162: const r7, int 1
    163: int.cmp eq, r8, r0, r7
    164: jmp @167
    165: const r8, bool false
    166: jmp @167
    167: br r8, @168, @211
    168: call _, f13, [r1]
    169: call _, f13, [r1]
    170: call r6, f5, [r1]
    171: call r8, f9, [r6]
    172: br r8, @173, @210
    173: call _, f12, [r1]
    174: const r0, int 1
    175: call r1, f4, [r0]
    176: call r3, f3, [r1]
    177: await r0, r3
    178: const r7, int 0
    179: int.cmp eq, r8, r0, r7
    180: br r8, @181, @186
    181: call r3, f3, [r1]
    182: await r0, r3
    183: const r7, int 0
    184: int.cmp eq, r8, r0, r7
    185: jmp @188
    186: const r8, bool false
    187: jmp @188
    188: br r8, @189, @209
    189: call r6, f5, [r1]
    190: call r0, f8, [r6]
    191: const r7, int 2
    192: int.cmp eq, r8, r0, r7
    193: br r8, @194, @208
    194: const r0, int 0
    195: call r1, f4, [r0]
    196: const r8, bool false
    197: jmp @201
    198: catch r11, object, 5, r14, @199, @207
    199: const r8, bool true
    200: jmp @201
    201: br r8, @202, @204
    202: const r0, int 42
    203: ret r0
    204: trap confirm, "confirm failed"
    205: fail.caused r16, r15, r11
    206: raise r16
    207: raise r11
    208: trap confirm, "confirm failed"
    209: trap confirm, "confirm failed"
    210: trap confirm, "confirm failed"
    211: trap confirm, "confirm failed"
    212: trap confirm, "confirm failed"
    213: trap confirm, "confirm failed"
    214: fail.caused r16, r15, r11
    215: raise r16
    216: raise r11
    217: trap confirm, "confirm failed"
    218: trap confirm, "confirm failed"
    219: fail.caused r16, r15, r11
    220: raise r16
    221: raise r11
    222: trap confirm, "confirm failed"
    223: fail.caused r16, r15, r11
    224: raise r16
    225: raise r11
    226: fail.caused r16, r15, r11
    227: raise r16
    228: raise r11
    229: trap confirm, "confirm failed"
    230: trap confirm, "confirm failed"
    231: trap confirm, "confirm failed"
    232: trap confirm, "confirm failed"
    233: trap confirm, "confirm failed"
}
function f3 "async.barrier::Barrier.wait" async intrinsic "okkama.barrier.Barrier.wait" [obj4] [] {
    registers [t1]
    params [r0]
    result t2
}
function f4 "async.barrier::Barrier.new#0" {
    registers [t2, t2, t6, t1, t10]
    params [r0]
    result t1
    0: const r1, int 0
    1: int.cmp gt, r2, r0, r1
    2: br r2, @3, @5
    3: obj.new r3, obj1, [r0]
    4: ret r3
    5: call r4, f15, [r0]
    6: throw r4
}
function f5 "async.barrier::Barrier.state" intrinsic "okkama.barrier.Barrier.state" [obj2] [] {
    registers [t1]
    params [r0]
    result t5
}
function f6 "async.barrier::BarrierState.parties" intrinsic "okkama.barrier.BarrierState.parties" [] [] {
    registers [t5]
    params [r0]
    result t2
}
function f7 "async.barrier::BarrierState.waiters" intrinsic "okkama.barrier.BarrierState.waiters" [] [] {
    registers [t5]
    params [r0]
    result t2
}
function f8 "async.barrier::BarrierState.generation" intrinsic "okkama.barrier.BarrierState.generation" [] [] {
    registers [t5]
    params [r0]
    result t2
}
function f9 "async.barrier::BarrierState.broken" intrinsic "okkama.barrier.BarrierState.broken" [] [] {
    registers [t5]
    params [r0]
    result t6
}
function f10 "Cancelled.text" {
    registers [t8, t0]
    params [r0]
    result t0
    0: const r1, str "Operation was cancelled"
    1: ret r1
}
function f11 "async.barrier::BrokenBarrier.text" {
    registers [t9, t0]
    params [r0]
    result t0
    0: const r1, str "barrier is broken"
    1: ret r1
}
function f12 "async.barrier::Barrier.reset" intrinsic "okkama.barrier.Barrier.reset" [] [] {
    registers [t1]
    params [r0]
    result void
}
function f13 "async.barrier::Barrier.abort" intrinsic "okkama.barrier.Barrier.abort" [] [] {
    registers [t1]
    params [r0]
    result void
}
function f14 "async.barrier::InvalidPartyCount.text" {
    registers [t10, t0]
    params [r0]
    result t0
    0: const r1, str "barrier party count must be positive"
    1: ret r1
}
function f15 "async.barrier::InvalidPartyCount.new#0" {
    registers [t2, t10]
    params [r0]
    result t10
    0: obj.new r1, obj5, [r0]
    1: ret r1
}
entry f2
