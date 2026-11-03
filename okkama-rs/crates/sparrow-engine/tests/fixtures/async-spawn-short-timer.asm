sparrow 9
module "async_spawn_short_timer" "v1.0.0"
type t0 object obj0
type t1 str
type t2 bool
type t3 future t2
type t4 int
type t5 future t4
object obj0 "Job" {
}
function f0 "Job.new#0" {
    registers [t0]
    params []
    result t0
    0: obj.new r0, obj0, []
    1: ret r0
}
function f1 "Job.run" async {
    registers [t0, t1, t3, t2, t4]
    params [r0]
    result t4
    0: const r1, str "1"
    1: call r2, f3, [r1]
    2: await r3, r2
    3: const r4, int 42
    4: ret r4
}
function f2 "main" async {
    registers [t0, t5, t5, t2, t4]
    params []
    result t4
    0: call r0, f0, []
    1: call r1, f1, [r0]
    2: fut.spawn r2, r1
    3: fut done, r3, r2
    4: br r3, @5, @7
    5: const r4, int 0
    6: jmp @9
    7: await r4, r2
    8: jmp @9
    9: ret r4
}
function f3 "runtime.time::sleep_nanoseconds" async native "runtime/time@1::sleep_nanoseconds/1" {
    registers [t1]
    params [r0]
    result t2
}
entry f2
