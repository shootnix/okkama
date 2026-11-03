sparrow 9
module "async_collect" "v1.0.0"
type t0 str
type t1 role role2
type t2 int
type t3 future t2
type t4 bool
type t5 role role6
type t6 object obj1
type t7 role role1
type t8 array t3
type t9 array t2
type t10 object obj0
type t11 future t4
type t12 failure
type t13 object obj2
type t14 object obj3
type t15 object obj5
type t16 object obj4
type t17 role role5
type t18 future t17
type t19 object obj6
type t20 object obj7
type t21 object obj8
type t22 object obj9
type t23 object obj11
type t24 object obj10
role role0 "Error" builtin {
    method "text" [] -> t0
}
role role1 "collections.sequence::Sequencing<Future<int>>" {
    method "cursor" [] -> t1
    method "size" [] -> t2
    method "item" [t2] -> t3
    method "first" [] -> t3
    method "last" [] -> t3
}
role role2 "collections.iteration::Cursor<Future<int>>" {
    method "available" [] -> t4
    method "item" [] -> t3
    method "successor" [] -> t1
}
role role3 "collections.iteration::Iterating<Future<int>>" {
    method "cursor" [] -> t1
}
role role4 "size::Sizing" {
    method "size" [] -> t2
}
role role5 "collections.sequence::Sequencing<int>" {
    method "cursor" [] -> t5
    method "size" [] -> t2
    method "item" [t2] -> t2
    method "first" [] -> t2
    method "last" [] -> t2
}
role role6 "collections.iteration::Cursor<int>" {
    method "available" [] -> t4
    method "item" [] -> t2
    method "successor" [] -> t5
}
role role7 "collections.iteration::Iterating<int>" {
    method "cursor" [] -> t5
}
object obj0 "Job" {
    field "value" t2
    field "delay" t0
    field "done" t6
}
object obj1 "async.event::Event" {
    field "initial" t4
}
object obj2 "Problem" {
    does role0 [f2]
}
object obj3 "Broken" {
}
object obj4 "async.collect::AllFutures<int>" {
    field "origins" t7
}
object obj5 "core::ArraySequence<Future<int>, Future<int>>" {
    field "items" t8
    does role1 [f10, f11, f12, f13, f14]
    does role3 [f10]
    does role4 [f11]
}
object obj6 "async.collect::Raced<int>" {
    field "origins" t7
}
object obj7 "async.collect::FirstSuccessful<int>" {
    field "origins" t7
}
object obj8 "core::ArrayCursor<Future<int>, Future<int>>" {
    field "items" t8
    field "at" t2
    does role2 [f23, f24, f25]
}
object obj9 "core::ArraySequence<int, int>" {
    field "items" t9
    does role5 [f27, f28, f29, f30, f31]
    does role7 [f27]
    does role4 [f28]
}
object obj10 "async.collect::NoFutures" {
    does role0 [f32]
}
object obj11 "core::ArrayCursor<int, int>" {
    field "items" t9
    field "at" t2
    does role6 [f33, f34, f35]
}
function f0 "Job.new#0" {
    registers [t2, t0, t6, t10]
    params [r0, r1, r2]
    result t10
    0: obj.new r3, obj0, [r0, r1, r2]
    1: ret r3
}
function f1 "Job.run" async {
    registers [t10, t0, t11, t4, t2, t6, t12, t12, t12]
    params [r0]
    result t2
    handler 0 5 @9 r6
    handler 9 12 @12 r7
    0: obj.get r1, r0, 1
    1: call r2, f7, [r1]
    2: await r3, r2
    3: obj.get r4, r0, 0
    4: jmp @5
    5: obj.get r5, r0, 2
    6: call _, f8, [r5]
    7: jmp @8
    8: ret r4
    9: obj.get r5, r0, 2
    10: call _, f8, [r5]
    11: jmp @14
    12: fail.secondary r8, r6, r7
    13: jmp @14(r6 <- r8)
    14: raise r6
}
function f2 "Problem.text" {
    registers [t13, t0]
    params [r0]
    result t0
    0: const r1, str "attempt failed"
    1: ret r1
}
function f3 "Problem.new#0" {
    registers [t13]
    params []
    result t13
    0: obj.new r0, obj2, []
    1: ret r0
}
function f4 "Broken.new#0" {
    registers [t14]
    params []
    result t14
    0: obj.new r0, obj3, []
    1: ret r0
}
function f5 "Broken.run" async {
    registers [t14, t13]
    params [r0]
    result t2
    0: call r1, f3, []
    1: throw r1
}
function f6 "main" async {
    registers [t6, t2, t0, t10, t3, t3, t8, t15, t7, t16, t18, t18, t4, t17, t2, t6, t19, t3, t14, t20, t12, t13, t12, t12]
    params []
    result t2
    handler 121 125 @125 r20
    handler 126 128 @132 r22
    0: call r0, f9, []
    1: const r1, int 11
    2: const r2, str "1000000"
    3: call r3, f0, [r1, r2, r0]
    4: call r4, f1, [r3]
    5: const r1, int 31
    6: const r2, str "0"
    7: call r3, f0, [r1, r2, r0]
    8: call r5, f1, [r3]
    9: arr.new r6, [r4, r5, r4]
    10: call r7, f15, [r6]
    11: role.wrap r8, r7, role1
    12: call r9, f16, [r8]
    13: call r10, f17, [r9]
    14: call r11, f17, [r9]
    15: val.cmp eq, r12, r10, r11
    16: br r12, @17, @141
    17: await r13, r10
    18: call.role r1, r13, role5, 1, []
    19: const r14, int 3
    20: int.cmp eq, r12, r1, r14
    21: br r12, @22, @27
    22: const r1, int 0
    23: call.role r14, r13, role5, 2, [r1]
    24: const r1, int 11
    25: int.cmp eq, r12, r14, r1
    26: jmp @29
    27: const r12, bool false
    28: jmp @29
    29: br r12, @30, @35
    30: const r1, int 1
    31: call.role r14, r13, role5, 2, [r1]
    32: const r1, int 31
    33: int.cmp eq, r12, r14, r1
    34: jmp @37
    35: const r12, bool false
    36: jmp @37
    37: br r12, @38, @43
    38: const r1, int 2
    39: call.role r14, r13, role5, 2, [r1]
    40: const r1, int 11
    41: int.cmp eq, r12, r14, r1
    42: jmp @45
    43: const r12, bool false
    44: jmp @45
    45: br r12, @46, @140
    46: call r10, f17, [r9]
    47: await r13, r10
    48: const r1, int 0
    49: call.role r14, r13, role5, 2, [r1]
    50: const r1, int 11
    51: int.cmp eq, r12, r14, r1
    52: br r12, @53, @139
    53: arr.new r6, []
    54: call r7, f15, [r6]
    55: role.wrap r8, r7, role1
    56: call r9, f16, [r8]
    57: call r10, f17, [r9]
    58: await r13, r10
    59: call.role r1, r13, role5, 1, []
    60: const r14, int 0
    61: int.cmp eq, r12, r1, r14
    62: br r12, @63, @138
    63: call r15, f9, []
    64: const r1, int 99
    65: const r2, str "9223372036854775807"
    66: call r3, f0, [r1, r2, r15]
    67: call r4, f1, [r3]
    68: const r1, int 42
    69: const r2, str "1000000"
    70: call r3, f0, [r1, r2, r0]
    71: call r5, f1, [r3]
    72: arr.new r6, [r4, r5]
    73: call r7, f15, [r6]
    74: role.wrap r8, r7, role1
    75: call r16, f18, [r8]
    76: call r5, f19, [r16]
    77: call r17, f19, [r16]
    78: val.cmp eq, r12, r5, r17
    79: br r12, @80, @137
    80: call r5, f19, [r16]
    81: await r1, r5
    82: const r14, int 42
    83: int.cmp eq, r12, r1, r14
    84: br r12, @85, @87
    85: call r12, f20, [r15]
    86: jmp @89
    87: const r12, bool false
    88: jmp @89
    89: br r12, @90, @92
    90: fut cancelled, r12, r4
    91: jmp @94
    92: const r12, bool false
    93: jmp @94
    94: br r12, @95, @136
    95: call r18, f4, []
    96: call r4, f5, [r18]
    97: const r1, int 42
    98: const r2, str "1000000"
    99: call r3, f0, [r1, r2, r0]
    100: call r5, f1, [r3]
    101: arr.new r6, [r4, r5]
    102: call r7, f15, [r6]
    103: role.wrap r8, r7, role1
    104: call r19, f21, [r8]
    105: call r4, f22, [r19]
    106: await r1, r4
    107: const r14, int 42
    108: int.cmp eq, r12, r1, r14
    109: br r12, @110, @135
    110: call r18, f4, []
    111: call r4, f5, [r18]
    112: const r1, int 99
    113: const r2, str "1000000"
    114: call r3, f0, [r1, r2, r0]
    115: call r5, f1, [r3]
    116: arr.new r6, [r4, r5]
    117: call r7, f15, [r6]
    118: role.wrap r8, r7, role1
    119: call r16, f18, [r8]
    120: jmp @121
    121: call r4, f19, [r16]
    122: await r1, r4
    123: const r12, bool false
    124: jmp @128
    125: catch r20, object, 2, r21, @126, @134
    126: const r12, bool true
    127: jmp @128
    128: br r12, @129, @131
    129: const r1, int 42
    130: ret r1
    131: trap confirm, "confirm failed"
    132: fail.caused r23, r22, r20
    133: raise r23
    134: raise r20
    135: trap confirm, "confirm failed"
    136: trap confirm, "confirm failed"
    137: trap confirm, "confirm failed"
    138: trap confirm, "confirm failed"
    139: trap confirm, "confirm failed"
    140: trap confirm, "confirm failed"
    141: trap confirm, "confirm failed"
}
function f7 "runtime.time::sleep_nanoseconds" async native "runtime/time@1::sleep_nanoseconds/1" {
    registers [t0]
    params [r0]
    result t4
}
function f8 "async.event::Event.set" intrinsic "okkama.event.Event.set" [] [] {
    registers [t6]
    params [r0]
    result void
}
function f9 "async.event::Event.new#0" intrinsic "okkama.event.Event.new" [obj1] [] {
    registers []
    params []
    result t6
}
function f10 "core::ArraySequence<Future<int>, Future<int>>.cursor" {
    registers [t15, t8, t2, t21, t1]
    params [r0]
    result t1
    0: obj.get r1, r0, 0
    1: const r2, int 0
    2: call r3, f26, [r1, r2]
    3: role.wrap r4, r3, role2
    4: ret r4
}
function f11 "core::ArraySequence<Future<int>, Future<int>>.size" {
    registers [t15, t8, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: arr.len r2, r1
    2: ret r2
}
function f12 "core::ArraySequence<Future<int>, Future<int>>.item" {
    registers [t15, t2, t8, t3]
    params [r0, r1]
    result t3
    0: obj.get r2, r0, 0
    1: arr.get r3, r2, r1
    2: ret r3
}
function f13 "core::ArraySequence<Future<int>, Future<int>>.first" {
    registers [t15, t8, t2, t3]
    params [r0]
    result t3
    0: obj.get r1, r0, 0
    1: const r2, int 0
    2: arr.lookup r1, r2, r3, @3, @4
    3: ret r3
    4: trap empty, "Cannot read first item of an empty sequence"
}
function f14 "core::ArraySequence<Future<int>, Future<int>>.last" {
    registers [t15, t8, t2, t2, t2, t3]
    params [r0]
    result t3
    0: obj.get r1, r0, 0
    1: arr.len r2, r1
    2: const r3, int 1
    3: int.sub r4, r2, r3
    4: arr.lookup r1, r4, r5, @5, @6
    5: ret r5
    6: trap empty, "Cannot read last item of an empty sequence"
}
function f15 "core::ArraySequence<Future<int>, Future<int>>.new" {
    registers [t8, t15]
    params [r0]
    result t15
    0: obj.new r1, obj5, [r0]
    1: ret r1
}
function f16 "async.collect::AllFutures<int>.new#0" {
    registers [t7, t16]
    params [r0]
    result t16
    0: obj.new r1, obj4, [r0]
    1: ret r1
}
function f17 "async.collect::AllFutures<int>.results" async intrinsic "okkama.collect.AllFutures.results" [obj9] [] {
    registers [t16]
    params [r0]
    result t17
}
function f18 "async.collect::Raced<int>.new#0" {
    registers [t7, t19]
    params [r0]
    result t19
    0: obj.new r1, obj6, [r0]
    1: ret r1
}
function f19 "async.collect::Raced<int>.result" async intrinsic "okkama.collect.Raced.result" [obj10] [] {
    registers [t19]
    params [r0]
    result t2
}
function f20 "async.event::Event.ready" intrinsic "okkama.event.Event.ready" [] [] {
    registers [t6]
    params [r0]
    result t4
}
function f21 "async.collect::FirstSuccessful<int>.new#0" {
    registers [t7, t20]
    params [r0]
    result t20
    0: obj.new r1, obj7, [r0]
    1: ret r1
}
function f22 "async.collect::FirstSuccessful<int>.result" async intrinsic "okkama.collect.FirstSuccessful.result" [obj10] [] {
    registers [t20]
    params [r0]
    result t2
}
function f23 "core::ArrayCursor<Future<int>, Future<int>>.available" {
    registers [t21, t8, t2, t2, t4]
    params [r0]
    result t4
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.len r3, r1
    3: int.cmp lt, r4, r2, r3
    4: ret r4
}
function f24 "core::ArrayCursor<Future<int>, Future<int>>.item" {
    registers [t21, t8, t2, t3]
    params [r0]
    result t3
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.lookup r1, r2, r3, @3, @4
    3: ret r3
    4: trap empty, "Array cursor is exhausted"
}
function f25 "core::ArrayCursor<Future<int>, Future<int>>.successor" {
    registers [t21, t8, t2, t3, t2, t2, t1]
    params [r0]
    result t1
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.lookup r1, r2, r3, @3, @8
    3: const r4, int 1
    4: int.add r5, r2, r4
    5: call r0, f26, [r1, r5]
    6: role.wrap r6, r0, role2
    7: ret r6
    8: trap empty, "Array cursor is exhausted"
}
function f26 "core::ArrayCursor<Future<int>, Future<int>>.new" {
    registers [t8, t2, t21]
    params [r0, r1]
    result t21
    0: obj.new r2, obj8, [r0, r1]
    1: ret r2
}
function f27 "core::ArraySequence<int, int>.cursor" {
    registers [t22, t9, t2, t23, t5]
    params [r0]
    result t5
    0: obj.get r1, r0, 0
    1: const r2, int 0
    2: call r3, f36, [r1, r2]
    3: role.wrap r4, r3, role6
    4: ret r4
}
function f28 "core::ArraySequence<int, int>.size" {
    registers [t22, t9, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: arr.len r2, r1
    2: ret r2
}
function f29 "core::ArraySequence<int, int>.item" {
    registers [t22, t2, t9, t2]
    params [r0, r1]
    result t2
    0: obj.get r2, r0, 0
    1: arr.get r3, r2, r1
    2: ret r3
}
function f30 "core::ArraySequence<int, int>.first" {
    registers [t22, t9, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: const r2, int 0
    2: arr.lookup r1, r2, r2, @3, @4
    3: ret r2
    4: trap empty, "Cannot read first item of an empty sequence"
}
function f31 "core::ArraySequence<int, int>.last" {
    registers [t22, t9, t2, t2, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: arr.len r2, r1
    2: const r3, int 1
    3: int.sub r4, r2, r3
    4: arr.lookup r1, r4, r2, @5, @6
    5: ret r2
    6: trap empty, "Cannot read last item of an empty sequence"
}
function f32 "async.collect::NoFutures.text" {
    registers [t24, t0]
    params [r0]
    result t0
    0: const r1, str "no futures remain"
    1: ret r1
}
function f33 "core::ArrayCursor<int, int>.available" {
    registers [t23, t9, t2, t2, t4]
    params [r0]
    result t4
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.len r3, r1
    3: int.cmp lt, r4, r2, r3
    4: ret r4
}
function f34 "core::ArrayCursor<int, int>.item" {
    registers [t23, t9, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.lookup r1, r2, r2, @3, @4
    3: ret r2
    4: trap empty, "Array cursor is exhausted"
}
function f35 "core::ArrayCursor<int, int>.successor" {
    registers [t23, t9, t2, t2, t2, t5]
    params [r0]
    result t5
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.lookup r1, r2, r3, @3, @8
    3: const r3, int 1
    4: int.add r4, r2, r3
    5: call r0, f36, [r1, r4]
    6: role.wrap r5, r0, role6
    7: ret r5
    8: trap empty, "Array cursor is exhausted"
}
function f36 "core::ArrayCursor<int, int>.new" {
    registers [t9, t2, t23]
    params [r0, r1]
    result t23
    0: obj.new r2, obj11, [r0, r1]
    1: ret r2
}
entry f6
