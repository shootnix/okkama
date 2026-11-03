sparrow 9
module "async_spawn" "v1.0.0"
type t0 str
type t1 future t0
type t2 object obj0
type t3 bool
type t4 future t3
type t5 object obj1
type t6 int
type t7 future t6
object obj0 "Job" {
}
object obj1 "Reader" {
    field "pending" t1
}
function f0 "Job.new#0" {
    registers [t2]
    params []
    result t2
    0: obj.new r0, obj0, []
    1: ret r0
}
function f1 "Job.value" async {
    registers [t2, t0, t4, t3]
    params [r0]
    result t0
    0: const r1, str "1000000"
    1: call r2, f5, [r1]
    2: await r3, r2
    3: const r1, str "done"
    4: ret r1
}
function f2 "Reader.new#0" {
    registers [t1, t5]
    params [r0]
    result t5
    0: obj.new r1, obj1, [r0]
    1: ret r1
}
function f3 "Reader.value" async {
    registers [t5, t1, t0, t0, t0, t3, t6]
    params [r0]
    result t6
    0: obj.get r1, r0, 0
    1: await r2, r1
    2: obj.get r1, r0, 0
    3: await r3, r1
    4: const r4, str "done"
    5: str.cmp eq, r5, r2, r4
    6: br r5, @7, @10
    7: const r2, str "done"
    8: str.cmp eq, r5, r3, r2
    9: jmp @12
    10: const r5, bool false
    11: jmp @12
    12: br r5, @13, @15
    13: const r6, int 21
    14: jmp @17
    15: const r6, int 0
    16: jmp @17
    17: ret r6
}
function f4 "main" async {
    registers [t2, t1, t1, t5, t7, t7, t7, t6, t6, t6]
    params []
    result t6
    0: call r0, f0, []
    1: call r1, f1, [r0]
    2: fut.spawn r2, r1
    3: call r3, f2, [r2]
    4: call r4, f3, [r3]
    5: fut.spawn r5, r4
    6: call r3, f2, [r2]
    7: call r4, f3, [r3]
    8: fut.spawn r6, r4
    9: await r7, r5
    10: await r8, r6
    11: int.add r9, r7, r8
    12: ret r9
}
function f5 "runtime.time::sleep_nanoseconds" async native "runtime/time@1::sleep_nanoseconds/1" {
    registers [t0]
    params [r0]
    result t3
}
entry f4
