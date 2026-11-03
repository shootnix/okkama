sparrow 9
module "async_error_text_spawn" "v1.0.0"
type t0 str
type t1 object obj1
type t2 bool
type t3 object obj0
type t4 future t2
type t5 int
type t6 failure
type t7 object obj2
type t8 future t5
type t9 object obj3
role role0 "Error" builtin {
    method "text" [] -> t0
}
object obj0 "Background" {
    field "done" t1
}
object obj1 "async.event::Event" {
    field "initial" t2
}
object obj2 "Problem" {
    field "done" t1
    does role0 [f2]
}
object obj3 "Failing" {
    field "done" t1
}
function f0 "Background.new#0" {
    registers [t1, t3]
    params [r0]
    result t3
    0: obj.new r1, obj0, [r0]
    1: ret r1
}
function f1 "Background.run" async {
    registers [t3, t0, t4, t2, t5, t1, t6, t6, t6]
    params [r0]
    result t5
    handler 0 5 @9 r6
    handler 9 12 @12 r7
    0: const r1, str "9223372036854775807"
    1: call r2, f7, [r1]
    2: await r3, r2
    3: const r4, int 0
    4: jmp @5
    5: obj.get r5, r0, 0
    6: call _, f8, [r5]
    7: jmp @8
    8: ret r4
    9: obj.get r5, r0, 0
    10: call _, f8, [r5]
    11: jmp @14
    12: fail.secondary r8, r6, r7
    13: jmp @14(r6 <- r8)
    14: raise r6
}
function f2 "Problem.text" {
    registers [t7, t1, t3, t8, t8, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 0
    1: call r2, f0, [r1]
    2: call r3, f1, [r2]
    3: fut.spawn r4, r3
    4: const r5, str "background failure"
    5: ret r5
}
function f3 "Problem.new#0" {
    registers [t1, t7]
    params [r0]
    result t7
    0: obj.new r1, obj2, [r0]
    1: ret r1
}
function f4 "Failing.new#0" {
    registers [t1, t9]
    params [r0]
    result t9
    0: obj.new r1, obj3, [r0]
    1: ret r1
}
function f5 "Failing.run" async {
    registers [t9, t1, t7]
    params [r0]
    result t5
    0: obj.get r1, r0, 0
    1: call r2, f3, [r1]
    2: throw r2
}
function f6 "main" async {
    registers [t1, t9, t8, t8, t5]
    params []
    result t5
    0: call r0, f9, []
    1: call r1, f4, [r0]
    2: call r2, f5, [r1]
    3: fut.spawn r3, r2
    4: const r4, int 42
    5: ret r4
}
function f7 "runtime.time::sleep_nanoseconds" async native "runtime/time@1::sleep_nanoseconds/1" {
    registers [t0]
    params [r0]
    result t2
}
function f8 "async.event::Event.set" intrinsic "okkama.event.Event.set" [] [] {
    registers [t1]
    params [r0]
    result void
}
function f9 "async.event::Event.new#0" intrinsic "okkama.event.Event.new" [obj1] [] {
    registers []
    params []
    result t1
}
entry f6
