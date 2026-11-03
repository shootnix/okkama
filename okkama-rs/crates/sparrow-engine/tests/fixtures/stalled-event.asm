sparrow 9
module "stalled_event" "v1.0.0"
type t0 bool
type t1 object obj0
type t2 future t1
type t3 int
type t4 failure
object obj0 "async.event::Event" {
    field "initial" t0
}
function f0 "main" async {
    registers [t1, t1, t2, t3, t4, t4, t4]
    params []
    result t3
    handler 2 7 @10 r4
    handler 10 12 @12 r5
    0: call r0, f1, []
    1: jmp @2
    2: call r1, f1, []
    3: call r2, f2, [r1]
    4: await r1, r2
    5: const r3, int 42
    6: jmp @7
    7: call _, f3, [r0]
    8: jmp @9
    9: ret r3
    10: call _, f3, [r0]
    11: jmp @14
    12: fail.secondary r6, r4, r5
    13: jmp @14(r4 <- r6)
    14: raise r4
}
function f1 "async.event::Event.new#0" intrinsic "okkama.event.Event.new" [obj0] [] {
    registers []
    params []
    result t1
}
function f2 "async.event::Event.wait" async intrinsic "okkama.event.Event.wait" [] [] {
    registers [t1]
    params [r0]
    result t1
}
function f3 "async.event::Event.set" intrinsic "okkama.event.Event.set" [] [] {
    registers [t1]
    params [r0]
    result void
}
entry f0
