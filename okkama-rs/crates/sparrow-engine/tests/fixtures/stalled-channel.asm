sparrow 9
module "stalled_channel" "v1.0.0"
type t0 str
type t1 bool
type t2 int
type t3 object obj0
type t4 object obj1
type t5 object obj2
type t6 future t2
type t7 failure
type t8 object obj3
type t9 object obj4
role role0 "Error" builtin {
    method "text" [] -> t0
}
object obj0 "async.event::Event" {
    field "initial" t1
}
object obj1 "async.channel::Channel<int>" {
}
object obj2 "async.channel::Receiving<int>" {
}
object obj3 "async.channel::InvalidChannelCapacity" {
    field "capacity" t2
    does role0 [f6]
}
object obj4 "async.channel::ChannelClosed" {
    does role0 [f7]
}
function f0 "main" async {
    registers [t3, t2, t4, t5, t6, t7, t7, t7]
    params []
    result t2
    handler 2 8 @11 r5
    handler 11 13 @13 r6
    0: call r0, f1, []
    1: jmp @2
    2: const r1, int 0
    3: call r2, f2, [r1]
    4: call r3, f3, [r2]
    5: call r4, f4, [r3]
    6: await r1, r4
    7: jmp @8
    8: call _, f5, [r0]
    9: jmp @10
    10: ret r1
    11: call _, f5, [r0]
    12: jmp @15
    13: fail.secondary r7, r5, r6
    14: jmp @15(r5 <- r7)
    15: raise r5
}
function f1 "async.event::Event.new#0" intrinsic "okkama.event.Event.new" [obj0] [] {
    registers []
    params []
    result t3
}
function f2 "async.channel::Channel<int>.new#0" intrinsic "okkama.channel.Channel.new_capacity" [obj1, obj3] [] {
    registers [t2]
    params [r0]
    result t4
}
function f3 "async.channel::Channel<int>.receiving" intrinsic "okkama.channel.Channel.receiving" [obj2] [] {
    registers [t4]
    params [r0]
    result t5
}
function f4 "async.channel::Receiving<int>.item" async intrinsic "okkama.channel.Receiving.item" [obj4] [] {
    registers [t5]
    params [r0]
    result t2
}
function f5 "async.event::Event.set" intrinsic "okkama.event.Event.set" [] [] {
    registers [t3]
    params [r0]
    result void
}
function f6 "async.channel::InvalidChannelCapacity.text" {
    registers [t8, t0]
    params [r0]
    result t0
    0: const r1, str "channel capacity must not be negative"
    1: ret r1
}
function f7 "async.channel::ChannelClosed.text" {
    registers [t9, t0]
    params [r0]
    result t0
    0: const r1, str "channel is closed"
    1: ret r1
}
entry f0
