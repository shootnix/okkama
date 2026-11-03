sparrow 9
module "async_scope" "v1.0.0"
type t0 str
type t1 bool
type t2 int
type t3 future t2
type t4 object obj1
type t5 object obj4
type t6 object obj7
type t7 role role2
type t8 fn [t7] -> t3
type t9 future t0
type t10 fn [t7] -> t9
type t11 object obj0
type t12 future t1
type t13 failure
type t14 object obj2
type t15 object obj3
type t16 object obj5
type t17 object obj6
type t18 object obj8
type t19 object obj9
type t20 object obj10
type t21 object obj11
type t22 object obj12
role role0 "Error" builtin {
    method "text" [] -> t0
}
role role1 "io.stream::Closing" {
    method "closed" [] -> t1
    method "close" [] -> void
}
role role2 "async.scope::Supervising<int>" {
    method "start" [t3] -> void
    method "active" [] -> t2
    method "cancel" [] -> void
}
object obj0 "Child" {
    field "done" t4
    field "delay" t0
}
object obj1 "async.event::Event" {
    field "initial" t1
}
object obj2 "Problem" {
    does role0 [f2]
}
object obj3 "Body" {
    field "done" t4
    field "calls" t5
    field "fail" t1
}
object obj4 "async.atomic::Atomic<int>" {
    does role1 [f4, f5]
}
object obj5 "WaitingBody" {
    field "done" t4
}
object obj6 "Waiting" {
    field "scope" t6
}
object obj7 "async.scope::FutureScope<int, int>" {
    field "action" t8
}
object obj8 "async.scope::FutureScope<int, string>" {
    field "action" t10
}
object obj9 "Cancelled" builtin {
    does role0 [f24]
}
object obj10 "async.atomic::AtomicClosed" {
    does role0 [f25]
}
object obj11 "async.scope::FutureGroup<int>" {
    does role2 [f26, f27, f28]
}
object obj12 "async.scope::FinishedFutureGroup" {
    does role0 [f29]
}
function f0 "Child.new#0" {
    registers [t4, t0, t11]
    params [r0, r1]
    result t11
    0: obj.new r2, obj0, [r0, r1]
    1: ret r2
}
function f1 "Child.run" async {
    registers [t11, t0, t12, t1, t2, t4, t13, t13, t13]
    params [r0]
    result t2
    handler 0 5 @9 r6
    handler 9 12 @12 r7
    0: obj.get r1, r0, 1
    1: call r2, f13, [r1]
    2: await r3, r2
    3: const r4, int 21
    4: jmp @5
    5: obj.get r5, r0, 0
    6: call _, f14, [r5]
    7: jmp @8
    8: ret r4
    9: obj.get r5, r0, 0
    10: call _, f14, [r5]
    11: jmp @14
    12: fail.secondary r8, r6, r7
    13: jmp @14(r6 <- r8)
    14: raise r6
}
function f2 "Problem.text" {
    registers [t14, t0]
    params [r0]
    result t0
    0: const r1, str "body failed"
    1: ret r1
}
function f3 "Problem.new#0" {
    registers [t14]
    params []
    result t14
    0: obj.new r0, obj2, []
    1: ret r0
}
function f4 "async.atomic::Atomic<int>.closed" intrinsic "okkama.atomic.Atomic.closed" [] [] {
    registers [t5]
    params [r0]
    result t1
}
function f5 "async.atomic::Atomic<int>.close" intrinsic "okkama.atomic.Atomic.close" [] [] {
    registers [t5]
    params [r0]
    result void
}
function f6 "Body.new#0" {
    registers [t4, t5, t1, t15]
    params [r0, r1, r2]
    result t15
    0: obj.new r3, obj3, [r0, r1, r2]
    1: ret r3
}
function f7 "Body.run" async {
    registers [t15, t7, t5, t5, t2, t2, t2, t4, t1, t0, t11, t3, t14]
    params [r0, r1]
    result t0
    0: obj.get r2, r0, 1
    1: obj.get r3, r0, 1
    2: call r4, f16, [r3]
    3: const r5, int 1
    4: int.add r6, r4, r5
    5: call _, f15, [r2, r6]
    6: obj.get r7, r0, 0
    7: obj.get r8, r0, 2
    8: br r8, @9, @11
    9: const r9, str "9223372036854775807"
    10: jmp @13
    11: const r9, str "1000000"
    12: jmp @13
    13: call r10, f0, [r7, r9]
    14: call r11, f1, [r10]
    15: call.role _, r1, role2, 0, [r11]
    16: obj.get r8, r0, 2
    17: br r8, @18, @20
    18: call r12, f3, []
    19: throw r12
    20: const r9, str "answer"
    21: ret r9
}
function f8 "WaitingBody.new#0" {
    registers [t4, t16]
    params [r0]
    result t16
    0: obj.new r1, obj5, [r0]
    1: ret r1
}
function f9 "WaitingBody.run" async {
    registers [t16, t7, t4, t0, t11, t3, t2]
    params [r0, r1]
    result t2
    0: obj.get r2, r0, 0
    1: const r3, str "9223372036854775807"
    2: call r4, f0, [r2, r3]
    3: call r5, f1, [r4]
    4: call.role _, r1, role2, 0, [r5]
    5: const r6, int 42
    6: ret r6
}
function f10 "Waiting.new#0" {
    registers [t6, t17]
    params [r0]
    result t17
    0: obj.new r1, obj6, [r0]
    1: ret r1
}
function f11 "Waiting.run" async {
    registers [t17, t6, t3, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: call r2, f17, [r1]
    2: await r3, r2
    3: ret r3
}
function f12 "main" async {
    registers [t4, t2, t5, t1, t15, t10, t18, t9, t9, t2, t0, t0, t13, t14, t16, t8, t6, t17, t3, t3, t19, t13, t13]
    params []
    result t2
    handler 45 49 @49 r12
    handler 50 52 @93 r21
    handler 74 77 @77 r12
    handler 78 80 @89 r21
    0: call r0, f18, []
    1: const r1, int 0
    2: call r2, f19, [r1]
    3: const r3, bool false
    4: call r4, f6, [r0, r2, r3]
    5: fn.closure r5, f7, [r4]
    6: call r6, f20, [r5]
    7: call r7, f21, [r6]
    8: call r8, f21, [r6]
    9: val.cmp eq, r3, r7, r8
    10: br r3, @11, @15
    11: call r1, f16, [r2]
    12: const r9, int 0
    13: int.cmp eq, r3, r1, r9
    14: jmp @17
    15: const r3, bool false
    16: jmp @17
    17: br r3, @18, @98
    18: await r10, r7
    19: const r11, str "answer"
    20: str.cmp eq, r3, r10, r11
    21: br r3, @22, @24
    22: call r3, f22, [r0]
    23: jmp @26
    24: const r3, bool false
    25: jmp @26
    26: br r3, @27, @97
    27: call r7, f21, [r6]
    28: await r10, r7
    29: const r11, str "answer"
    30: str.cmp eq, r3, r10, r11
    31: br r3, @32, @36
    32: call r1, f16, [r2]
    33: const r9, int 1
    34: int.cmp eq, r3, r1, r9
    35: jmp @38
    36: const r3, bool false
    37: jmp @38
    38: br r3, @39, @96
    39: call r0, f18, []
    40: const r3, bool true
    41: call r4, f6, [r0, r2, r3]
    42: fn.closure r5, f7, [r4]
    43: call r6, f20, [r5]
    44: jmp @45
    45: call r7, f21, [r6]
    46: await r10, r7
    47: const r3, bool false
    48: jmp @52
    49: catch r12, object, 2, r13, @50, @95
    50: const r3, bool true
    51: jmp @52
    52: br r3, @53, @55
    53: call r3, f22, [r0]
    54: jmp @57
    55: const r3, bool false
    56: jmp @57
    57: br r3, @58, @62
    58: call r1, f16, [r2]
    59: const r9, int 2
    60: int.cmp eq, r3, r1, r9
    61: jmp @64
    62: const r3, bool false
    63: jmp @64
    64: br r3, @65, @92
    65: call r0, f18, []
    66: call r14, f8, [r0]
    67: fn.closure r15, f9, [r14]
    68: call r16, f23, [r15]
    69: call r17, f10, [r16]
    70: call r18, f11, [r17]
    71: fut.spawn r19, r18
    72: fut cancel, _, r19
    73: jmp @74
    74: await r1, r19
    75: const r3, bool false
    76: jmp @80
    77: catch r12, object, 9, r20, @78, @91
    78: const r3, bool true
    79: jmp @80
    80: br r3, @81, @83
    81: call r3, f22, [r0]
    82: jmp @85
    83: const r3, bool false
    84: jmp @85
    85: br r3, @86, @88
    86: const r1, int 42
    87: ret r1
    88: trap confirm, "confirm failed"
    89: fail.caused r22, r21, r12
    90: raise r22
    91: raise r12
    92: trap confirm, "confirm failed"
    93: fail.caused r22, r21, r12
    94: raise r22
    95: raise r12
    96: trap confirm, "confirm failed"
    97: trap confirm, "confirm failed"
    98: trap confirm, "confirm failed"
}
function f13 "runtime.time::sleep_nanoseconds" async native "runtime/time@1::sleep_nanoseconds/1" {
    registers [t0]
    params [r0]
    result t1
}
function f14 "async.event::Event.set" intrinsic "okkama.event.Event.set" [] [] {
    registers [t4]
    params [r0]
    result void
}
function f15 "async.atomic::Atomic<int>.replace" intrinsic "okkama.atomic.Atomic.replace" [obj10] [] {
    registers [t5, t2]
    params [r0, r1]
    result void
}
function f16 "async.atomic::Atomic<int>.value" intrinsic "okkama.atomic.Atomic.value" [obj10] [] {
    registers [t5]
    params [r0]
    result t2
}
function f17 "async.scope::FutureScope<int, int>.result" async intrinsic "okkama.scope.FutureScope.result" [obj11] [] {
    registers [t6]
    params [r0]
    result t2
}
function f18 "async.event::Event.new#0" intrinsic "okkama.event.Event.new" [obj1] [] {
    registers []
    params []
    result t4
}
function f19 "async.atomic::Atomic<int>.new#0" intrinsic "okkama.atomic.Atomic.new_value" [obj4] [] {
    registers [t2]
    params [r0]
    result t5
}
function f20 "async.scope::FutureScope<int, string>.new#0" {
    registers [t10, t18]
    params [r0]
    result t18
    0: obj.new r1, obj8, [r0]
    1: ret r1
}
function f21 "async.scope::FutureScope<int, string>.result" async intrinsic "okkama.scope.FutureScope.result" [obj11] [] {
    registers [t18]
    params [r0]
    result t0
}
function f22 "async.event::Event.ready" intrinsic "okkama.event.Event.ready" [] [] {
    registers [t4]
    params [r0]
    result t1
}
function f23 "async.scope::FutureScope<int, int>.new#0" {
    registers [t8, t6]
    params [r0]
    result t6
    0: obj.new r1, obj7, [r0]
    1: ret r1
}
function f24 "Cancelled.text" {
    registers [t19, t0]
    params [r0]
    result t0
    0: const r1, str "Operation was cancelled"
    1: ret r1
}
function f25 "async.atomic::AtomicClosed.text" {
    registers [t20, t0]
    params [r0]
    result t0
    0: const r1, str "atomic state is closed"
    1: ret r1
}
function f26 "async.scope::FutureGroup<int>.start" intrinsic "okkama.scope.FutureGroup.start" [obj12] [] {
    registers [t21, t3]
    params [r0, r1]
    result void
}
function f27 "async.scope::FutureGroup<int>.active" intrinsic "okkama.scope.FutureGroup.active" [] [] {
    registers [t21]
    params [r0]
    result t2
}
function f28 "async.scope::FutureGroup<int>.cancel" intrinsic "okkama.scope.FutureGroup.cancel" [] [] {
    registers [t21]
    params [r0]
    result void
}
function f29 "async.scope::FinishedFutureGroup.text" {
    registers [t22, t0]
    params [r0]
    result t0
    0: const r1, str "future group is already finishing"
    1: ret r1
}
entry f12
