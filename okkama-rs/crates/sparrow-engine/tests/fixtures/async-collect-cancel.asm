sparrow 9
module "async_collect_cancel" "v1.0.0"
type t0 role role1
type t1 int
type t2 future t1
type t3 bool
type t4 role role3
type t5 str
type t6 object obj1
type t7 object obj3
type t8 role role0
type t9 array t2
type t10 array t1
type t11 object obj0
type t12 future t3
type t13 failure
type t14 object obj2
type t15 role role2
type t16 future t15
type t17 object obj4
type t18 object obj5
type t19 object obj7
type t20 object obj6
type t21 object obj8
role role0 "collections.sequence::Sequencing<Future<int>>" {
    method "cursor" [] -> t0
    method "size" [] -> t1
    method "item" [t1] -> t2
    method "first" [] -> t2
    method "last" [] -> t2
}
role role1 "collections.iteration::Cursor<Future<int>>" {
    method "available" [] -> t3
    method "item" [] -> t2
    method "successor" [] -> t0
}
role role2 "collections.sequence::Sequencing<int>" {
    method "cursor" [] -> t4
    method "size" [] -> t1
    method "item" [t1] -> t1
    method "first" [] -> t1
    method "last" [] -> t1
}
role role3 "collections.iteration::Cursor<int>" {
    method "available" [] -> t3
    method "item" [] -> t1
    method "successor" [] -> t4
}
role role4 "collections.iteration::Iterating<Future<int>>" {
    method "cursor" [] -> t0
}
role role5 "size::Sizing" {
    method "size" [] -> t1
}
role role6 "Error" builtin {
    method "text" [] -> t5
}
role role7 "collections.iteration::Iterating<int>" {
    method "cursor" [] -> t4
}
object obj0 "Job" {
    field "started" t6
    field "done" t6
}
object obj1 "async.event::Event" {
    field "initial" t3
}
object obj2 "Waiting" {
    field "all" t7
}
object obj3 "async.collect::AllFutures<int>" {
    field "origins" t8
}
object obj4 "core::ArraySequence<Future<int>, Future<int>>" {
    field "items" t9
    does role0 [f9, f10, f11, f12, f13]
    does role4 [f9]
    does role5 [f10]
}
object obj5 "Cancelled" builtin {
    does role6 [f17]
}
object obj6 "core::ArraySequence<int, int>" {
    field "items" t10
    does role2 [f18, f19, f20, f21, f22]
    does role7 [f18]
    does role5 [f19]
}
object obj7 "core::ArrayCursor<Future<int>, Future<int>>" {
    field "items" t9
    field "at" t1
    does role1 [f23, f24, f25]
}
object obj8 "core::ArrayCursor<int, int>" {
    field "items" t10
    field "at" t1
    does role3 [f27, f28, f29]
}
function f0 "Job.new#0" {
    registers [t6, t6, t11]
    params [r0, r1]
    result t11
    0: obj.new r2, obj0, [r0, r1]
    1: ret r2
}
function f1 "Job.run" async {
    registers [t11, t6, t5, t12, t3, t1, t13, t13, t13]
    params [r0]
    result t1
    handler 0 7 @11 r6
    handler 11 14 @14 r7
    0: obj.get r1, r0, 0
    1: call _, f5, [r1]
    2: const r2, str "9223372036854775807"
    3: call r3, f6, [r2]
    4: await r4, r3
    5: const r5, int 0
    6: jmp @7
    7: obj.get r1, r0, 1
    8: call _, f5, [r1]
    9: jmp @10
    10: ret r5
    11: obj.get r1, r0, 1
    12: call _, f5, [r1]
    13: jmp @16
    14: fail.secondary r8, r6, r7
    15: jmp @16(r6 <- r8)
    16: raise r6
}
function f2 "Waiting.new#0" {
    registers [t7, t14]
    params [r0]
    result t14
    0: obj.new r1, obj2, [r0]
    1: ret r1
}
function f3 "Waiting.run" async {
    registers [t14, t7, t16, t15, t1]
    params [r0]
    result t1
    0: obj.get r1, r0, 0
    1: call r2, f7, [r1]
    2: await r3, r2
    3: const r4, int 0
    4: ret r4
}
function f4 "main" async {
    registers [t6, t6, t6, t6, t11, t2, t2, t9, t17, t8, t7, t14, t3, t1, t13, t18, t13, t13]
    params []
    result t1
    handler 24 27 @27 r14
    handler 28 30 @44 r16
    0: call r0, f8, []
    1: call r1, f8, []
    2: call r2, f8, []
    3: call r3, f8, []
    4: call r4, f0, [r0, r2]
    5: call r5, f1, [r4]
    6: call r4, f0, [r1, r3]
    7: call r6, f1, [r4]
    8: arr.new r7, [r5, r6]
    9: call r8, f14, [r7]
    10: role.wrap r9, r8, role0
    11: call r10, f15, [r9]
    12: call r11, f2, [r10]
    13: call r5, f3, [r11]
    14: fut.spawn r6, r5
    15: call r12, f16, [r0]
    16: br r12, @17, @19
    17: call r12, f16, [r1]
    18: jmp @21
    19: const r12, bool false
    20: jmp @21
    21: br r12, @22, @47
    22: fut cancel, _, r6
    23: jmp @24
    24: await r13, r6
    25: const r12, bool false
    26: jmp @30
    27: catch r14, object, 5, r15, @28, @46
    28: const r12, bool true
    29: jmp @30
    30: br r12, @31, @33
    31: call r12, f16, [r2]
    32: jmp @35
    33: const r12, bool false
    34: jmp @35
    35: br r12, @36, @38
    36: call r12, f16, [r3]
    37: jmp @40
    38: const r12, bool false
    39: jmp @40
    40: br r12, @41, @43
    41: const r13, int 42
    42: ret r13
    43: trap confirm, "confirm failed"
    44: fail.caused r17, r16, r14
    45: raise r17
    46: raise r14
    47: trap confirm, "confirm failed"
}
function f5 "async.event::Event.set" intrinsic "okkama.event.Event.set" [] [] {
    registers [t6]
    params [r0]
    result void
}
function f6 "runtime.time::sleep_nanoseconds" async native "runtime/time@1::sleep_nanoseconds/1" {
    registers [t5]
    params [r0]
    result t3
}
function f7 "async.collect::AllFutures<int>.results" async intrinsic "okkama.collect.AllFutures.results" [obj6] [] {
    registers [t7]
    params [r0]
    result t15
}
function f8 "async.event::Event.new#0" intrinsic "okkama.event.Event.new" [obj1] [] {
    registers []
    params []
    result t6
}
function f9 "core::ArraySequence<Future<int>, Future<int>>.cursor" {
    registers [t17, t9, t1, t19, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 0
    1: const r2, int 0
    2: call r3, f26, [r1, r2]
    3: role.wrap r4, r3, role1
    4: ret r4
}
function f10 "core::ArraySequence<Future<int>, Future<int>>.size" {
    registers [t17, t9, t1]
    params [r0]
    result t1
    0: obj.get r1, r0, 0
    1: arr.len r2, r1
    2: ret r2
}
function f11 "core::ArraySequence<Future<int>, Future<int>>.item" {
    registers [t17, t1, t9, t2]
    params [r0, r1]
    result t2
    0: obj.get r2, r0, 0
    1: arr.get r3, r2, r1
    2: ret r3
}
function f12 "core::ArraySequence<Future<int>, Future<int>>.first" {
    registers [t17, t9, t1, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: const r2, int 0
    2: arr.lookup r1, r2, r3, @3, @4
    3: ret r3
    4: trap empty, "Cannot read first item of an empty sequence"
}
function f13 "core::ArraySequence<Future<int>, Future<int>>.last" {
    registers [t17, t9, t1, t1, t1, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: arr.len r2, r1
    2: const r3, int 1
    3: int.sub r4, r2, r3
    4: arr.lookup r1, r4, r5, @5, @6
    5: ret r5
    6: trap empty, "Cannot read last item of an empty sequence"
}
function f14 "core::ArraySequence<Future<int>, Future<int>>.new" {
    registers [t9, t17]
    params [r0]
    result t17
    0: obj.new r1, obj4, [r0]
    1: ret r1
}
function f15 "async.collect::AllFutures<int>.new#0" {
    registers [t8, t7]
    params [r0]
    result t7
    0: obj.new r1, obj3, [r0]
    1: ret r1
}
function f16 "async.event::Event.ready" intrinsic "okkama.event.Event.ready" [] [] {
    registers [t6]
    params [r0]
    result t3
}
function f17 "Cancelled.text" {
    registers [t18, t5]
    params [r0]
    result t5
    0: const r1, str "Operation was cancelled"
    1: ret r1
}
function f18 "core::ArraySequence<int, int>.cursor" {
    registers [t20, t10, t1, t21, t4]
    params [r0]
    result t4
    0: obj.get r1, r0, 0
    1: const r2, int 0
    2: call r3, f30, [r1, r2]
    3: role.wrap r4, r3, role3
    4: ret r4
}
function f19 "core::ArraySequence<int, int>.size" {
    registers [t20, t10, t1]
    params [r0]
    result t1
    0: obj.get r1, r0, 0
    1: arr.len r2, r1
    2: ret r2
}
function f20 "core::ArraySequence<int, int>.item" {
    registers [t20, t1, t10, t1]
    params [r0, r1]
    result t1
    0: obj.get r2, r0, 0
    1: arr.get r3, r2, r1
    2: ret r3
}
function f21 "core::ArraySequence<int, int>.first" {
    registers [t20, t10, t1]
    params [r0]
    result t1
    0: obj.get r1, r0, 0
    1: const r2, int 0
    2: arr.lookup r1, r2, r2, @3, @4
    3: ret r2
    4: trap empty, "Cannot read first item of an empty sequence"
}
function f22 "core::ArraySequence<int, int>.last" {
    registers [t20, t10, t1, t1, t1]
    params [r0]
    result t1
    0: obj.get r1, r0, 0
    1: arr.len r2, r1
    2: const r3, int 1
    3: int.sub r4, r2, r3
    4: arr.lookup r1, r4, r2, @5, @6
    5: ret r2
    6: trap empty, "Cannot read last item of an empty sequence"
}
function f23 "core::ArrayCursor<Future<int>, Future<int>>.available" {
    registers [t19, t9, t1, t1, t3]
    params [r0]
    result t3
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.len r3, r1
    3: int.cmp lt, r4, r2, r3
    4: ret r4
}
function f24 "core::ArrayCursor<Future<int>, Future<int>>.item" {
    registers [t19, t9, t1, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.lookup r1, r2, r3, @3, @4
    3: ret r3
    4: trap empty, "Array cursor is exhausted"
}
function f25 "core::ArrayCursor<Future<int>, Future<int>>.successor" {
    registers [t19, t9, t1, t2, t1, t1, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.lookup r1, r2, r3, @3, @8
    3: const r4, int 1
    4: int.add r5, r2, r4
    5: call r0, f26, [r1, r5]
    6: role.wrap r6, r0, role1
    7: ret r6
    8: trap empty, "Array cursor is exhausted"
}
function f26 "core::ArrayCursor<Future<int>, Future<int>>.new" {
    registers [t9, t1, t19]
    params [r0, r1]
    result t19
    0: obj.new r2, obj7, [r0, r1]
    1: ret r2
}
function f27 "core::ArrayCursor<int, int>.available" {
    registers [t21, t10, t1, t1, t3]
    params [r0]
    result t3
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.len r3, r1
    3: int.cmp lt, r4, r2, r3
    4: ret r4
}
function f28 "core::ArrayCursor<int, int>.item" {
    registers [t21, t10, t1]
    params [r0]
    result t1
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.lookup r1, r2, r2, @3, @4
    3: ret r2
    4: trap empty, "Array cursor is exhausted"
}
function f29 "core::ArrayCursor<int, int>.successor" {
    registers [t21, t10, t1, t1, t1, t4]
    params [r0]
    result t4
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: arr.lookup r1, r2, r3, @3, @8
    3: const r3, int 1
    4: int.add r4, r2, r3
    5: call r0, f30, [r1, r4]
    6: role.wrap r5, r0, role3
    7: ret r5
    8: trap empty, "Array cursor is exhausted"
}
function f30 "core::ArrayCursor<int, int>.new" {
    registers [t10, t1, t21]
    params [r0, r1]
    result t21
    0: obj.new r2, obj8, [r0, r1]
    1: ret r2
}
entry f4
