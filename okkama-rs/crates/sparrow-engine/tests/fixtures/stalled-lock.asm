sparrow 9
module "stalled_lock" "v1.0.0"
type t0 bool
type t1 object obj0
type t2 object obj1
type t3 object obj2
type t4 future t3
type t5 int
type t6 failure
role role0 "io.stream::Closing" {
    method "closed" [] -> t0
    method "close" [] -> void
}
object obj0 "async.event::Event" {
    field "initial" t0
}
object obj1 "async.lock::Lock" {
}
object obj2 "async.lock::LockGuard" {
    does role0 [f4, f5]
}
function f0 "main" async {
    registers [t1, t2, t4, t3, t3, t5, t6, t6, t6]
    params []
    result t5
    handler 2 6 @20 r6
    handler 6 11 @16 r6
    handler 11 13 @20 r6
    handler 16 18 @18 r7
    handler 20 22 @22 r7
    0: call r0, f1, []
    1: jmp @2
    2: call r1, f2, []
    3: call r2, f3, [r1]
    4: await r3, r2
    5: jmp @6
    6: call r2, f3, [r1]
    7: await r4, r2
    8: call _, f5, [r4]
    9: const r5, int 42
    10: jmp @11
    11: call _, f5, [r3]
    12: jmp @13
    13: call _, f6, [r0]
    14: jmp @15
    15: ret r5
    16: call _, f5, [r3]
    17: jmp @20
    18: fail.secondary r8, r6, r7
    19: jmp @20(r6 <- r8)
    20: call _, f6, [r0]
    21: jmp @24
    22: fail.secondary r8, r6, r7
    23: jmp @24(r6 <- r8)
    24: raise r6
}
function f1 "async.event::Event.new#0" intrinsic "okkama.event.Event.new" [obj0] [] {
    registers []
    params []
    result t1
}
function f2 "async.lock::Lock.new#0" intrinsic "okkama.lock.Lock.new" [obj1] [] {
    registers []
    params []
    result t2
}
function f3 "async.lock::Lock.guard" async intrinsic "okkama.lock.Lock.guard" [obj2] [] {
    registers [t2]
    params [r0]
    result t3
}
function f4 "async.lock::LockGuard.closed" intrinsic "okkama.lock.LockGuard.closed" [] [] {
    registers [t3]
    params [r0]
    result t0
}
function f5 "async.lock::LockGuard.close" intrinsic "okkama.lock.LockGuard.close" [] [] {
    registers [t3]
    params [r0]
    result void
}
function f6 "async.event::Event.set" intrinsic "okkama.event.Event.set" [] [] {
    registers [t1]
    params [r0]
    result void
}
entry f0
