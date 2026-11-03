sparrow 9
module "async_lock" "v1.0.0"
type t0 bool
type t1 str
type t2 object obj1
type t3 object obj2
type t4 object obj0
type t5 object obj3
type t6 future t5
type t7 int
type t8 failure
type t9 future t7
type t10 object obj4
role role0 "io.stream::Closing" {
    method "closed" [] -> t0
    method "close" [] -> void
}
role role1 "Error" builtin {
    method "text" [] -> t1
}
object obj0 "Reader" {
    field "lock" t2
    field "signal" t3
    field "first" t0
}
object obj1 "async.lock::Lock" {
}
object obj2 "async.event::Event" {
    field "initial" t0
}
object obj3 "async.lock::LockGuard" {
    does role0 [f4, f5]
}
object obj4 "Cancelled" builtin {
    does role1 [f10]
}
function f0 "Reader.new#0" {
    registers [t2, t3, t0, t4]
    params [r0, r1, r2]
    result t4
    0: obj.new r3, obj0, [r0, r1, r2]
    1: ret r3
}
function f1 "Reader.run" async {
    registers [t4, t2, t6, t5, t0, t3, t7, t8, t8, t8]
    params [r0]
    result t7
    handler 4 6 @21 r7
    handler 6 10 @21 r7
    handler 10 13 @21 r7
    handler 13 15 @21 r7
    handler 15 17 @21 r7
    handler 21 23 @23 r8
    0: obj.get r1, r0, 0
    1: call r2, f3, [r1]
    2: await r3, r2
    3: jmp @4
    4: obj.get r4, r0, 2
    5: br r4, @6, @10
    6: obj.get r5, r0, 1
    7: call _, f6, [r5]
    8: const r6, int 21
    9: jmp @18
    10: obj.get r5, r0, 1
    11: call r4, f7, [r5]
    12: br r4, @13, @15
    13: const r6, int 21
    14: jmp @17
    15: const r6, int 0
    16: jmp @17
    17: jmp @18
    18: call _, f5, [r3]
    19: jmp @20
    20: ret r6
    21: call _, f5, [r3]
    22: jmp @25
    23: fail.secondary r9, r7, r8
    24: jmp @25(r7 <- r9)
    25: raise r7
}
function f2 "main" async {
    registers [t2, t6, t5, t3, t3, t0, t4, t9, t9, t9, t9, t7, t7, t7, t8, t10, t0, t8, t8]
    params []
    result t7
    handler 24 27 @27 r14
    handler 28 30 @53 r17
    0: call r0, f8, []
    1: call r1, f3, [r0]
    2: await r2, r1
    3: call r3, f9, []
    4: call r4, f9, []
    5: const r5, bool true
    6: call r6, f0, [r0, r3, r5]
    7: call r7, f1, [r6]
    8: fut.spawn r8, r7
    9: const r5, bool true
    10: call r6, f0, [r0, r4, r5]
    11: call r7, f1, [r6]
    12: fut.spawn r9, r7
    13: const r5, bool false
    14: call r6, f0, [r0, r3, r5]
    15: call r7, f1, [r6]
    16: fut.spawn r10, r7
    17: fut cancel, _, r9
    18: fut cancel, _, r9
    19: call _, f5, [r2]
    20: call _, f5, [r2]
    21: await r11, r8
    22: await r12, r10
    23: jmp @24
    24: await r13, r9
    25: const r5, bool false
    26: jmp @30
    27: catch r14, object, 4, r15, @28, @55
    28: const r5, bool true
    29: jmp @30
    30: br r5, @31, @34
    31: call r5, f7, [r4]
    32: bool.not r16, r5
    33: jmp @36
    34: const r5, bool false
    35: jmp @36(r16 <- r5)
    36: br r16, @37, @40
    37: call r5, f11, [r0]
    38: bool.not r16, r5
    39: jmp @42
    40: const r5, bool false
    41: jmp @42(r16 <- r5)
    42: br r16, @43, @45
    43: call r5, f4, [r2]
    44: jmp @47
    45: const r5, bool false
    46: jmp @47
    47: br r5, @48, @50
    48: int.add r13, r11, r12
    49: jmp @52
    50: const r11, int 0
    51: jmp @52(r13 <- r11)
    52: ret r13
    53: fail.caused r18, r17, r14
    54: raise r18
    55: raise r14
}
function f3 "async.lock::Lock.guard" async intrinsic "okkama.lock.Lock.guard" [obj3] [] {
    registers [t2]
    params [r0]
    result t5
}
function f4 "async.lock::LockGuard.closed" intrinsic "okkama.lock.LockGuard.closed" [] [] {
    registers [t5]
    params [r0]
    result t0
}
function f5 "async.lock::LockGuard.close" intrinsic "okkama.lock.LockGuard.close" [] [] {
    registers [t5]
    params [r0]
    result void
}
function f6 "async.event::Event.set" intrinsic "okkama.event.Event.set" [] [] {
    registers [t3]
    params [r0]
    result void
}
function f7 "async.event::Event.ready" intrinsic "okkama.event.Event.ready" [] [] {
    registers [t3]
    params [r0]
    result t0
}
function f8 "async.lock::Lock.new#0" intrinsic "okkama.lock.Lock.new" [obj1] [] {
    registers []
    params []
    result t2
}
function f9 "async.event::Event.new#0" intrinsic "okkama.event.Event.new" [obj2] [] {
    registers []
    params []
    result t3
}
function f10 "Cancelled.text" {
    registers [t10, t1]
    params [r0]
    result t1
    0: const r1, str "Operation was cancelled"
    1: ret r1
}
function f11 "async.lock::Lock.locked" intrinsic "okkama.lock.Lock.locked" [] [] {
    registers [t2]
    params [r0]
    result t0
}
entry f2
