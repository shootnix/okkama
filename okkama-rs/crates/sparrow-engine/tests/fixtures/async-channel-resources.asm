sparrow 9
module "async_channel_resources" "v1.0.0"
type t0 bool
type t1 str
type t2 object obj1
type t3 object obj3
type t4 int
type t5 object obj0
type t6 region
type t7 failure
type t8 object obj2
type t9 future t3
type t10 object obj4
type t11 future t4
type t12 object obj5
type t13 future t5
type t14 object obj6
type t15 object obj7
type t16 object obj8
role role0 "io.stream::Closing" {
    method "closed" [] -> t0
    method "close" [] -> void
}
role role1 "Error" builtin {
    method "text" [] -> t1
}
object obj0 "Resource" {
    field "destroyed" t2
    destroy f2
}
object obj1 "async.atomic::Atomic<int>" {
    does role0 [f0, f1]
}
object obj2 "Sender" {
    field "output" t3
    field "destroyed" t2
}
object obj3 "async.channel::Sending<Resource>" {
}
object obj4 "async.channel::Channel<Resource>" {
}
object obj5 "async.channel::Receiving<Resource>" {
}
object obj6 "async.atomic::AtomicClosed" {
    does role1 [f16]
}
object obj7 "async.channel::ChannelClosed" {
    does role1 [f17]
}
object obj8 "async.channel::InvalidChannelCapacity" {
    field "capacity" t4
    does role1 [f18]
}
function f0 "async.atomic::Atomic<int>.closed" intrinsic "okkama.atomic.Atomic.closed" [] [] {
    registers [t2]
    params [r0]
    result t0
}
function f1 "async.atomic::Atomic<int>.close" intrinsic "okkama.atomic.Atomic.close" [] [] {
    registers [t2]
    params [r0]
    result void
}
function f2 "Resource.destroy" {
    registers [t5, t2, t2, t4, t4, t4]
    params [r0]
    result void
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: obj.get r2, r0, 0
    4: call r3, f9, [r2]
    5: const r4, int 1
    6: int.add r5, r3, r4
    7: call _, f8, [r1, r5]
    8: jmp @9
    9: ret _
}
function f3 "Resource.new#0" {
    registers [t2, t5]
    params [r0]
    result t5
    0: jmp @1
    1: jmp @2
    2: obj.new r1, obj0, [r0]
    3: jmp @4
    4: ret r1
}
function f4 "Resource.value" {
    registers [t5, t6, t4, t7, t7]
    params [r0]
    result t4
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
function f5 "Sender.new#0" {
    registers [t3, t2, t8]
    params [r0, r1]
    result t8
    0: jmp @1
    1: jmp @2
    2: obj.new r2, obj2, [r0, r1]
    3: jmp @4
    4: ret r2
}
function f6 "Sender.run" async {
    registers [t8, t6, t2, t5, t3, t9, t4, t7, t7]
    params [r0]
    result t4
    handler 3 12 @15 r7
    0: jmp @1
    1: region.enter r1
    2: jmp @3
    3: obj.get r2, r0, 1
    4: call r3, f3, [r2]
    5: region.track r1, r3
    6: region.adopt r1, r3
    7: obj.get r4, r0, 0
    8: call r5, f10, [r4, r3]
    9: await r4, r5
    10: const r6, int 0
    11: jmp @12
    12: region.leave r1, _
    13: jmp @14
    14: ret r6
    15: region.unwind r8, r1, r7
    16: raise r8
}
function f7 "main" async {
    registers [t6, t4, t2, t10, t3, t8, t11, t4, t0, t12, t13, t5, t7, t7]
    params []
    result t4
    handler 3 15 @37 r12
    handler 15 23 @37 r12
    handler 23 27 @37 r12
    handler 27 29 @37 r12
    handler 30 32 @37 r12
    handler 35 36 @37 r12
    handler 36 37 @37 r12
    0: jmp @1
    1: region.enter r0
    2: jmp @3
    3: const r1, int 0
    4: call r2, f11, [r1]
    5: const r1, int 1
    6: call r3, f12, [r1]
    7: call r4, f13, [r3]
    8: call r5, f5, [r4, r2]
    9: call r6, f6, [r5]
    10: await r1, r6
    11: call r1, f9, [r2]
    12: const r7, int 1
    13: int.cmp eq, r8, r1, r7
    14: br r8, @15, @36
    15: call r9, f14, [r3]
    16: call r10, f15, [r9]
    17: await r11, r10
    18: region.adopt r0, r11
    19: call r1, f4, [r11]
    20: const r7, int 42
    21: int.cmp eq, r8, r1, r7
    22: br r8, @23, @27
    23: call r1, f9, [r2]
    24: const r7, int 1
    25: int.cmp eq, r8, r1, r7
    26: jmp @29
    27: const r8, bool false
    28: jmp @29
    29: br r8, @30, @35
    30: const r1, int 42
    31: jmp @32
    32: region.leave r0, _
    33: jmp @34
    34: ret r1
    35: trap confirm, "confirm failed"
    36: trap confirm, "confirm failed"
    37: region.unwind r13, r0, r12
    38: raise r13
}
function f8 "async.atomic::Atomic<int>.replace" intrinsic "okkama.atomic.Atomic.replace" [obj6] [] {
    registers [t2, t4]
    params [r0, r1]
    result void
}
function f9 "async.atomic::Atomic<int>.value" intrinsic "okkama.atomic.Atomic.value" [obj6] [] {
    registers [t2]
    params [r0]
    result t4
}
function f10 "async.channel::Sending<Resource>.send" async intrinsic "okkama.channel.Sending.send" [obj7] [] {
    registers [t3, t5]
    params [r0, r1]
    result t3
}
function f11 "async.atomic::Atomic<int>.new#0" intrinsic "okkama.atomic.Atomic.new_value" [obj1] [] {
    registers [t4]
    params [r0]
    result t2
}
function f12 "async.channel::Channel<Resource>.new#0" intrinsic "okkama.channel.Channel.new_capacity" [obj4, obj8] [] {
    registers [t4]
    params [r0]
    result t10
}
function f13 "async.channel::Channel<Resource>.sending" intrinsic "okkama.channel.Channel.sending" [obj3] [] {
    registers [t10]
    params [r0]
    result t3
}
function f14 "async.channel::Channel<Resource>.receiving" intrinsic "okkama.channel.Channel.receiving" [obj5] [] {
    registers [t10]
    params [r0]
    result t12
}
function f15 "async.channel::Receiving<Resource>.item" async intrinsic "okkama.channel.Receiving.item" [obj7] [] {
    registers [t12]
    params [r0]
    result t5
}
function f16 "async.atomic::AtomicClosed.text" {
    registers [t14, t1]
    params [r0]
    result t1
    0: jmp @1
    1: jmp @2
    2: const r1, str "atomic state is closed"
    3: jmp @4
    4: ret r1
}
function f17 "async.channel::ChannelClosed.text" {
    registers [t15, t1]
    params [r0]
    result t1
    0: jmp @1
    1: jmp @2
    2: const r1, str "channel is closed"
    3: jmp @4
    4: ret r1
}
function f18 "async.channel::InvalidChannelCapacity.text" {
    registers [t16, t1]
    params [r0]
    result t1
    0: jmp @1
    1: jmp @2
    2: const r1, str "channel capacity must not be negative"
    3: jmp @4
    4: ret r1
}
entry f7
