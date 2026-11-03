sparrow 9
module "async_group" "v1.0.0"
type t0 int
type t1 future t0
type t2 str
type t3 object obj1
type t4 bool
type t5 object obj3
type t6 object obj0
type t7 future t3
type t8 failure
type t9 object obj2
type t10 future t5
type t11 object obj4
type t12 object obj5
role role0 "async.scope::Supervising<int>" {
    method "start" [t1] -> void
    method "active" [] -> t0
    method "cancel" [] -> void
}
role role1 "Error" builtin {
    method "text" [] -> t2
}
object obj0 "Work" {
    field "begin" t3
    field "gate" t3
    field "done" t3
}
object obj1 "async.event::Event" {
    field "initial" t4
}
object obj2 "Waiting" {
    field "group" t5
}
object obj3 "async.scope::FutureGroup<int>" {
    does role0 [f2, f3, f4]
}
object obj4 "async.scope::FinishedFutureGroup" {
    does role1 [f10]
}
object obj5 "Cancelled" builtin {
    does role1 [f15]
}
function f0 "Work.new#0" {
    registers [t3, t3, t3, t6]
    params [r0, r1, r2]
    result t6
    0: obj.new r3, obj0, [r0, r1, r2]
    1: ret r3
}
function f1 "Work.run" async {
    registers [t6, t3, t7, t0, t8, t8, t8]
    params [r0]
    result t0
    handler 3 8 @12 r4
    handler 12 15 @15 r5
    0: obj.get r1, r0, 0
    1: call _, f8, [r1]
    2: jmp @3
    3: obj.get r1, r0, 1
    4: call r2, f9, [r1]
    5: await r1, r2
    6: const r3, int 21
    7: jmp @8
    8: obj.get r1, r0, 2
    9: call _, f8, [r1]
    10: jmp @11
    11: ret r3
    12: obj.get r1, r0, 2
    13: call _, f8, [r1]
    14: jmp @17
    15: fail.secondary r6, r4, r5
    16: jmp @17(r4 <- r6)
    17: raise r4
}
function f2 "async.scope::FutureGroup<int>.start" intrinsic "okkama.scope.FutureGroup.start" [obj4] [] {
    registers [t5, t1]
    params [r0, r1]
    result void
}
function f3 "async.scope::FutureGroup<int>.active" intrinsic "okkama.scope.FutureGroup.active" [] [] {
    registers [t5]
    params [r0]
    result t0
}
function f4 "async.scope::FutureGroup<int>.cancel" intrinsic "okkama.scope.FutureGroup.cancel" [] [] {
    registers [t5]
    params [r0]
    result void
}
function f5 "Waiting.new#0" {
    registers [t5, t9]
    params [r0]
    result t9
    0: obj.new r1, obj2, [r0]
    1: ret r1
}
function f6 "Waiting.run" async {
    registers [t9, t5, t10, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 0
    1: call r2, f11, [r1]
    2: await r1, r2
    3: const r3, int 42
    4: ret r3
}
function f7 "main" async {
    registers [t5, t3, t3, t3, t6, t1, t4, t4, t0, t0, t10, t5, t8, t11, t9, t1, t12, t8, t8]
    params []
    result t0
    handler 40 45 @45 r12
    handler 46 48 @85 r17
    handler 59 62 @62 r12
    handler 63 65 @81 r17
    0: call r0, f12, []
    1: call r1, f13, []
    2: call r2, f13, []
    3: call r3, f13, []
    4: call r4, f0, [r1, r2, r3]
    5: call r5, f1, [r4]
    6: call _, f2, [r0, r5]
    7: call _, f2, [r0, r5]
    8: call r6, f14, [r1]
    9: br r6, @10, @13
    10: call r6, f14, [r3]
    11: bool.not r7, r6
    12: jmp @15
    13: const r6, bool false
    14: jmp @15(r7 <- r6)
    15: br r7, @16, @20
    16: call r8, f3, [r0]
    17: const r9, int 1
    18: int.cmp eq, r6, r8, r9
    19: jmp @22
    20: const r6, bool false
    21: jmp @22
    22: br r6, @23, @89
    23: call _, f4, [r0]
    24: call _, f4, [r0]
    25: call r10, f11, [r0]
    26: await r11, r10
    27: call r10, f11, [r0]
    28: await r11, r10
    29: call r6, f14, [r3]
    30: br r6, @31, @35
    31: call r8, f3, [r0]
    32: const r9, int 0
    33: int.cmp eq, r6, r8, r9
    34: jmp @37
    35: const r6, bool false
    36: jmp @37
    37: br r6, @38, @88
    38: call _, f2, [r0, r5]
    39: jmp @40
    40: call r4, f0, [r1, r2, r3]
    41: call r5, f1, [r4]
    42: call _, f2, [r0, r5]
    43: const r6, bool false
    44: jmp @48
    45: catch r12, object, 4, r13, @46, @87
    46: const r6, bool true
    47: jmp @48
    48: br r6, @49, @84
    49: call r0, f12, []
    50: call r3, f13, []
    51: call r4, f0, [r1, r2, r3]
    52: call r5, f1, [r4]
    53: call _, f2, [r0, r5]
    54: call r14, f5, [r0]
    55: call r5, f6, [r14]
    56: fut.spawn r15, r5
    57: fut cancel, _, r15
    58: jmp @59
    59: await r8, r15
    60: const r6, bool false
    61: jmp @65
    62: catch r12, object, 5, r16, @63, @83
    63: const r6, bool true
    64: jmp @65
    65: br r6, @66, @68
    66: call r6, f14, [r3]
    67: jmp @70
    68: const r6, bool false
    69: jmp @70
    70: br r6, @71, @75
    71: call r8, f3, [r0]
    72: const r9, int 0
    73: int.cmp eq, r6, r8, r9
    74: jmp @77
    75: const r6, bool false
    76: jmp @77
    77: br r6, @78, @80
    78: const r8, int 42
    79: ret r8
    80: trap confirm, "confirm failed"
    81: fail.caused r18, r17, r12
    82: raise r18
    83: raise r12
    84: trap confirm, "confirm failed"
    85: fail.caused r18, r17, r12
    86: raise r18
    87: raise r12
    88: trap confirm, "confirm failed"
    89: trap confirm, "confirm failed"
}
function f8 "async.event::Event.set" intrinsic "okkama.event.Event.set" [] [] {
    registers [t3]
    params [r0]
    result void
}
function f9 "async.event::Event.wait" async intrinsic "okkama.event.Event.wait" [] [] {
    registers [t3]
    params [r0]
    result t3
}
function f10 "async.scope::FinishedFutureGroup.text" {
    registers [t11, t2]
    params [r0]
    result t2
    0: const r1, str "future group is already finishing"
    1: ret r1
}
function f11 "async.scope::FutureGroup<int>.wait" async intrinsic "okkama.scope.FutureGroup.wait" [] [] {
    registers [t5]
    params [r0]
    result t5
}
function f12 "async.scope::FutureGroup<int>.new#0" intrinsic "okkama.scope.FutureGroup.new" [obj3] [] {
    registers []
    params []
    result t5
}
function f13 "async.event::Event.new#0" intrinsic "okkama.event.Event.new" [obj1] [] {
    registers []
    params []
    result t3
}
function f14 "async.event::Event.ready" intrinsic "okkama.event.Event.ready" [] [] {
    registers [t3]
    params [r0]
    result t4
}
function f15 "Cancelled.text" {
    registers [t12, t2]
    params [r0]
    result t2
    0: const r1, str "Operation was cancelled"
    1: ret r1
}
entry f7
