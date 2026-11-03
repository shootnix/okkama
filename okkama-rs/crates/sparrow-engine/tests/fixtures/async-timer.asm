sparrow 9
module "async_timer" "v1.0.0"
type t0 str
type t1 bool
type t2 future t1
type t3 int
function f0 "main" async {
    registers [t0, t2, t1, t1, t3]
    params []
    result t3
    0: const r0, str "10000000"
    1: call r1, f1, [r0]
    2: await r2, r1
    3: await r3, r1
    4: br r2, @5, @6
    5: jmp @8
    6: const r2, bool false
    7: jmp @8(r3 <- r2)
    8: br r3, @9, @11
    9: const r4, int 42
    10: jmp @13
    11: const r4, int 0
    12: jmp @13
    13: ret r4
}
function f1 "runtime.time::sleep_nanoseconds" async native "runtime/time@1::sleep_nanoseconds/1" {
    registers [t0]
    params [r0]
    result t1
}
entry f0
