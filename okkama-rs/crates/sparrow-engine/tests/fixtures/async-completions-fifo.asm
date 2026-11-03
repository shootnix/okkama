sparrow 9
module "async_completions_fifo" "v1.0.0"
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
type t10 object obj2
type t11 future t2
type t12 object obj4
type t13 object obj5
type t14 object obj6
type t15 failure
type t16 object obj7
type t17 future t13
type t18 object obj10
type t19 object obj8
type t20 object obj9
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
object obj0 "Source" {
    field "input" t5
}
object obj1 "async.channel::Receiving<int>" {
}
object obj2 "Read" {
    field "stream" t6
}
object obj3 "async.collect::FutureCompletions<int>" {
    field "origins" t7
}
object obj4 "async.channel::Channel<int>" {
}
object obj5 "async.channel::Sending<int>" {
}
object obj6 "core::ArraySequence<Future<int>, Future<int>>" {
    field "items" t8
    does role0 [f10, f11, f12, f13, f14]
    does role2 [f10]
    does role3 [f11]
}
object obj7 "async.collect::NoFutures" {
    does role4 [f17]
}
object obj8 "async.channel::ChannelClosed" {
    does role4 [f20]
}
object obj9 "async.channel::InvalidChannelCapacity" {
    field "capacity" t1
    does role4 [f21]
}
object obj10 "core::ArrayCursor<Future<int>, Future<int>>" {
    field "items" t8
    field "at" t1
    does role1 [f22, f23, f24]
}
function f0 "Source.new#0" {
    registers [t5, t9]
    params [r0]
    result t9
    0: obj.new r1, obj0, [r0]
    1: ret r1
}
function f1 "Source.run" async {
    registers [t9, t5, t2, t1]
    params [r0]
    result t1
    0: obj.get r1, r0, 0
    1: call r2, f5, [r1]
    2: await r3, r2
    3: ret r3
}
function f2 "Read.new#0" {
    registers [t6, t10]
    params [r0]
    result t10
    0: obj.new r1, obj2, [r0]
    1: ret r1
}
function f3 "Read.run" async {
    registers [t10, t6, t11, t2, t1]
    params [r0]
    result t1
    0: obj.get r1, r0, 0
    1: call r2, f6, [r1]
    2: await r3, r2
    3: await r4, r3
    4: ret r4
}
function f4 "main" async {
    registers [t1, t12, t5, t13, t9, t2, t2, t8, t14, t7, t6, t10, t2, t2, t2, t11, t3, t15, t16, t3, t17, t13, t1, t15, t15]
    params []
    result t1
    handler 19 23 @23 r17
    handler 24 26 @73 r23
    0: const r0, int 0
    1: call r1, f7, [r0]
    2: call r2, f8, [r1]
    3: call r3, f9, [r1]
    4: call r4, f0, [r2]
    5: call r5, f1, [r4]
    6: call r4, f0, [r2]
    7: call r6, f1, [r4]
    8: arr.new r7, [r5, r6]
    9: call r8, f15, [r7]
    10: role.wrap r9, r8, role0
    11: call r10, f16, [r9]
    12: call r11, f2, [r10]
    13: call r12, f3, [r11]
    14: fut.spawn r13, r12
    15: call r11, f2, [r10]
    16: call r12, f3, [r11]
    17: fut.spawn r14, r12
    18: jmp @19
    19: call r15, f6, [r10]
    20: await r12, r15
    21: const r16, bool false
    22: jmp @26
    23: catch r17, object, 7, r18, @24, @75
    24: const r16, bool true
    25: jmp @26
    26: br r16, @27, @30
    27: fut cancelled, r16, r5
    28: bool.not r19, r16
    29: jmp @32
    30: const r16, bool false
    31: jmp @32(r19 <- r16)
    32: br r19, @33, @36
    33: fut cancelled, r16, r6
    34: bool.not r19, r16
    35: jmp @38
    36: const r16, bool false
    37: jmp @38(r19 <- r16)
    38: br r19, @39, @72
    39: const r0, int 1
    40: call r20, f18, [r3, r0]
    41: await r21, r20
    42: await r0, r5
    43: const r22, int 1
    44: int.cmp eq, r16, r0, r22
    45: br r16, @46, @71
    46: call r0, f19, [r10]
    47: const r22, int 1
    48: int.cmp eq, r16, r0, r22
    49: br r16, @50, @70
    50: const r0, int 41
    51: call r20, f18, [r3, r0]
    52: await r3, r20
    53: await r0, r13
    54: const r22, int 1
    55: int.cmp eq, r16, r0, r22
    56: br r16, @57, @69
    57: await r0, r14
    58: const r22, int 41
    59: int.cmp eq, r16, r0, r22
    60: br r16, @61, @68
    61: call r0, f19, [r10]
    62: const r22, int 0
    63: int.cmp eq, r16, r0, r22
    64: br r16, @65, @67
    65: const r0, int 42
    66: ret r0
    67: trap confirm, "confirm failed"
    68: trap confirm, "confirm failed"
    69: trap confirm, "confirm failed"
    70: trap confirm, "confirm failed"
    71: trap confirm, "confirm failed"
    72: trap confirm, "confirm failed"
    73: fail.caused r24, r23, r17
    74: raise r24
    75: raise r17
}
function f5 "async.channel::Receiving<int>.item" async intrinsic "okkama.channel.Receiving.item" [obj8] [] {
    registers [t5]
    params [r0]
    result t1
}
function f6 "async.collect::FutureCompletions<int>.completion" async intrinsic "okkama.collect.FutureCompletions.completion" [obj7] [] {
    registers [t6]
    params [r0]
    result t2
}
function f7 "async.channel::Channel<int>.new#0" intrinsic "okkama.channel.Channel.new_capacity" [obj4, obj9] [] {
    registers [t1]
    params [r0]
    result t12
}
function f8 "async.channel::Channel<int>.receiving" intrinsic "okkama.channel.Channel.receiving" [obj1] [] {
    registers [t12]
    params [r0]
    result t5
}
function f9 "async.channel::Channel<int>.sending" intrinsic "okkama.channel.Channel.sending" [obj5] [] {
    registers [t12]
    params [r0]
    result t13
}
function f10 "core::ArraySequence<Future<int>, Future<int>>.cursor" {
    registers [t14, t8, t1, t18, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 0
    1: const r2, int 0
    2: call r3, f25, [r1, r2]
    3: role.wrap r4, r3, role1
    4: ret r4
}
function f11 "core::ArraySequence<Future<int>, Future<int>>.size" {
    registers [t14, t8, t1]
    params [r0]
    result t1
    0: obj.get r1, r0, 0
    1: arr.len r2, r1
    2: ret r2
}
function f12 "core::ArraySequence<Future<int>, Future<int>>.item" {
    registers [t14, t1, t8, t2]
    params [r0, r1]
    result t2
    0: obj.get r2, r0, 0
    1: arr.get r3, r2, r1
    2: ret r3
}
function f13 "core::ArraySequence<Future<int>, Future<int>>.first" {
    registers [t14, t8, t1, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: const r2, int 0
    2: arr.lookup r1, r2, r3, @3, @4
    3: ret r3
    4: trap empty, "Cannot read first item of an empty sequence"
}
function f14 "core::ArraySequence<Future<int>, Future<int>>.last" {
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
function f15 "core::ArraySequence<Future<int>, Future<int>>.new" {
    registers [t8, t14]
    params [r0]
    result t14
    0: obj.new r1, obj6, [r0]
    1: ret r1
}
function f16 "async.collect::FutureCompletions<int>.new#0" {
    registers [t7, t6]
    params [r0]
    result t6
    0: obj.new r1, obj3, [r0]
    1: ret r1
}
function f17 "async.collect::NoFutures.text" {
    registers [t16, t4]
    params [r0]
    result t4
    0: const r1, str "no futures remain"
    1: ret r1
}
function f18 "async.channel::Sending<int>.send" async intrinsic "okkama.channel.Sending.send" [obj8] [] {
    registers [t13, t1]
    params [r0, r1]
    result t13
}
function f19 "async.collect::FutureCompletions<int>.remaining_count" intrinsic "okkama.collect.FutureCompletions.remaining_count" [] [] {
    registers [t6]
    params [r0]
    result t1
}
function f20 "async.channel::ChannelClosed.text" {
    registers [t19, t4]
    params [r0]
    result t4
    0: const r1, str "channel is closed"
    1: ret r1
}
function f21 "async.channel::InvalidChannelCapacity.text" {
    registers [t20, t4]
    params [r0]
    result t4
    0: const r1, str "channel capacity must not be negative"
    1: ret r1
}
function f22 "core::ArrayCursor<Future<int>, Future<int>>.available" {
    registers [t18, t8, t1, t1, t3]
    params [r0]
    result t3
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.len r3, r1
    3: int.cmp lt, r4, r2, r3
    4: ret r4
}
function f23 "core::ArrayCursor<Future<int>, Future<int>>.item" {
    registers [t18, t8, t1, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.lookup r1, r2, r3, @3, @4
    3: ret r3
    4: trap empty, "Array cursor is exhausted"
}
function f24 "core::ArrayCursor<Future<int>, Future<int>>.successor" {
    registers [t18, t8, t1, t2, t1, t1, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.lookup r1, r2, r3, @3, @8
    3: const r4, int 1
    4: int.add r5, r2, r4
    5: call r0, f25, [r1, r5]
    6: role.wrap r6, r0, role1
    7: ret r6
    8: trap empty, "Array cursor is exhausted"
}
function f25 "core::ArrayCursor<Future<int>, Future<int>>.new" {
    registers [t8, t1, t18]
    params [r0, r1]
    result t18
    0: obj.new r2, obj10, [r0, r1]
    1: ret r2
}
entry f4
