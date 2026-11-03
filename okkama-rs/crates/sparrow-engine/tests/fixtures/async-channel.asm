sparrow 9
module "async_channel" "v1.0.0"
type t0 str
type t1 object obj1
type t2 int
type t3 object obj3
type t4 object obj0
type t5 future t1
type t6 object obj2
type t7 future t2
type t8 object obj4
type t9 bool
type t10 failure
type t11 object obj5
type t12 object obj6
type t13 object obj7
role role0 "Error" builtin {
    method "text" [] -> t0
}
object obj0 "Sender" {
    field "output" t1
    field "value" t2
}
object obj1 "async.channel::Sending<int>" {
}
object obj2 "Receiver" {
    field "input" t3
}
object obj3 "async.channel::Receiving<int>" {
}
object obj4 "async.channel::Channel<int>" {
}
object obj5 "Cancelled" builtin {
    does role0 [f10]
}
object obj6 "async.channel::ChannelClosed" {
    does role0 [f12]
}
object obj7 "async.channel::InvalidChannelCapacity" {
    field "capacity" t2
    does role0 [f13]
}
function f0 "Sender.new#0" {
    registers [t1, t2, t4]
    params [r0, r1]
    result t4
    0: obj.new r2, obj0, [r0, r1]
    1: ret r2
}
function f1 "Sender.run" async {
    registers [t4, t1, t2, t5]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: call r3, f5, [r1, r2]
    3: await r1, r3
    4: obj.get r2, r0, 1
    5: ret r2
}
function f2 "Receiver.new#0" {
    registers [t3, t6]
    params [r0]
    result t6
    0: obj.new r1, obj2, [r0]
    1: ret r1
}
function f3 "Receiver.run" async {
    registers [t6, t3, t7, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: call r2, f6, [r1]
    2: await r3, r2
    3: ret r3
}
function f4 "main" async {
    registers [t2, t8, t1, t3, t5, t1, t4, t7, t7, t7, t7, t9, t9, t10, t11, t2, t6, t12, t10, t10]
    params []
    result t2
    handler 30 33 @33 r13
    handler 34 36 @140 r18
    handler 84 87 @87 r13
    handler 88 90 @129 r18
    handler 97 100 @100 r13
    handler 101 103 @125 r18
    0: const r0, int 1
    1: call r1, f7, [r0]
    2: call r2, f8, [r1]
    3: call r3, f9, [r1]
    4: const r0, int 10
    5: call r4, f5, [r2, r0]
    6: await r5, r4
    7: const r0, int 20
    8: call r6, f0, [r2, r0]
    9: call r7, f1, [r6]
    10: fut.spawn r8, r7
    11: const r0, int 99
    12: call r6, f0, [r2, r0]
    13: call r7, f1, [r6]
    14: fut.spawn r9, r7
    15: const r0, int 30
    16: call r6, f0, [r2, r0]
    17: call r7, f1, [r6]
    18: fut.spawn r10, r7
    19: fut done, r11, r8
    20: bool.not r12, r11
    21: br r12, @22, @25
    22: fut done, r11, r10
    23: bool.not r12, r11
    24: jmp @27
    25: const r11, bool false
    26: jmp @27(r12 <- r11)
    27: br r12, @28, @143
    28: fut cancel, _, r9
    29: jmp @30
    30: await r0, r9
    31: const r11, bool false
    32: jmp @36
    33: catch r13, object, 5, r14, @34, @142
    34: const r11, bool true
    35: jmp @36
    36: br r11, @37, @139
    37: call r7, f6, [r3]
    38: await r0, r7
    39: const r15, int 10
    40: int.cmp eq, r11, r0, r15
    41: br r11, @42, @138
    42: call r7, f6, [r3]
    43: await r0, r7
    44: const r15, int 20
    45: int.cmp eq, r11, r0, r15
    46: br r11, @47, @137
    47: call r7, f6, [r3]
    48: await r0, r7
    49: const r15, int 30
    50: int.cmp eq, r11, r0, r15
    51: br r11, @52, @136
    52: await r0, r8
    53: const r15, int 20
    54: int.cmp eq, r11, r0, r15
    55: br r11, @56, @135
    56: await r0, r10
    57: const r15, int 30
    58: int.cmp eq, r11, r0, r15
    59: br r11, @60, @134
    60: call r16, f2, [r3]
    61: call r7, f3, [r16]
    62: fut.spawn r8, r7
    63: call r16, f2, [r3]
    64: call r7, f3, [r16]
    65: fut.spawn r9, r7
    66: call r16, f2, [r3]
    67: call r7, f3, [r16]
    68: fut.spawn r10, r7
    69: fut cancel, _, r9
    70: const r0, int 11
    71: call r4, f5, [r2, r0]
    72: await r5, r4
    73: const r0, int 31
    74: call r4, f5, [r2, r0]
    75: await r5, r4
    76: await r0, r8
    77: const r15, int 11
    78: int.cmp eq, r11, r0, r15
    79: br r11, @80, @133
    80: await r0, r10
    81: const r15, int 31
    82: int.cmp eq, r11, r0, r15
    83: br r11, @84, @132
    84: await r0, r9
    85: const r11, bool false
    86: jmp @90
    87: catch r13, object, 5, r14, @88, @131
    88: const r11, bool true
    89: jmp @90
    90: br r11, @91, @128
    91: call r16, f2, [r3]
    92: call r7, f3, [r16]
    93: fut.spawn r8, r7
    94: call _, f11, [r2]
    95: call _, f11, [r2]
    96: jmp @97
    97: await r0, r8
    98: const r11, bool false
    99: jmp @103
    100: catch r13, object, 6, r17, @101, @127
    101: const r11, bool true
    102: jmp @103
    103: br r11, @104, @124
    104: const r0, int 0
    105: call r1, f7, [r0]
    106: call r2, f8, [r1]
    107: const r0, int 42
    108: call r6, f0, [r2, r0]
    109: call r7, f1, [r6]
    110: fut.spawn r8, r7
    111: fut done, r11, r8
    112: bool.not r12, r11
    113: br r12, @114, @123
    114: call r3, f9, [r1]
    115: call r7, f6, [r3]
    116: await r0, r7
    117: const r15, int 42
    118: int.cmp eq, r11, r0, r15
    119: br r11, @120, @122
    120: await r0, r8
    121: ret r0
    122: trap confirm, "confirm failed"
    123: trap confirm, "confirm failed"
    124: trap confirm, "confirm failed"
    125: fail.caused r19, r18, r13
    126: raise r19
    127: raise r13
    128: trap confirm, "confirm failed"
    129: fail.caused r19, r18, r13
    130: raise r19
    131: raise r13
    132: trap confirm, "confirm failed"
    133: trap confirm, "confirm failed"
    134: trap confirm, "confirm failed"
    135: trap confirm, "confirm failed"
    136: trap confirm, "confirm failed"
    137: trap confirm, "confirm failed"
    138: trap confirm, "confirm failed"
    139: trap confirm, "confirm failed"
    140: fail.caused r19, r18, r13
    141: raise r19
    142: raise r13
    143: trap confirm, "confirm failed"
}
function f5 "async.channel::Sending<int>.send" async intrinsic "okkama.channel.Sending.send" [obj6] [] {
    registers [t1, t2]
    params [r0, r1]
    result t1
}
function f6 "async.channel::Receiving<int>.item" async intrinsic "okkama.channel.Receiving.item" [obj6] [] {
    registers [t3]
    params [r0]
    result t2
}
function f7 "async.channel::Channel<int>.new#0" intrinsic "okkama.channel.Channel.new_capacity" [obj4, obj7] [] {
    registers [t2]
    params [r0]
    result t8
}
function f8 "async.channel::Channel<int>.sending" intrinsic "okkama.channel.Channel.sending" [obj1] [] {
    registers [t8]
    params [r0]
    result t1
}
function f9 "async.channel::Channel<int>.receiving" intrinsic "okkama.channel.Channel.receiving" [obj3] [] {
    registers [t8]
    params [r0]
    result t3
}
function f10 "Cancelled.text" {
    registers [t11, t0]
    params [r0]
    result t0
    0: const r1, str "Operation was cancelled"
    1: ret r1
}
function f11 "async.channel::Sending<int>.close" intrinsic "okkama.channel.Sending.close" [obj6] [] {
    registers [t1]
    params [r0]
    result void
}
function f12 "async.channel::ChannelClosed.text" {
    registers [t12, t0]
    params [r0]
    result t0
    0: const r1, str "channel is closed"
    1: ret r1
}
function f13 "async.channel::InvalidChannelCapacity.text" {
    registers [t13, t0]
    params [r0]
    result t0
    0: const r1, str "channel capacity must not be negative"
    1: ret r1
}
entry f4
