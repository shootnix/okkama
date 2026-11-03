sparrow 9
module "async_semaphore" "v1.0.0"
type t0 bool
type t1 str
type t2 object obj1
type t3 object obj2
type t4 int
type t5 object obj0
type t6 object obj3
type t7 future t6
type t8 failure
type t9 future t4
type t10 object obj4
type t11 object obj5
role role0 "io.stream::Closing" {
    method "closed" [] -> t0
    method "close" [] -> void
}
role role1 "Error" builtin {
    method "text" [] -> t1
}
object obj0 "Job" {
    field "pool" t2
    field "before" t3
    field "after" t3
}
object obj1 "async.semaphore::Semaphore" {
    field "amount" t4
}
object obj2 "async.event::Event" {
    field "initial" t0
}
object obj3 "async.semaphore::Permit" {
    does role0 [f4, f5]
}
object obj4 "Cancelled" builtin {
    does role1 [f12]
}
object obj5 "async.semaphore::InvalidPermitCount" {
    field "amount" t4
    does role1 [f13]
}
function f0 "Job.new#0" {
    registers [t2, t3, t3, t5]
    params [r0, r1, r2]
    result t5
    0: obj.new r3, obj0, [r0, r1, r2]
    1: ret r3
}
function f1 "Job.run" async {
    registers [t5, t2, t7, t6, t3, t0, t4, t8, t8, t8]
    params [r0]
    result t4
    handler 4 7 @15 r7
    handler 7 11 @15 r7
    handler 14 15 @15 r7
    handler 15 17 @17 r8
    0: obj.get r1, r0, 0
    1: call r2, f3, [r1]
    2: await r3, r2
    3: jmp @4
    4: obj.get r4, r0, 1
    5: call r5, f6, [r4]
    6: br r5, @7, @14
    7: obj.get r4, r0, 2
    8: call _, f7, [r4]
    9: const r6, int 21
    10: jmp @11
    11: call _, f5, [r3]
    12: jmp @13
    13: ret r6
    14: trap confirm, "confirm failed"
    15: call _, f5, [r3]
    16: jmp @19
    17: fail.secondary r9, r7, r8
    18: jmp @19(r7 <- r9)
    19: raise r7
}
function f2 "main" async {
    registers [t4, t2, t7, t6, t6, t4, t0, t3, t3, t3, t3, t5, t9, t9, t9, t9, t0, t4, t4, t8, t10, t11, t8, t8]
    params []
    result t4
    handler 52 55 @55 r19
    handler 56 58 @100 r22
    handler 82 86 @86 r19
    handler 87 89 @93 r22
    0: const r0, int 2
    1: call r1, f8, [r0]
    2: call r2, f3, [r1]
    3: await r3, r2
    4: call r2, f3, [r1]
    5: await r4, r2
    6: call r0, f9, [r1]
    7: const r5, int 0
    8: int.cmp eq, r6, r0, r5
    9: br r6, @10, @106
    10: const r6, bool true
    11: call r7, f10, [r6]
    12: call r8, f11, []
    13: call r9, f11, []
    14: call r10, f11, []
    15: call r11, f0, [r1, r7, r8]
    16: call r12, f1, [r11]
    17: fut.spawn r13, r12
    18: call r11, f0, [r1, r7, r10]
    19: call r12, f1, [r11]
    20: fut.spawn r14, r12
    21: call r11, f0, [r1, r8, r9]
    22: call r12, f1, [r11]
    23: fut.spawn r15, r12
    24: fut cancel, _, r14
    25: call _, f5, [r3]
    26: call _, f5, [r4]
    27: call r0, f9, [r1]
    28: const r5, int 0
    29: int.cmp ge, r6, r0, r5
    30: br r6, @31, @35
    31: call r0, f9, [r1]
    32: const r5, int 2
    33: int.cmp le, r6, r0, r5
    34: jmp @37
    35: const r6, bool false
    36: jmp @37
    37: br r6, @38, @105
    38: await r0, r13
    39: await r5, r15
    40: call r6, f6, [r9]
    41: br r6, @42, @45
    42: call r6, f6, [r10]
    43: bool.not r16, r6
    44: jmp @47
    45: const r6, bool false
    46: jmp @47(r16 <- r6)
    47: br r16, @48, @104
    48: call r17, f9, [r1]
    49: const r18, int 2
    50: int.cmp eq, r6, r17, r18
    51: br r6, @52, @103
    52: await r17, r14
    53: const r6, bool false
    54: jmp @58
    55: catch r19, object, 4, r20, @56, @102
    56: const r6, bool true
    57: jmp @58
    58: br r6, @59, @99
    59: call _, f5, [r3]
    60: call r6, f4, [r3]
    61: br r6, @62, @66
    62: call r17, f9, [r1]
    63: const r18, int 2
    64: int.cmp eq, r6, r17, r18
    65: jmp @68
    66: const r6, bool false
    67: jmp @68
    68: br r6, @69, @98
    69: const r17, int 9223372036854775807
    70: call r1, f8, [r17]
    71: call r2, f3, [r1]
    72: await r3, r2
    73: call r17, f9, [r1]
    74: const r18, int 9223372036854775806
    75: int.cmp eq, r6, r17, r18
    76: br r6, @77, @97
    77: call _, f5, [r3]
    78: call r17, f9, [r1]
    79: const r18, int 9223372036854775807
    80: int.cmp eq, r6, r17, r18
    81: br r6, @82, @96
    82: const r17, int 0
    83: call r1, f8, [r17]
    84: const r6, bool false
    85: jmp @89
    86: catch r19, object, 5, r21, @87, @95
    87: const r6, bool true
    88: jmp @89
    89: br r6, @90, @92
    90: int.add r17, r0, r5
    91: ret r17
    92: trap confirm, "confirm failed"
    93: fail.caused r23, r22, r19
    94: raise r23
    95: raise r19
    96: trap confirm, "confirm failed"
    97: trap confirm, "confirm failed"
    98: trap confirm, "confirm failed"
    99: trap confirm, "confirm failed"
    100: fail.caused r23, r22, r19
    101: raise r23
    102: raise r19
    103: trap confirm, "confirm failed"
    104: trap confirm, "confirm failed"
    105: trap confirm, "confirm failed"
    106: trap confirm, "confirm failed"
}
function f3 "async.semaphore::Semaphore.permit" async intrinsic "okkama.semaphore.Semaphore.permit" [obj3] [] {
    registers [t2]
    params [r0]
    result t6
}
function f4 "async.semaphore::Permit.closed" intrinsic "okkama.semaphore.Permit.closed" [] [] {
    registers [t6]
    params [r0]
    result t0
}
function f5 "async.semaphore::Permit.close" intrinsic "okkama.semaphore.Permit.close" [] [] {
    registers [t6]
    params [r0]
    result void
}
function f6 "async.event::Event.ready" intrinsic "okkama.event.Event.ready" [] [] {
    registers [t3]
    params [r0]
    result t0
}
function f7 "async.event::Event.set" intrinsic "okkama.event.Event.set" [] [] {
    registers [t3]
    params [r0]
    result void
}
function f8 "async.semaphore::Semaphore.new#0" {
    registers [t4, t4, t0, t2, t11]
    params [r0]
    result t2
    0: const r1, int 0
    1: int.cmp gt, r2, r0, r1
    2: br r2, @3, @5
    3: obj.new r3, obj1, [r0]
    4: ret r3
    5: call r4, f14, [r0]
    6: throw r4
}
function f9 "async.semaphore::Semaphore.permits" intrinsic "okkama.semaphore.Semaphore.permits" [] [] {
    registers [t2]
    params [r0]
    result t4
}
function f10 "async.event::Event.new#1" intrinsic "okkama.event.Event.new_ready" [obj2] [] {
    registers [t0]
    params [r0]
    result t3
}
function f11 "async.event::Event.new#0" intrinsic "okkama.event.Event.new" [obj2] [] {
    registers []
    params []
    result t3
}
function f12 "Cancelled.text" {
    registers [t10, t1]
    params [r0]
    result t1
    0: const r1, str "Operation was cancelled"
    1: ret r1
}
function f13 "async.semaphore::InvalidPermitCount.text" {
    registers [t11, t1]
    params [r0]
    result t1
    0: const r1, str "semaphore permit count must be positive"
    1: ret r1
}
function f14 "async.semaphore::InvalidPermitCount.new#0" {
    registers [t4, t11]
    params [r0]
    result t11
    0: obj.new r1, obj5, [r0]
    1: ret r1
}
entry f2
