sparrow 9
module "stream_init_timing" "v1.0.0"
type t0 str
type t1 bool
type t2 role role3
type t3 int
type t4 future t3
type t5 object obj2
type t6 role role2
type t7 array t4
type t8 object obj0
type t9 object obj1
type t10 object obj3
type t11 object obj5
type t12 object obj4
type t13 future t4
type t14 failure
type t15 object obj7
type t16 object obj6
type t17 object obj8
role role0 "Error" builtin {
    method "text" [] -> t0
}
role role1 "io.stream::Closing" {
    method "closed" [] -> t1
    method "close" [] -> void
}
role role2 "collections.sequence::Sequencing<Future<int>>" {
    method "cursor" [] -> t2
    method "size" [] -> t3
    method "item" [t3] -> t4
    method "first" [] -> t4
    method "last" [] -> t4
}
role role3 "collections.iteration::Cursor<Future<int>>" {
    method "available" [] -> t1
    method "item" [] -> t4
    method "successor" [] -> t2
}
role role4 "collections.iteration::Iterating<Future<int>>" {
    method "cursor" [] -> t2
}
role role5 "size::Sizing" {
    method "size" [] -> t3
}
object obj0 "Problem" {
    does role0 [f0]
}
object obj1 "Sources" {
    field "calls" t5
    field "origin" t6
    field "fail" t1
    does role2 [f4, f5, f6, f7, f8]
}
object obj2 "async.atomic::Atomic<int>" {
    does role1 [f2, f3]
}
object obj3 "Work" {
}
object obj4 "async.collect::FutureCompletions<int>" {
    field "origins" t6
}
object obj5 "core::ArraySequence<Future<int>, Future<int>>" {
    field "items" t7
    does role2 [f16, f17, f18, f19, f20]
    does role4 [f16]
    does role5 [f17]
}
object obj6 "async.atomic::AtomicClosed" {
    does role0 [f24]
}
object obj7 "core::ArrayCursor<Future<int>, Future<int>>" {
    field "items" t7
    field "at" t3
    does role3 [f25, f26, f27]
}
object obj8 "async.collect::NoFutures" {
    does role0 [f29]
}
function f0 "Problem.text" {
    registers [t8, t0]
    params [r0]
    result t0
    0: const r1, str "initialization failed"
    1: ret r1
}
function f1 "Problem.new#0" {
    registers [t8]
    params []
    result t8
    0: obj.new r0, obj0, []
    1: ret r0
}
function f2 "async.atomic::Atomic<int>.closed" intrinsic "okkama.atomic.Atomic.closed" [] [] {
    registers [t5]
    params [r0]
    result t1
}
function f3 "async.atomic::Atomic<int>.close" intrinsic "okkama.atomic.Atomic.close" [] [] {
    registers [t5]
    params [r0]
    result void
}
function f4 "Sources.cursor" {
    registers [t9, t6, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 1
    1: call.role r2, r1, role2, 0, []
    2: ret r2
}
function f5 "Sources.size" {
    registers [t9, t5, t5, t3, t3, t3, t1, t8, t6]
    params [r0]
    result t3
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 0
    2: call r3, f14, [r2]
    3: const r4, int 1
    4: int.add r5, r3, r4
    5: call _, f13, [r1, r5]
    6: obj.get r6, r0, 2
    7: br r6, @8, @13
    8: obj.get r1, r0, 0
    9: call r3, f14, [r1]
    10: const r4, int 1
    11: int.cmp eq, r6, r3, r4
    12: jmp @15
    13: const r6, bool false
    14: jmp @15
    15: br r6, @16, @18
    16: call r7, f1, []
    17: throw r7
    18: obj.get r8, r0, 1
    19: call.role r3, r8, role2, 1, []
    20: ret r3
}
function f6 "Sources.item" {
    registers [t9, t3, t6, t4]
    params [r0, r1]
    result t4
    0: obj.get r2, r0, 1
    1: call.role r3, r2, role2, 2, [r1]
    2: ret r3
}
function f7 "Sources.first" {
    registers [t9, t6, t4]
    params [r0]
    result t4
    0: obj.get r1, r0, 1
    1: call.role r2, r1, role2, 3, []
    2: ret r2
}
function f8 "Sources.last" {
    registers [t9, t6, t4]
    params [r0]
    result t4
    0: obj.get r1, r0, 1
    1: call.role r2, r1, role2, 4, []
    2: ret r2
}
function f9 "Sources.new#0" {
    registers [t5, t6, t1, t9]
    params [r0, r1, r2]
    result t9
    0: obj.new r3, obj1, [r0, r1, r2]
    1: ret r3
}
function f10 "Work.new#0" {
    registers [t10]
    params []
    result t10
    0: obj.new r0, obj3, []
    1: ret r0
}
function f11 "Work.run" async {
    registers [t10, t3]
    params [r0]
    result t3
    0: const r1, int 42
    1: ret r1
}
function f12 "main" async {
    registers [t3, t5, t10, t4, t7, t1, t11, t6, t9, t12, t13, t3, t14, t8, t14, t14]
    params []
    result t3
    handler 33 36 @36 r12
    handler 37 39 @62 r14
    0: const r0, int 0
    1: call r1, f15, [r0]
    2: call r2, f10, []
    3: call r3, f11, [r2]
    4: arr.new r4, [r3]
    5: const r5, bool false
    6: call r6, f21, [r4]
    7: role.wrap r7, r6, role2
    8: call r8, f9, [r1, r7, r5]
    9: role.wrap r7, r8, role2
    10: call r9, f22, [r7]
    11: call r10, f23, [r9]
    12: call r0, f14, [r1]
    13: const r11, int 1
    14: int.cmp eq, r5, r0, r11
    15: br r5, @16, @66
    16: await r3, r10
    17: await r0, r3
    18: const r11, int 42
    19: int.cmp eq, r5, r0, r11
    20: br r5, @21, @65
    21: const r0, int 0
    22: call r1, f15, [r0]
    23: call r2, f10, []
    24: call r3, f11, [r2]
    25: arr.new r4, [r3]
    26: const r5, bool true
    27: call r6, f21, [r4]
    28: role.wrap r7, r6, role2
    29: call r8, f9, [r1, r7, r5]
    30: role.wrap r7, r8, role2
    31: call r9, f22, [r7]
    32: jmp @33
    33: call r10, f23, [r9]
    34: const r5, bool false
    35: jmp @39
    36: catch r12, object, 0, r13, @37, @64
    37: const r5, bool true
    38: jmp @39
    39: br r5, @40, @44
    40: call r0, f14, [r1]
    41: const r11, int 1
    42: int.cmp eq, r5, r0, r11
    43: jmp @46
    44: const r5, bool false
    45: jmp @46
    46: br r5, @47, @61
    47: call r10, f23, [r9]
    48: call r0, f14, [r1]
    49: const r11, int 2
    50: int.cmp eq, r5, r0, r11
    51: br r5, @52, @60
    52: await r3, r10
    53: call r0, f14, [r1]
    54: const r11, int 2
    55: int.cmp eq, r5, r0, r11
    56: br r5, @57, @59
    57: await r0, r3
    58: ret r0
    59: trap confirm, "confirm failed"
    60: trap confirm, "confirm failed"
    61: trap confirm, "confirm failed"
    62: fail.caused r15, r14, r12
    63: raise r15
    64: raise r12
    65: trap confirm, "confirm failed"
    66: trap confirm, "confirm failed"
}
function f13 "async.atomic::Atomic<int>.replace" intrinsic "okkama.atomic.Atomic.replace" [obj6] [] {
    registers [t5, t3]
    params [r0, r1]
    result void
}
function f14 "async.atomic::Atomic<int>.value" intrinsic "okkama.atomic.Atomic.value" [obj6] [] {
    registers [t5]
    params [r0]
    result t3
}
function f15 "async.atomic::Atomic<int>.new#0" intrinsic "okkama.atomic.Atomic.new_value" [obj2] [] {
    registers [t3]
    params [r0]
    result t5
}
function f16 "core::ArraySequence<Future<int>, Future<int>>.cursor" {
    registers [t11, t7, t3, t15, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: const r2, int 0
    2: call r3, f28, [r1, r2]
    3: role.wrap r4, r3, role3
    4: ret r4
}
function f17 "core::ArraySequence<Future<int>, Future<int>>.size" {
    registers [t11, t7, t3]
    params [r0]
    result t3
    0: obj.get r1, r0, 0
    1: arr.len r2, r1
    2: ret r2
}
function f18 "core::ArraySequence<Future<int>, Future<int>>.item" {
    registers [t11, t3, t7, t4]
    params [r0, r1]
    result t4
    0: obj.get r2, r0, 0
    1: arr.get r3, r2, r1
    2: ret r3
}
function f19 "core::ArraySequence<Future<int>, Future<int>>.first" {
    registers [t11, t7, t3, t4]
    params [r0]
    result t4
    0: obj.get r1, r0, 0
    1: const r2, int 0
    2: arr.lookup r1, r2, r3, @3, @4
    3: ret r3
    4: trap empty, "Cannot read first item of an empty sequence"
}
function f20 "core::ArraySequence<Future<int>, Future<int>>.last" {
    registers [t11, t7, t3, t3, t3, t4]
    params [r0]
    result t4
    0: obj.get r1, r0, 0
    1: arr.len r2, r1
    2: const r3, int 1
    3: int.sub r4, r2, r3
    4: arr.lookup r1, r4, r5, @5, @6
    5: ret r5
    6: trap empty, "Cannot read last item of an empty sequence"
}
function f21 "core::ArraySequence<Future<int>, Future<int>>.new" {
    registers [t7, t11]
    params [r0]
    result t11
    0: obj.new r1, obj5, [r0]
    1: ret r1
}
function f22 "async.collect::FutureCompletions<int>.new#0" {
    registers [t6, t12]
    params [r0]
    result t12
    0: obj.new r1, obj4, [r0]
    1: ret r1
}
function f23 "async.collect::FutureCompletions<int>.completion" async intrinsic "okkama.collect.FutureCompletions.completion" [obj8] [] {
    registers [t12]
    params [r0]
    result t4
}
function f24 "async.atomic::AtomicClosed.text" {
    registers [t16, t0]
    params [r0]
    result t0
    0: const r1, str "atomic state is closed"
    1: ret r1
}
function f25 "core::ArrayCursor<Future<int>, Future<int>>.available" {
    registers [t15, t7, t3, t3, t1]
    params [r0]
    result t1
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.len r3, r1
    3: int.cmp lt, r4, r2, r3
    4: ret r4
}
function f26 "core::ArrayCursor<Future<int>, Future<int>>.item" {
    registers [t15, t7, t3, t4]
    params [r0]
    result t4
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.lookup r1, r2, r3, @3, @4
    3: ret r3
    4: trap empty, "Array cursor is exhausted"
}
function f27 "core::ArrayCursor<Future<int>, Future<int>>.successor" {
    registers [t15, t7, t3, t4, t3, t3, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.lookup r1, r2, r3, @3, @8
    3: const r4, int 1
    4: int.add r5, r2, r4
    5: call r0, f28, [r1, r5]
    6: role.wrap r6, r0, role3
    7: ret r6
    8: trap empty, "Array cursor is exhausted"
}
function f28 "core::ArrayCursor<Future<int>, Future<int>>.new" {
    registers [t7, t3, t15]
    params [r0, r1]
    result t15
    0: obj.new r2, obj7, [r0, r1]
    1: ret r2
}
function f29 "async.collect::NoFutures.text" {
    registers [t17, t0]
    params [r0]
    result t0
    0: const r1, str "no futures remain"
    1: ret r1
}
entry f12
