sparrow 9
module "async_event" "v1.0.0"
type t0 str
type t1 object obj1
type t2 bool
type t3 object obj0
type t4 future t1
type t5 int
type t6 future t5
type t7 failure
type t8 object obj2
role role0 "Error" builtin {
    method "text" [] -> t0
}
object obj0 "Reader" {
    field "event" t1
}
object obj1 "async.event::Event" {
    field "initial" t2
}
object obj2 "Cancelled" builtin {
    does role0 [f6]
}
function f0 "Reader.new#0" {
    registers [t1, t3]
    params [r0]
    result t3
    0: obj.new r1, obj0, [r0]
    1: ret r1
}
function f1 "Reader.run" async {
    registers [t3, t1, t4, t5]
    params [r0]
    result t5
    0: obj.get r1, r0, 0
    1: call r2, f3, [r1]
    2: await r1, r2
    3: const r3, int 21
    4: ret r3
}
function f2 "main" async {
    registers [t1, t3, t6, t6, t6, t5, t7, t8, t5, t4, t1, t2, t5, t7, t7]
    params []
    result t5
    handler 11 14 @14 r6
    handler 15 17 @31 r13
    0: call r0, f4, []
    1: call r1, f0, [r0]
    2: call r2, f1, [r1]
    3: fut.spawn r3, r2
    4: call r1, f0, [r0]
    5: call r2, f1, [r1]
    6: fut.spawn r4, r2
    7: fut cancel, _, r3
    8: call _, f5, [r0]
    9: call _, f5, [r0]
    10: jmp @11
    11: await r5, r3
    12: const r5, int 0
    13: jmp @17
    14: catch r6, object, 2, r7, @15, @33
    15: const r5, int 21
    16: jmp @17
    17: await r8, r4
    18: call r9, f3, [r0]
    19: await r10, r9
    20: const r11, bool true
    21: call r10, f7, [r11]
    22: call r9, f3, [r10]
    23: await r10, r9
    24: call r11, f8, [r0]
    25: br r11, @26, @28
    26: int.add r12, r5, r8
    27: jmp @30
    28: const r5, int 0
    29: jmp @30(r12 <- r5)
    30: ret r12
    31: fail.caused r14, r13, r6
    32: raise r14
    33: raise r6
}
function f3 "async.event::Event.wait" async intrinsic "okkama.event.Event.wait" [] [] {
    registers [t1]
    params [r0]
    result t1
}
function f4 "async.event::Event.new#0" intrinsic "okkama.event.Event.new" [obj1] [] {
    registers []
    params []
    result t1
}
function f5 "async.event::Event.set" intrinsic "okkama.event.Event.set" [] [] {
    registers [t1]
    params [r0]
    result void
}
function f6 "Cancelled.text" {
    registers [t8, t0]
    params [r0]
    result t0
    0: const r1, str "Operation was cancelled"
    1: ret r1
}
function f7 "async.event::Event.new#1" intrinsic "okkama.event.Event.new_ready" [obj1] [] {
    registers [t2]
    params [r0]
    result t1
}
function f8 "async.event::Event.ready" intrinsic "okkama.event.Event.ready" [] [] {
    registers [t1]
    params [r0]
    result t2
}
entry f2
