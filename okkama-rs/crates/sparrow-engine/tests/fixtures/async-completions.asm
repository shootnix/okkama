sparrow 9
module "async_completions" "v1.0.0"
type t0 role role1
type t1 int
type t2 future t1
type t3 bool
type t4 str
type t5 object obj1
type t6 object obj3
type t7 role role0
type t8 array t2
type t9 object obj0
type t10 future t3
type t11 failure
type t12 object obj2
type t13 future t2
type t14 object obj4
type t15 object obj5
type t16 object obj6
type t17 object obj7
role role0 "collections.sequence::Sequencing<Future<int>>" {
    method "cursor" [] -> t0
    method "size" [] -> t1
    method "item" [t1] -> t2
    method "first" [] -> t2
    method "last" [] -> t2
}
role role1 "collections.iteration::Cursor<Future<int>>" {
    method "available" [] -> t3
    method "item" [] -> t2
    method "successor" [] -> t0
}
role role2 "collections.iteration::Iterating<Future<int>>" {
    method "cursor" [] -> t0
}
role role3 "size::Sizing" {
    method "size" [] -> t1
}
role role4 "Error" builtin {
    method "text" [] -> t4
}
object obj0 "Job" {
    field "value" t1
    field "delay" t4
    field "done" t5
}
object obj1 "async.event::Event" {
    field "initial" t3
}
object obj2 "Read" {
    field "stream" t6
}
object obj3 "async.collect::FutureCompletions<int>" {
    field "origins" t7
}
object obj4 "core::ArraySequence<Future<int>, Future<int>>" {
    field "items" t8
    does role0 [f9, f10, f11, f12, f13]
    does role2 [f9]
    does role3 [f10]
}
object obj5 "async.collect::NoFutures" {
    does role4 [f18]
}
object obj6 "Cancelled" builtin {
    does role4 [f19]
}
object obj7 "core::ArrayCursor<Future<int>, Future<int>>" {
    field "items" t8
    field "at" t1
    does role1 [f20, f21, f22]
}
function f0 "Job.new#0" {
    registers [t1, t4, t5, t9]
    params [r0, r1, r2]
    result t9
    0: obj.new r3, obj0, [r0, r1, r2]
    1: ret r3
}
function f1 "Job.run" async {
    registers [t9, t4, t10, t3, t1, t5, t11, t11, t11]
    params [r0]
    result t1
    handler 0 5 @9 r6
    handler 9 12 @12 r7
    0: obj.get r1, r0, 1
    1: call r2, f5, [r1]
    2: await r3, r2
    3: obj.get r4, r0, 0
    4: jmp @5
    5: obj.get r5, r0, 2
    6: call _, f6, [r5]
    7: jmp @8
    8: ret r4
    9: obj.get r5, r0, 2
    10: call _, f6, [r5]
    11: jmp @14
    12: fail.secondary r8, r6, r7
    13: jmp @14(r6 <- r8)
    14: raise r6
}
function f2 "Read.new#0" {
    registers [t6, t12]
    params [r0]
    result t12
    0: obj.new r1, obj2, [r0]
    1: ret r1
}
function f3 "Read.run" async {
    registers [t12, t6, t13, t2, t1]
    params [r0]
    result t1
    0: obj.get r1, r0, 0
    1: call r2, f7, [r1]
    2: await r3, r2
    3: await r4, r3
    4: ret r4
}
function f4 "main" async {
    registers [t5, t1, t4, t9, t2, t2, t8, t14, t7, t6, t1, t3, t3, t13, t2, t11, t15, t5, t12, t16, t11, t11]
    params []
    result t1
    handler 60 64 @64 r15
    handler 65 67 @133 r20
    handler 90 93 @93 r15
    handler 94 96 @129 r20
    handler 97 100 @100 r15
    handler 101 103 @126 r20
    0: call r0, f8, []
    1: const r1, int 11
    2: const r2, str "1000000"
    3: call r3, f0, [r1, r2, r0]
    4: call r4, f1, [r3]
    5: const r1, int 31
    6: const r2, str "0"
    7: call r3, f0, [r1, r2, r0]
    8: call r5, f1, [r3]
    9: arr.new r6, [r4, r5, r5]
    10: call r7, f14, [r6]
    11: role.wrap r8, r7, role0
    12: call r9, f15, [r8]
    13: call r1, f16, [r9]
    14: const r10, int 3
    15: int.cmp eq, r11, r1, r10
    16: br r11, @17, @20
    17: call r11, f17, [r0]
    18: bool.not r12, r11
    19: jmp @22
    20: const r11, bool false
    21: jmp @22(r12 <- r11)
    22: br r12, @23, @140
    23: call r13, f7, [r9]
    24: await r14, r13
    25: val.cmp eq, r11, r14, r5
    26: br r11, @27, @31
    27: await r1, r14
    28: const r10, int 31
    29: int.cmp eq, r11, r1, r10
    30: jmp @33
    31: const r11, bool false
    32: jmp @33
    33: br r11, @34, @139
    34: call r13, f7, [r9]
    35: await r14, r13
    36: val.cmp eq, r11, r14, r5
    37: br r11, @38, @42
    38: await r1, r14
    39: const r10, int 31
    40: int.cmp eq, r11, r1, r10
    41: jmp @44
    42: const r11, bool false
    43: jmp @44
    44: br r11, @45, @138
    45: call r13, f7, [r9]
    46: await r5, r13
    47: val.cmp eq, r11, r5, r4
    48: br r11, @49, @53
    49: await r1, r5
    50: const r10, int 11
    51: int.cmp eq, r11, r1, r10
    52: jmp @55
    53: const r11, bool false
    54: jmp @55
    55: br r11, @56, @137
    56: call r1, f16, [r9]
    57: const r10, int 0
    58: int.cmp eq, r11, r1, r10
    59: br r11, @60, @136
    60: call r13, f7, [r9]
    61: await r4, r13
    62: const r11, bool false
    63: jmp @67
    64: catch r15, object, 5, r16, @65, @135
    65: const r11, bool true
    66: jmp @67
    67: br r11, @68, @132
    68: call r0, f8, []
    69: call r17, f8, []
    70: const r1, int 10
    71: const r2, str "9223372036854775807"
    72: call r3, f0, [r1, r2, r0]
    73: call r4, f1, [r3]
    74: const r1, int 20
    75: const r2, str "9223372036854775807"
    76: call r3, f0, [r1, r2, r17]
    77: call r5, f1, [r3]
    78: arr.new r6, [r4, r5]
    79: call r7, f14, [r6]
    80: role.wrap r8, r7, role0
    81: call r9, f15, [r8]
    82: call r18, f2, [r9]
    83: call r4, f3, [r18]
    84: fut.spawn r5, r4
    85: call r18, f2, [r9]
    86: call r4, f3, [r18]
    87: fut.spawn r14, r4
    88: fut cancel, _, r5
    89: jmp @90
    90: await r1, r5
    91: const r11, bool false
    92: jmp @96
    93: catch r15, object, 6, r19, @94, @131
    94: const r11, bool true
    95: jmp @96
    96: jmp @97
    97: await r1, r14
    98: const r12, bool false
    99: jmp @103
    100: catch r15, object, 6, r19, @101, @128
    101: const r12, bool true
    102: jmp @103
    103: br r11, @104, @105
    104: jmp @107
    105: const r11, bool false
    106: jmp @107(r12 <- r11)
    107: br r12, @108, @110
    108: call r11, f17, [r0]
    109: jmp @112
    110: const r11, bool false
    111: jmp @112
    112: br r11, @113, @115
    113: call r11, f17, [r17]
    114: jmp @117
    115: const r11, bool false
    116: jmp @117
    117: br r11, @118, @125
    118: call r1, f16, [r9]
    119: const r10, int 0
    120: int.cmp eq, r11, r1, r10
    121: br r11, @122, @124
    122: const r1, int 42
    123: ret r1
    124: trap confirm, "confirm failed"
    125: trap confirm, "confirm failed"
    126: fail.caused r21, r20, r15
    127: raise r21
    128: raise r15
    129: fail.caused r21, r20, r15
    130: raise r21
    131: raise r15
    132: trap confirm, "confirm failed"
    133: fail.caused r21, r20, r15
    134: raise r21
    135: raise r15
    136: trap confirm, "confirm failed"
    137: trap confirm, "confirm failed"
    138: trap confirm, "confirm failed"
    139: trap confirm, "confirm failed"
    140: trap confirm, "confirm failed"
}
function f5 "runtime.time::sleep_nanoseconds" async native "runtime/time@1::sleep_nanoseconds/1" {
    registers [t4]
    params [r0]
    result t3
}
function f6 "async.event::Event.set" intrinsic "okkama.event.Event.set" [] [] {
    registers [t5]
    params [r0]
    result void
}
function f7 "async.collect::FutureCompletions<int>.completion" async intrinsic "okkama.collect.FutureCompletions.completion" [obj5] [] {
    registers [t6]
    params [r0]
    result t2
}
function f8 "async.event::Event.new#0" intrinsic "okkama.event.Event.new" [obj1] [] {
    registers []
    params []
    result t5
}
function f9 "core::ArraySequence<Future<int>, Future<int>>.cursor" {
    registers [t14, t8, t1, t17, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 0
    1: const r2, int 0
    2: call r3, f23, [r1, r2]
    3: role.wrap r4, r3, role1
    4: ret r4
}
function f10 "core::ArraySequence<Future<int>, Future<int>>.size" {
    registers [t14, t8, t1]
    params [r0]
    result t1
    0: obj.get r1, r0, 0
    1: arr.len r2, r1
    2: ret r2
}
function f11 "core::ArraySequence<Future<int>, Future<int>>.item" {
    registers [t14, t1, t8, t2]
    params [r0, r1]
    result t2
    0: obj.get r2, r0, 0
    1: arr.get r3, r2, r1
    2: ret r3
}
function f12 "core::ArraySequence<Future<int>, Future<int>>.first" {
    registers [t14, t8, t1, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: const r2, int 0
    2: arr.lookup r1, r2, r3, @3, @4
    3: ret r3
    4: trap empty, "Cannot read first item of an empty sequence"
}
function f13 "core::ArraySequence<Future<int>, Future<int>>.last" {
    registers [t14, t8, t1, t1, t1, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: arr.len r2, r1
    2: const r3, int 1
    3: int.sub r4, r2, r3
    4: arr.lookup r1, r4, r5, @5, @6
    5: ret r5
    6: trap empty, "Cannot read last item of an empty sequence"
}
function f14 "core::ArraySequence<Future<int>, Future<int>>.new" {
    registers [t8, t14]
    params [r0]
    result t14
    0: obj.new r1, obj4, [r0]
    1: ret r1
}
function f15 "async.collect::FutureCompletions<int>.new#0" {
    registers [t7, t6]
    params [r0]
    result t6
    0: obj.new r1, obj3, [r0]
    1: ret r1
}
function f16 "async.collect::FutureCompletions<int>.remaining_count" intrinsic "okkama.collect.FutureCompletions.remaining_count" [] [] {
    registers [t6]
    params [r0]
    result t1
}
function f17 "async.event::Event.ready" intrinsic "okkama.event.Event.ready" [] [] {
    registers [t5]
    params [r0]
    result t3
}
function f18 "async.collect::NoFutures.text" {
    registers [t15, t4]
    params [r0]
    result t4
    0: const r1, str "no futures remain"
    1: ret r1
}
function f19 "Cancelled.text" {
    registers [t16, t4]
    params [r0]
    result t4
    0: const r1, str "Operation was cancelled"
    1: ret r1
}
function f20 "core::ArrayCursor<Future<int>, Future<int>>.available" {
    registers [t17, t8, t1, t1, t3]
    params [r0]
    result t3
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.len r3, r1
    3: int.cmp lt, r4, r2, r3
    4: ret r4
}
function f21 "core::ArrayCursor<Future<int>, Future<int>>.item" {
    registers [t17, t8, t1, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.lookup r1, r2, r3, @3, @4
    3: ret r3
    4: trap empty, "Array cursor is exhausted"
}
function f22 "core::ArrayCursor<Future<int>, Future<int>>.successor" {
    registers [t17, t8, t1, t2, t1, t1, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.lookup r1, r2, r3, @3, @8
    3: const r4, int 1
    4: int.add r5, r2, r4
    5: call r0, f23, [r1, r5]
    6: role.wrap r6, r0, role1
    7: ret r6
    8: trap empty, "Array cursor is exhausted"
}
function f23 "core::ArrayCursor<Future<int>, Future<int>>.new" {
    registers [t8, t1, t17]
    params [r0, r1]
    result t17
    0: obj.new r2, obj7, [r0, r1]
    1: ret r2
}
entry f4
