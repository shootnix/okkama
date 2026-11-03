sparrow 9
module "async_worker" "v1.0.0"
type t0 int
type t1 object obj0
type t2 array t0
type t3 builder t0
type t4 bool
type t5 str
type t6 fn [t0] -> t0
type t7 future t0
type t8 fn [t0, t2] -> t2
type t9 future t2
type t10 fn [t0, t5] -> t5
type t11 future t5
type t12 fn [t0, t0] -> t0
object obj0 "Calculator" {
    field "seed" t0
}
function f0 "Calculator.new#0" {
    registers [t0, t1]
    params [r0]
    result t1
    0: obj.new r1, obj0, [r0]
    1: ret r1
}
function f1 "Calculator.calculate" {
    registers [t1, t0, t0, t0]
    params [r0, r1]
    result t0
    0: obj.get r2, r0, 0
    1: int.mul r3, r2, r1
    2: ret r3
}
function f2 "Calculator.array" {
    registers [t1, t2, t3, t0, t0, t4, t0, t0, t0, t3, t0]
    params [r0, r1]
    result t2
    0: builder.new r2
    1: const r3, int 0
    2: arr.len r4, r1
    3: const r10, int 1
    4: jmp @5
    5: int.cmp lt, r5, r3, r4
    6: br r5, @7, @14
    7: arr.get r6, r1, r3
    8: obj.get r7, r0, 0
    9: int.add r8, r6, r7
    10: builder.push r9, r2, r8
    11: jmp @12
    12: int.add r6, r3, r10
    13: jmp @5(r3 <- r6, r2 <- r9)
    14: builder.finish r1, r2
    15: ret r1
}
function f3 "Calculator.text" {
    registers [t1, t5]
    params [r0, r1]
    result t5
    0: ret r1
}
function f4 "main" async {
    registers [t0, t1, t6, t7, t0, t0, t0, t2, t8, t9, t5, t10, t11, t4, t5]
    params []
    result t0
    0: const r0, int 7
    1: call r1, f0, [r0]
    2: call r2, f5, [r1]
    3: const r0, int 6
    4: worker.spawn r3, r2, [r0], false
    5: const r0, int 2
    6: const r4, int 1
    7: const r5, int 2
    8: const r6, int 3
    9: arr.new r7, [r4, r5, r6]
    10: fn.closure r8, f6, []
    11: worker.spawn r9, r8, [r0, r7], true
    12: const r0, int 0
    13: const r10, str "worker value 🦀"
    14: fn.closure r11, f7, []
    15: worker.spawn r12, r11, [r0, r10], true
    16: await r0, r3
    17: const r4, int 42
    18: int.cmp eq, r13, r0, r4
    19: br r13, @20, @39
    20: await r0, r3
    21: const r4, int 42
    22: int.cmp eq, r13, r0, r4
    23: br r13, @24, @38
    24: await r7, r9
    25: const r0, int 2
    26: arr.get r4, r7, r0
    27: const r0, int 5
    28: int.cmp eq, r13, r4, r0
    29: br r13, @30, @37
    30: await r10, r12
    31: const r14, str "worker value 🦀"
    32: str.cmp eq, r13, r10, r14
    33: br r13, @34, @36
    34: const r0, int 42
    35: ret r0
    36: trap confirm, "confirm failed"
    37: trap confirm, "confirm failed"
    38: trap confirm, "confirm failed"
    39: trap confirm, "confirm failed"
}
function f5 "Calculator.calculate$bind#5" {
    registers [t1, t6, t12, t0, t6]
    params [r0]
    result t6
    0: fn.closure r1, f1, [r0]
    1: fn.closure r2, f8, []
    2: obj.get r3, r0, 0
    3: worker.bind r4, r1, r2, [r3]
    4: ret r4
}
function f6 "Calculator.array$worker#6" {
    registers [t0, t2, t1, t2]
    params [r0, r1]
    result t2
    0: call r2, f0, [r0]
    1: call r3, f2, [r2, r1]
    2: ret r3
}
function f7 "Calculator.text$worker#7" {
    registers [t0, t5, t1, t5]
    params [r0, r1]
    result t5
    0: call r2, f0, [r0]
    1: call r3, f3, [r2, r1]
    2: ret r3
}
function f8 "Calculator.calculate$worker#8" {
    registers [t0, t0, t1]
    params [r0, r1]
    result t0
    0: call r2, f0, [r0]
    1: call r0, f1, [r2, r1]
    2: ret r0
}
entry f4
