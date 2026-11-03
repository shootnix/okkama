sparrow 9
module "async_system_clock" "v1.0.0"
type t0 object obj3
type t1 object obj5
type t2 object obj4
type t3 future void
type t4 enum enum0
type t5 str
type t6 object obj12
type t7 object obj1
type t8 bool
type t9 rat
type t10 int
type t11 future t10
type t12 role role0
type t13 object obj0
type t14 future t8
type t15 failure
type t16 object obj2
type t17 enum enum1
type t18 object obj6
type t19 object obj7
type t20 object obj8
type t21 object obj9
type t22 object obj10
type t23 object obj11
type t24 object obj13
type t25 object obj14
type t26 object obj15
enum enum0 "compare::Order" ["less", "equal", "greater"]
enum enum1 "time.duration::DurationUnit" ["nanosecond", "microsecond", "millisecond", "second", "minute", "hour", "day"]
role role0 "time.clock::Timing" {
    method "instant" [] -> t0
    method "monotonic_instant" [] -> t1
    method "wait" [t2] -> t3
}
role role1 "compare::Comparing<Duration>" {
    method "order" [t2] -> t4
}
role role2 "compare::Comparing<Instant>" {
    method "order" [t0] -> t4
}
role role3 "compare::Comparing<MonotonicInstant>" {
    method "order" [t1] -> t4
}
role role4 "Error" builtin {
    method "text" [] -> t5
}
role role5 "compare::Comparing<BigInt>" {
    method "order" [t6] -> t4
}
object obj0 "Work" {
    field "done" t7
}
object obj1 "async.event::Event" {
    field "initial" t8
}
object obj2 "time.clock::SystemClock" {
    does role0 [f8, f9, f10]
}
object obj3 "time.clock::Instant" {
    field "origin" t2
    does role2 [f6]
}
object obj4 "time.duration::Duration" {
    field "origin" t9
    does role1 [f5]
}
object obj5 "time.clock::MonotonicInstant" {
    field "origin" t2
    field "domain" t5
    does role3 [f7]
}
object obj6 "NativeFailure" builtin {
    field "domain" t5
    field "code" t5
    field "operation" t5
    field "message" t5
    does role4 [f17]
}
object obj7 "async.timeout::Within<int>" {
    field "origin" t11
    field "limit" t2
    field "clock" t12
}
object obj8 "async.timeout::TimedOut" {
    does role4 [f21]
}
object obj9 "async.timeout::Before<int>" {
    field "origin" t11
    field "deadline" t1
    field "clock" t12
}
object obj10 "time.clock::IncompatibleClock" {
    does role4 [f28]
}
object obj11 "time.clock::InvalidWait" {
    field "message" t5
    does role4 [f35]
}
object obj12 "numbers.bigint::BigInt" {
    field "origin" t5
    does role5 [f38]
}
object obj13 "time.duration::InvalidDuration" {
    field "message" t5
    does role4 [f40]
}
object obj14 "error::InvalidArgument" {
    field "message" t5
    does role4 [f44]
}
object obj15 "numbers.bigint::InvalidBigInt" {
    does role4 [f54]
}
function f0 "Work.new#0" {
    registers [t7, t13]
    params [r0]
    result t13
    0: obj.new r1, obj0, [r0]
    1: ret r1
}
function f1 "Work.run" async {
    registers [t13, t5, t14, t8, t10, t7, t15, t15, t15]
    params [r0]
    result t10
    handler 0 5 @9 r6
    handler 9 12 @12 r7
    0: const r1, str "9223372036854775807"
    1: call r2, f3, [r1]
    2: await r3, r2
    3: const r4, int 42
    4: jmp @5
    5: obj.get r5, r0, 0
    6: call _, f4, [r5]
    7: jmp @8
    8: ret r4
    9: obj.get r5, r0, 0
    10: call _, f4, [r5]
    11: jmp @14
    12: fail.secondary r8, r6, r7
    13: jmp @14(r6 <- r8)
    14: raise r6
}
function f2 "main" async {
    registers [t16, t1, t10, t9, t17, t2, t3, t1, t2, t9, t8, t0, t5, t5, t15, t18, t7, t13, t11, t12, t19, t11, t20, t21, t15, t15]
    params []
    result t10
    handler 27 31 @31 r14
    handler 32 36 @98 r24
    handler 43 47 @47 r14
    handler 48 50 @94 r24
    handler 69 73 @73 r14
    handler 74 76 @90 r24
    0: call r0, f11, []
    1: call r1, f9, [r0]
    2: const r2, int 1
    3: cvt r3, r2
    4: enum.case r4, enum1, 2
    5: call r5, f12, [r3, r4]
    6: call r6, f10, [r0, r5]
    7: await _, r6
    8: call r7, f9, [r0]
    9: call r8, f13, [r7, r1]
    10: call r3, f14, [r8]
    11: call r9, f14, [r5]
    12: rat.cmp ge, r10, r3, r9
    13: br r10, @14, @103
    14: call r11, f8, [r0]
    15: call r8, f15, [r11]
    16: call r3, f14, [r8]
    17: const r2, int 0
    18: cvt r9, r2
    19: rat.cmp gt, r10, r3, r9
    20: br r10, @21, @102
    21: const r10, bool false
    22: call r12, f16, [r10]
    23: const r10, bool false
    24: call r13, f16, [r10]
    25: str.cmp eq, r10, r12, r13
    26: br r10, @27, @101
    27: const r10, bool true
    28: call r12, f16, [r10]
    29: const r10, bool false
    30: jmp @36
    31: catch r14, object, 6, r15, @32, @100
    32: call r12, f18, [r15]
    33: const r13, str "permission_denied"
    34: str.cmp eq, r10, r12, r13
    35: jmp @36
    36: br r10, @37, @97
    37: call r16, f19, []
    38: call r17, f0, [r16]
    39: call r18, f1, [r17]
    40: role.wrap r19, r0, role0
    41: call r20, f20, [r18, r5, r19]
    42: jmp @43
    43: call r21, f22, [r20]
    44: await r2, r21
    45: const r10, bool false
    46: jmp @50
    47: catch r14, object, 8, r22, @48, @96
    48: const r10, bool true
    49: jmp @50
    50: br r10, @51, @53
    51: fut cancelled, r10, r18
    52: jmp @55
    53: const r10, bool false
    54: jmp @55
    55: br r10, @56, @58
    56: call r10, f23, [r16]
    57: jmp @60
    58: const r10, bool false
    59: jmp @60
    60: br r10, @61, @93
    61: call r16, f19, []
    62: call r17, f0, [r16]
    63: call r18, f1, [r17]
    64: call r1, f9, [r0]
    65: call r7, f24, [r1, r5]
    66: role.wrap r19, r0, role0
    67: call r23, f25, [r18, r7, r19]
    68: jmp @69
    69: call r21, f26, [r23]
    70: await r2, r21
    71: const r10, bool false
    72: jmp @76
    73: catch r14, object, 8, r22, @74, @92
    74: const r10, bool true
    75: jmp @76
    76: br r10, @77, @79
    77: fut cancelled, r10, r18
    78: jmp @81
    79: const r10, bool false
    80: jmp @81
    81: br r10, @82, @84
    82: call r10, f23, [r16]
    83: jmp @86
    84: const r10, bool false
    85: jmp @86
    86: br r10, @87, @89
    87: const r2, int 42
    88: ret r2
    89: trap confirm, "confirm failed"
    90: fail.caused r25, r24, r14
    91: raise r25
    92: raise r14
    93: trap confirm, "confirm failed"
    94: fail.caused r25, r24, r14
    95: raise r25
    96: raise r14
    97: trap confirm, "confirm failed"
    98: fail.caused r25, r24, r14
    99: raise r25
    100: raise r14
    101: trap confirm, "confirm failed"
    102: trap confirm, "confirm failed"
    103: trap confirm, "confirm failed"
}
function f3 "runtime.time::sleep_nanoseconds" async native "runtime/time@1::sleep_nanoseconds/1" {
    registers [t5]
    params [r0]
    result t8
}
function f4 "async.event::Event.set" intrinsic "okkama.event.Event.set" [] [] {
    registers [t7]
    params [r0]
    result void
}
function f5 "time.duration::Duration.order" {
    registers [t2, t2, t9, t9, t8, t4]
    params [r0, r1]
    result t4
    0: obj.get r2, r0, 0
    1: call r3, f14, [r1]
    2: rat.cmp lt, r4, r2, r3
    3: br r4, @4, @6
    4: enum.case r5, enum0, 0
    5: jmp @12
    6: rat.cmp eq, r4, r2, r3
    7: br r4, @8, @10
    8: enum.case r5, enum0, 1
    9: jmp @12
    10: enum.case r5, enum0, 2
    11: jmp @12
    12: ret r5
}
function f6 "time.clock::Instant.order" {
    registers [t0, t0, t2, t2, t4]
    params [r0, r1]
    result t4
    0: obj.get r2, r0, 0
    1: call r3, f15, [r1]
    2: call r4, f5, [r2, r3]
    3: ret r4
}
function f7 "time.clock::MonotonicInstant.order" {
    registers [t1, t1, t5, t5, t8, t2, t2, t4, t22]
    params [r0, r1]
    result t4
    0: obj.get r2, r0, 1
    1: call r3, f27, [r1]
    2: str.cmp eq, r4, r2, r3
    3: br r4, @4, @8
    4: obj.get r5, r0, 0
    5: call r6, f30, [r1]
    6: call r7, f5, [r5, r6]
    7: ret r7
    8: call r8, f29, []
    9: throw r8
}
function f8 "time.clock::SystemClock.instant" {
    registers [t16, t10, t9, t17, t2, t0]
    params [r0]
    result t0
    0: call r1, f31, []
    1: cvt r2, r1
    2: enum.case r3, enum1, 0
    3: call r4, f12, [r2, r3]
    4: call r5, f32, [r4]
    5: ret r5
}
function f9 "time.clock::SystemClock.monotonic_instant" {
    registers [t16, t10, t9, t17, t2, t8, t5, t1]
    params [r0]
    result t1
    0: call r1, f33, []
    1: cvt r2, r1
    2: enum.case r3, enum1, 0
    3: call r4, f12, [r2, r3]
    4: const r5, bool false
    5: call r6, f16, [r5]
    6: call r7, f34, [r4, r6]
    7: ret r7
}
function f10 "time.clock::SystemClock.wait" async {
    registers [t16, t2, t9, t10, t9, t8, t6, t5, t14, t23]
    params [r0, r1]
    result void
    0: call r2, f14, [r1]
    1: const r3, int 0
    2: cvt r4, r3
    3: rat.cmp ge, r5, r2, r4
    4: br r5, @5, @10
    5: call r6, f37, [r1]
    6: call r7, f39, [r6]
    7: call r8, f3, [r7]
    8: await r5, r8
    9: ret _
    10: const r7, str "wait duration must not be negative"
    11: call r9, f36, [r7]
    12: throw r9
}
function f11 "time.clock::SystemClock.new#0" {
    registers [t16]
    params []
    result t16
    0: obj.new r0, obj2, []
    1: ret r0
}
function f12 "time.duration::Duration.new#0" {
    registers [t9, t17, t17, t8, t10, t10, t9, t9, t5, t5, t2, t24]
    params [r0, r1]
    result t2
    0: enum.case r2, enum1, 0
    1: enum.cmp eq, r3, r1, r2
    2: br r3, @3, @7
    3: const r4, int 1
    4: const r5, int 1000000000
    5: rat.new r6, r4, r5
    6: jmp @48
    7: enum.case r2, enum1, 1
    8: enum.cmp eq, r3, r1, r2
    9: br r3, @10, @14
    10: const r4, int 1
    11: const r5, int 1000000
    12: rat.new r6, r4, r5
    13: jmp @48
    14: enum.case r2, enum1, 2
    15: enum.cmp eq, r3, r1, r2
    16: br r3, @17, @21
    17: const r4, int 1
    18: const r5, int 1000
    19: rat.new r6, r4, r5
    20: jmp @48
    21: enum.case r2, enum1, 3
    22: enum.cmp eq, r3, r1, r2
    23: br r3, @24, @27
    24: const r4, int 1
    25: cvt r6, r4
    26: jmp @48
    27: enum.case r2, enum1, 4
    28: enum.cmp eq, r3, r1, r2
    29: br r3, @30, @33
    30: const r4, int 60
    31: cvt r6, r4
    32: jmp @48
    33: enum.case r2, enum1, 5
    34: enum.cmp eq, r3, r1, r2
    35: br r3, @36, @39
    36: const r4, int 3600
    37: cvt r6, r4
    38: jmp @48
    39: enum.case r2, enum1, 6
    40: enum.cmp eq, r3, r1, r2
    41: br r3, @42, @45
    42: const r4, int 86400
    43: cvt r6, r4
    44: jmp @48
    45: const r4, int 1
    46: cvt r6, r4
    47: jmp @48
    48: rat.mul r7, r0, r6
    49: const r4, int 1000000000
    50: cvt r0, r4
    51: rat.mul r6, r7, r0
    52: rat.part r8, r6, true
    53: const r9, str "1"
    54: str.cmp eq, r3, r8, r9
    55: br r3, @56, @58
    56: obj.new r10, obj4, [r7]
    57: ret r10
    58: const r8, str "duration must contain an integral number of nanoseconds"
    59: call r11, f41, [r8]
    60: throw r11
}
function f13 "time.clock::MonotonicInstant.minus" {
    registers [t1, t1, t5, t5, t8, t2, t2, t2, t22]
    params [r0, r1]
    result t2
    0: obj.get r2, r0, 1
    1: call r3, f27, [r1]
    2: str.cmp eq, r4, r2, r3
    3: br r4, @4, @8
    4: obj.get r5, r0, 0
    5: call r6, f30, [r1]
    6: call r7, f42, [r5, r6]
    7: ret r7
    8: call r8, f29, []
    9: throw r8
}
function f14 "time.duration::Duration.seconds" {
    registers [t2, t9]
    params [r0]
    result t9
    0: obj.get r1, r0, 0
    1: ret r1
}
function f15 "time.clock::Instant.unix_duration" {
    registers [t0, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: ret r1
}
function f16 "runtime.time::clock_domain" native "runtime/time@1::clock_domain/1" {
    registers [t8]
    params [r0]
    result t5
}
function f17 "NativeFailure.text" {
    registers [t18, t5]
    params [r0]
    result t5
    0: obj.get r1, r0, 3
    1: ret r1
}
function f18 "NativeFailure.code" {
    registers [t18, t5]
    params [r0]
    result t5
    0: obj.get r1, r0, 1
    1: ret r1
}
function f19 "async.event::Event.new#0" intrinsic "okkama.event.Event.new" [obj1] [] {
    registers []
    params []
    result t7
}
function f20 "async.timeout::Within<int>.new#0" {
    registers [t11, t2, t12, t9, t10, t9, t8, t19, t5, t25]
    params [r0, r1, r2]
    result t19
    0: call r3, f14, [r1]
    1: const r4, int 0
    2: cvt r5, r4
    3: rat.cmp ge, r6, r3, r5
    4: br r6, @5, @7
    5: obj.new r7, obj7, [r0, r1, r2]
    6: ret r7
    7: const r8, str "timeout duration cannot be negative"
    8: call r9, f45, [r8]
    9: throw r9
}
function f21 "async.timeout::TimedOut.text" {
    registers [t20, t5]
    params [r0]
    result t5
    0: const r1, str "future timed out"
    1: ret r1
}
function f22 "async.timeout::Within<int>.result" async intrinsic "okkama.timeout.Within.result" [obj8] [f46, f47] {
    registers [t19]
    params [r0]
    result t10
}
function f23 "async.event::Event.ready" intrinsic "okkama.event.Event.ready" [] [] {
    registers [t7]
    params [r0]
    result t8
}
function f24 "time.clock::MonotonicInstant.plus" {
    registers [t1, t2, t2, t2, t5]
    params [r0, r1]
    result t1
    0: obj.get r2, r0, 0
    1: call r3, f48, [r2, r1]
    2: obj.get r4, r0, 1
    3: call r0, f34, [r3, r4]
    4: ret r0
}
function f25 "async.timeout::Before<int>.new#0" {
    registers [t11, t1, t12, t21]
    params [r0, r1, r2]
    result t21
    0: obj.new r3, obj9, [r0, r1, r2]
    1: ret r3
}
function f26 "async.timeout::Before<int>.result" async intrinsic "okkama.timeout.Before.result" [obj8] [f49, f50, f46, f47] {
    registers [t21]
    params [r0]
    result t10
}
function f27 "time.clock::MonotonicInstant.domain" {
    registers [t1, t5]
    params [r0]
    result t5
    0: obj.get r1, r0, 1
    1: ret r1
}
function f28 "time.clock::IncompatibleClock.text" {
    registers [t22, t5]
    params [r0]
    result t5
    0: const r1, str "incompatible monotonic clock domains"
    1: ret r1
}
function f29 "time.clock::IncompatibleClock.new#0" {
    registers [t22]
    params []
    result t22
    0: obj.new r0, obj10, []
    1: ret r0
}
function f30 "time.clock::MonotonicInstant.reading" {
    registers [t1, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: ret r1
}
function f31 "runtime.time::wall_clock" native "runtime/time@1::wall_clock/0" {
    registers []
    params []
    result t10
}
function f32 "time.clock::Instant.new#0" {
    registers [t2, t0]
    params [r0]
    result t0
    0: obj.new r1, obj3, [r0]
    1: ret r1
}
function f33 "runtime.time::monotonic_clock" native "runtime/time@1::monotonic_clock/0" {
    registers []
    params []
    result t10
}
function f34 "time.clock::MonotonicInstant.new#1" {
    registers [t2, t5, t10, t10, t8, t1, t22]
    params [r0, r1]
    result t1
    0: str.len r2, r1
    1: const r3, int 0
    2: int.cmp gt, r4, r2, r3
    3: br r4, @4, @8
    4: str.len r2, r1
    5: const r3, int 128
    6: int.cmp le, r4, r2, r3
    7: jmp @10
    8: const r4, bool false
    9: jmp @10
    10: br r4, @11, @13
    11: obj.new r5, obj5, [r0, r1]
    12: ret r5
    13: call r6, f29, []
    14: throw r6
}
function f35 "time.clock::InvalidWait.text" {
    registers [t23, t5]
    params [r0]
    result t5
    0: obj.get r1, r0, 0
    1: ret r1
}
function f36 "time.clock::InvalidWait.new#0" {
    registers [t5, t23]
    params [r0]
    result t23
    0: obj.new r1, obj11, [r0]
    1: ret r1
}
function f37 "time.duration::Duration.nanoseconds" {
    registers [t2, t9, t10, t9, t9, t5, t6]
    params [r0]
    result t6
    0: obj.get r1, r0, 0
    1: const r2, int 1000000000
    2: cvt r3, r2
    3: rat.mul r4, r1, r3
    4: rat.part r5, r4, false
    5: call r6, f51, [r5]
    6: ret r6
}
function f38 "numbers.bigint::BigInt.order" {
    registers [t6, t6, t5, t5, t10, t10, t8, t4]
    params [r0, r1]
    result t4
    0: obj.get r2, r0, 0
    1: call r3, f39, [r1]
    2: call r4, f52, [r2, r3]
    3: const r5, int 0
    4: int.cmp lt, r6, r4, r5
    5: br r6, @6, @8
    6: enum.case r7, enum0, 0
    7: jmp @16
    8: const r5, int 0
    9: int.cmp gt, r6, r4, r5
    10: br r6, @11, @13
    11: enum.case r7, enum0, 2
    12: jmp @15
    13: enum.case r7, enum0, 1
    14: jmp @15
    15: jmp @16
    16: ret r7
}
function f39 "numbers.bigint::BigInt.string" {
    registers [t6, t5]
    params [r0]
    result t5
    0: obj.get r1, r0, 0
    1: ret r1
}
function f40 "time.duration::InvalidDuration.text" {
    registers [t24, t5]
    params [r0]
    result t5
    0: obj.get r1, r0, 0
    1: ret r1
}
function f41 "time.duration::InvalidDuration.new#0" {
    registers [t5, t24]
    params [r0]
    result t24
    0: obj.new r1, obj13, [r0]
    1: ret r1
}
function f42 "time.duration::Duration.minus" {
    registers [t2, t2, t9, t9, t9, t17]
    params [r0, r1]
    result t2
    0: obj.get r2, r0, 0
    1: call r3, f14, [r1]
    2: rat.sub r4, r2, r3
    3: enum.case r5, enum1, 3
    4: call r0, f12, [r4, r5]
    5: ret r0
}
function f43 "error::InvalidArgument.default.message" {
    registers [t5]
    params []
    result t5
    0: const r0, str "invalid argument"
    1: ret r0
}
function f44 "error::InvalidArgument.text" {
    registers [t25, t5]
    params [r0]
    result t5
    0: obj.get r1, r0, 0
    1: ret r1
}
function f45 "error::InvalidArgument.new#1" {
    registers [t5, t5, t25]
    params [r0]
    result t25
    0: call r1, f43, []
    1: obj.new r2, obj14, [r0]
    2: ret r2
}
function f46 "member.seconds#46" {
    registers [t2, t9]
    params [r0]
    result t9
    0: call r1, f14, [r0]
    1: ret r1
}
function f47 "member.wait#47" {
    registers [t12, t2, t3]
    params [r0, r1]
    result t3
    0: call.role r2, r0, role0, 2, [r1]
    1: ret r2
}
function f48 "time.duration::Duration.plus" {
    registers [t2, t2, t9, t9, t9, t17]
    params [r0, r1]
    result t2
    0: obj.get r2, r0, 0
    1: call r3, f14, [r1]
    2: rat.add r4, r2, r3
    3: enum.case r5, enum1, 3
    4: call r0, f12, [r4, r5]
    5: ret r0
}
function f49 "member.monotonic_instant#49" {
    registers [t12, t1]
    params [r0]
    result t1
    0: call.role r1, r0, role0, 1, []
    1: ret r1
}
function f50 "member.minus#50" {
    registers [t1, t1, t2]
    params [r0, r1]
    result t2
    0: call r2, f13, [r0, r1]
    1: ret r2
}
function f51 "numbers.bigint::BigInt.new#0" {
    registers [t5, t8, t5, t6, t26]
    params [r0]
    result t6
    0: call r1, f53, [r0]
    1: br r1, @2, @5
    2: call r2, f56, [r0]
    3: obj.new r3, obj12, [r2]
    4: ret r3
    5: call r4, f55, []
    6: throw r4
}
function f52 "runtime.bigint::bigint_order" native "runtime/bigint@1::bigint_order/2" {
    registers [t5, t5]
    params [r0, r1]
    result t10
}
function f53 "runtime.bigint::bigint_valid" native "runtime/bigint@1::bigint_valid/1" {
    registers [t5]
    params [r0]
    result t8
}
function f54 "numbers.bigint::InvalidBigInt.text" {
    registers [t26, t5]
    params [r0]
    result t5
    0: const r1, str "invalid arbitrary-precision integer"
    1: ret r1
}
function f55 "numbers.bigint::InvalidBigInt.new#0" {
    registers [t26]
    params []
    result t26
    0: obj.new r0, obj15, []
    1: ret r0
}
function f56 "runtime.bigint::bigint_canonical" native "runtime/bigint@1::bigint_canonical/1" {
    registers [t5]
    params [r0]
    result t5
}
entry f2
