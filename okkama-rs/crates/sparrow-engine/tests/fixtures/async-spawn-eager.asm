sparrow 9
module "async_spawn_eager" "v1.0.0"
type t0 object obj0
type t1 int
type t2 bool
type t3 future t1
object obj0 "Walk" {
}
function f0 "Walk.new#0" {
    registers [t0]
    params []
    result t0
    0: obj.new r0, obj0, []
    1: ret r0
}
function f1 "Walk.descend" async {
    registers [t0, t1, t1, t2, t1, t3]
    params [r0, r1]
    result t1
    0: const r2, int 0
    1: int.cmp eq, r3, r1, r2
    2: br r3, @3, @5
    3: const r1, int 42
    4: jmp @10
    5: const r2, int 1
    6: int.sub r4, r1, r2
    7: call r5, f1, [r0, r4]
    8: await r1, r5
    9: jmp @10
    10: ret r1
}
function f2 "main" async {
    registers [t0, t1, t3, t3, t2]
    params []
    result t1
    0: call r0, f0, []
    1: const r1, int 200
    2: call r2, f1, [r0, r1]
    3: fut.spawn r3, r2
    4: fut done, r4, r3
    5: br r4, @6, @8
    6: await r1, r3
    7: jmp @10
    8: const r1, int 0
    9: jmp @10
    10: ret r1
}
entry f2
