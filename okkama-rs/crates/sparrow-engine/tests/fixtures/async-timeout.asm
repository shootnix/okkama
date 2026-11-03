sparrow 9
module "async_timeout" "v1.0.0"
type t0 bool
type t1 object obj3
type t2 object obj5
type t3 object obj4
type t4 future void
type t5 enum enum0
type t6 str
type t7 object obj1
type t8 object obj2
type t9 rat
type t10 object obj9
type t11 int
type t12 future t11
type t13 role role1
type t14 object obj11
type t15 object obj0
type t16 enum enum1
type t17 future t0
type t18 failure
type t19 object obj6
type t20 object obj7
type t21 object obj8
type t22 object obj10
type t23 object obj12
type t24 object obj13
type t25 object obj14
type t26 object obj15
type t27 object obj17
type t28 object obj16
enum enum0 "compare::Order" ["less", "equal", "greater"]
enum enum1 "time.duration::DurationUnit" ["nanosecond", "microsecond", "millisecond", "second", "minute", "hour", "day"]
role role0 "io.stream::Closing" {
    method "closed" [] -> t0
    method "close" [] -> void
}
role role1 "time.clock::Timing" {
    method "instant" [] -> t1
    method "monotonic_instant" [] -> t2
    method "wait" [t3] -> t4
}
role role2 "compare::Comparing<Duration>" {
    method "order" [t3] -> t5
}
role role3 "compare::Comparing<Instant>" {
    method "order" [t1] -> t5
}
role role4 "compare::Comparing<MonotonicInstant>" {
    method "order" [t2] -> t5
}
role role5 "Error" builtin {
    method "text" [] -> t6
}
object obj0 "Clock" {
    field "calls" t7
    field "done" t8
    field "delay" t6
    does role1 [f5, f6, f7]
}
object obj1 "async.atomic::Atomic<int>" {
    does role0 [f0, f1]
}
object obj2 "async.event::Event" {
    field "initial" t0
}
object obj3 "time.clock::Instant" {
    field "origin" t3
    does role3 [f3]
}
object obj4 "time.duration::Duration" {
    field "origin" t9
    does role2 [f2]
}
object obj5 "time.clock::MonotonicInstant" {
    field "origin" t3
    field "domain" t6
    does role4 [f4]
}
object obj6 "Problem" {
    does role5 [f9]
}
object obj7 "BrokenClock" {
    does role1 [f11, f12, f13]
}
object obj8 "Waiting" {
    field "origin" t10
}
object obj9 "async.timeout::Within<int>" {
    field "origin" t12
    field "limit" t3
    field "clock" t13
}
object obj10 "Work" {
    field "done" t8
    field "delay" t6
}
object obj11 "time.clock::IncompatibleClock" {
    does role5 [f23]
}
object obj12 "async.timeout::TimedOut" {
    does role5 [f37]
}
object obj13 "async.timeout::Before<int>" {
    field "origin" t12
    field "deadline" t2
    field "clock" t13
}
object obj14 "Cancelled" builtin {
    does role5 [f41]
}
object obj15 "time.duration::InvalidDuration" {
    field "message" t6
    does role5 [f42]
}
object obj16 "async.atomic::AtomicClosed" {
    does role5 [f44]
}
object obj17 "error::InvalidArgument" {
    field "message" t6
    does role5 [f48]
}
function f0 "async.atomic::Atomic<int>.closed" intrinsic "okkama.atomic.Atomic.closed" [] [] {
    registers [t7]
    params [r0]
    result t0
}
function f1 "async.atomic::Atomic<int>.close" intrinsic "okkama.atomic.Atomic.close" [] [] {
    registers [t7]
    params [r0]
    result void
}
function f2 "time.duration::Duration.order" {
    registers [t3, t3, t9, t9, t0, t5]
    params [r0, r1]
    result t5
    0: obj.get r2, r0, 0
    1: call r3, f20, [r1]
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
function f3 "time.clock::Instant.order" {
    registers [t1, t1, t3, t3, t5]
    params [r0, r1]
    result t5
    0: obj.get r2, r0, 0
    1: call r3, f21, [r1]
    2: call r4, f2, [r2, r3]
    3: ret r4
}
function f4 "time.clock::MonotonicInstant.order" {
    registers [t2, t2, t6, t6, t0, t3, t3, t5, t14]
    params [r0, r1]
    result t5
    0: obj.get r2, r0, 1
    1: call r3, f22, [r1]
    2: str.cmp eq, r4, r2, r3
    3: br r4, @4, @8
    4: obj.get r5, r0, 0
    5: call r6, f25, [r1]
    6: call r7, f2, [r5, r6]
    7: ret r7
    8: call r8, f24, []
    9: throw r8
}
function f5 "Clock.instant" {
    registers [t15, t11, t9, t16, t3, t1]
    params [r0]
    result t1
    0: const r1, int 0
    1: cvt r2, r1
    2: enum.case r3, enum1, 3
    3: call r4, f26, [r2, r3]
    4: call r5, f27, [r4]
    5: ret r5
}
function f6 "Clock.monotonic_instant" {
    registers [t15, t11, t9, t16, t3, t2]
    params [r0]
    result t2
    0: const r1, int 10
    1: cvt r2, r1
    2: enum.case r3, enum1, 3
    3: call r4, f26, [r2, r3]
    4: call r5, f28, [r4]
    5: ret r5
}
function f7 "Clock.wait" async {
    registers [t15, t3, t7, t7, t11, t11, t11, t6, t17, t0, t8, t18, t18, t18]
    params [r0, r1]
    result void
    handler 7 11 @15 r11
    handler 15 18 @18 r12
    0: obj.get r2, r0, 0
    1: obj.get r3, r0, 0
    2: call r4, f30, [r3]
    3: const r5, int 1
    4: int.add r6, r4, r5
    5: call _, f29, [r2, r6]
    6: jmp @7
    7: obj.get r7, r0, 2
    8: call r8, f31, [r7]
    9: await r9, r8
    10: jmp @11
    11: obj.get r10, r0, 1
    12: call _, f32, [r10]
    13: jmp @14
    14: ret _
    15: obj.get r10, r0, 1
    16: call _, f32, [r10]
    17: jmp @20
    18: fail.secondary r13, r11, r12
    19: jmp @20(r11 <- r13)
    20: raise r11
}
function f8 "Clock.new#0" {
    registers [t7, t8, t6, t15]
    params [r0, r1, r2]
    result t15
    0: obj.new r3, obj0, [r0, r1, r2]
    1: ret r3
}
function f9 "Problem.text" {
    registers [t19, t6]
    params [r0]
    result t6
    0: const r1, str "clock failed"
    1: ret r1
}
function f10 "Problem.new#0" {
    registers [t19]
    params []
    result t19
    0: obj.new r0, obj6, []
    1: ret r0
}
function f11 "BrokenClock.instant" {
    registers [t20, t19]
    params [r0]
    result t1
    0: call r1, f10, []
    1: throw r1
}
function f12 "BrokenClock.monotonic_instant" {
    registers [t20, t19]
    params [r0]
    result t2
    0: call r1, f10, []
    1: throw r1
}
function f13 "BrokenClock.wait" async {
    registers [t20, t3, t19]
    params [r0, r1]
    result void
    0: call r2, f10, []
    1: throw r2
}
function f14 "BrokenClock.new#0" {
    registers [t20]
    params []
    result t20
    0: obj.new r0, obj7, []
    1: ret r0
}
function f15 "Waiting.new#0" {
    registers [t10, t21]
    params [r0]
    result t21
    0: obj.new r1, obj8, [r0]
    1: ret r1
}
function f16 "Waiting.run" async {
    registers [t21, t10, t12, t11]
    params [r0]
    result t11
    0: obj.get r1, r0, 0
    1: call r2, f33, [r1]
    2: await r3, r2
    3: ret r3
}
function f17 "Work.new#0" {
    registers [t8, t6, t22]
    params [r0, r1]
    result t22
    0: obj.new r2, obj10, [r0, r1]
    1: ret r2
}
function f18 "Work.run" async {
    registers [t22, t6, t17, t0, t11, t8, t18, t18, t18]
    params [r0]
    result t11
    handler 0 5 @9 r6
    handler 9 12 @12 r7
    0: obj.get r1, r0, 1
    1: call r2, f31, [r1]
    2: await r3, r2
    3: const r4, int 42
    4: jmp @5
    5: obj.get r5, r0, 0
    6: call _, f32, [r5]
    7: jmp @8
    8: ret r4
    9: obj.get r5, r0, 0
    10: call _, f32, [r5]
    11: jmp @14
    12: fail.secondary r8, r6, r7
    13: jmp @14(r6 <- r8)
    14: raise r6
}
function f19 "main" async {
    registers [t11, t7, t8, t6, t15, t9, t16, t3, t3, t8, t22, t12, t13, t10, t12, t0, t11, t8, t18, t23, t2, t24, t20, t19, t21, t12, t25, t18, t18]
    params []
    result t11
    handler 42 46 @46 r18
    handler 47 49 @220 r27
    handler 104 108 @108 r18
    handler 109 111 @215 r27
    handler 130 134 @134 r18
    handler 135 137 @211 r27
    handler 151 155 @155 r18
    handler 156 158 @207 r27
    handler 178 181 @181 r18
    handler 182 184 @203 r27
    0: const r0, int 0
    1: call r1, f34, [r0]
    2: call r2, f35, []
    3: const r3, str "9223372036854775807"
    4: call r4, f8, [r1, r2, r3]
    5: const r0, int 0
    6: cvt r5, r0
    7: enum.case r6, enum1, 3
    8: call r7, f26, [r5, r6]
    9: const r0, int 1
    10: cvt r5, r0
    11: enum.case r6, enum1, 3
    12: call r8, f26, [r5, r6]
    13: call r9, f35, []
    14: const r3, str "0"
    15: call r10, f17, [r9, r3]
    16: call r11, f18, [r10]
    17: role.wrap r12, r4, role1
    18: call r13, f36, [r11, r7, r12]
    19: call r11, f33, [r13]
    20: call r14, f33, [r13]
    21: val.cmp eq, r15, r11, r14
    22: br r15, @23, @224
    23: call r11, f33, [r13]
    24: await r0, r11
    25: const r16, int 42
    26: int.cmp eq, r15, r0, r16
    27: br r15, @28, @32
    28: call r0, f30, [r1]
    29: const r16, int 0
    30: int.cmp eq, r15, r0, r16
    31: jmp @34
    32: const r15, bool false
    33: jmp @34
    34: br r15, @35, @223
    35: call r17, f35, []
    36: const r3, str "9223372036854775807"
    37: call r10, f17, [r17, r3]
    38: call r11, f18, [r10]
    39: role.wrap r12, r4, role1
    40: call r13, f36, [r11, r7, r12]
    41: jmp @42
    42: call r14, f33, [r13]
    43: await r0, r14
    44: const r15, bool false
    45: jmp @49
    46: catch r18, object, 12, r19, @47, @222
    47: const r15, bool true
    48: jmp @49
    49: br r15, @50, @52
    50: call r15, f38, [r17]
    51: jmp @54
    52: const r15, bool false
    53: jmp @54
    54: br r15, @55, @57
    55: fut cancelled, r15, r11
    56: jmp @59
    57: const r15, bool false
    58: jmp @59
    59: br r15, @60, @64
    60: call r0, f30, [r1]
    61: const r16, int 0
    62: int.cmp eq, r15, r0, r16
    63: jmp @66
    64: const r15, bool false
    65: jmp @66
    66: br r15, @67, @219
    67: const r3, str "1000000"
    68: call r10, f17, [r9, r3]
    69: call r11, f18, [r10]
    70: role.wrap r12, r4, role1
    71: call r13, f36, [r11, r8, r12]
    72: call r11, f33, [r13]
    73: await r0, r11
    74: const r16, int 42
    75: int.cmp eq, r15, r0, r16
    76: br r15, @77, @81
    77: call r0, f30, [r1]
    78: const r16, int 1
    79: int.cmp eq, r15, r0, r16
    80: jmp @83
    81: const r15, bool false
    82: jmp @83
    83: br r15, @84, @86
    84: call r15, f38, [r2]
    85: jmp @88
    86: const r15, bool false
    87: jmp @88
    88: br r15, @89, @218
    89: call r2, f35, []
    90: const r3, str "0"
    91: call r4, f8, [r1, r2, r3]
    92: call r9, f35, []
    93: const r3, str "9223372036854775807"
    94: call r10, f17, [r9, r3]
    95: call r11, f18, [r10]
    96: const r0, int 11
    97: cvt r5, r0
    98: enum.case r6, enum1, 3
    99: call r7, f26, [r5, r6]
    100: call r20, f28, [r7]
    101: role.wrap r12, r4, role1
    102: call r21, f39, [r11, r20, r12]
    103: jmp @104
    104: call r14, f40, [r21]
    105: await r0, r14
    106: const r15, bool false
    107: jmp @111
    108: catch r18, object, 12, r19, @109, @217
    109: const r15, bool true
    110: jmp @111
    111: br r15, @112, @114
    112: fut cancelled, r15, r11
    113: jmp @116
    114: const r15, bool false
    115: jmp @116
    116: br r15, @117, @119
    117: call r15, f38, [r2]
    118: jmp @121
    119: const r15, bool false
    120: jmp @121
    121: br r15, @122, @214
    122: call r2, f35, []
    123: const r3, str "9223372036854775807"
    124: call r10, f17, [r2, r3]
    125: call r11, f18, [r10]
    126: call r22, f14, []
    127: role.wrap r12, r22, role1
    128: call r13, f36, [r11, r8, r12]
    129: jmp @130
    130: call r14, f33, [r13]
    131: await r0, r14
    132: const r15, bool false
    133: jmp @137
    134: catch r18, object, 6, r23, @135, @213
    135: const r15, bool true
    136: jmp @137
    137: br r15, @138, @140
    138: fut cancelled, r15, r11
    139: jmp @142
    140: const r15, bool false
    141: jmp @142
    142: br r15, @143, @210
    143: call r2, f35, []
    144: const r3, str "9223372036854775807"
    145: call r10, f17, [r2, r3]
    146: call r11, f18, [r10]
    147: call r22, f14, []
    148: role.wrap r12, r22, role1
    149: call r21, f39, [r11, r20, r12]
    150: jmp @151
    151: call r14, f40, [r21]
    152: await r0, r14
    153: const r15, bool false
    154: jmp @158
    155: catch r18, object, 6, r23, @156, @209
    156: const r15, bool true
    157: jmp @158
    158: br r15, @159, @161
    159: fut cancelled, r15, r11
    160: jmp @163
    161: const r15, bool false
    162: jmp @163
    163: br r15, @164, @206
    164: call r2, f35, []
    165: call r9, f35, []
    166: const r3, str "9223372036854775807"
    167: call r4, f8, [r1, r9, r3]
    168: const r3, str "9223372036854775807"
    169: call r10, f17, [r2, r3]
    170: call r11, f18, [r10]
    171: role.wrap r12, r4, role1
    172: call r13, f36, [r11, r8, r12]
    173: call r24, f15, [r13]
    174: call r14, f16, [r24]
    175: fut.spawn r25, r14
    176: fut cancel, _, r25
    177: jmp @178
    178: await r0, r25
    179: const r15, bool false
    180: jmp @184
    181: catch r18, object, 14, r26, @182, @205
    182: const r15, bool true
    183: jmp @184
    184: br r15, @185, @187
    185: fut cancelled, r15, r11
    186: jmp @189
    187: const r15, bool false
    188: jmp @189
    189: br r15, @190, @192
    190: call r15, f38, [r2]
    191: jmp @194
    192: const r15, bool false
    193: jmp @194
    194: br r15, @195, @197
    195: call r15, f38, [r9]
    196: jmp @199
    197: const r15, bool false
    198: jmp @199
    199: br r15, @200, @202
    200: const r0, int 42
    201: ret r0
    202: trap confirm, "confirm failed"
    203: fail.caused r28, r27, r18
    204: raise r28
    205: raise r18
    206: trap confirm, "confirm failed"
    207: fail.caused r28, r27, r18
    208: raise r28
    209: raise r18
    210: trap confirm, "confirm failed"
    211: fail.caused r28, r27, r18
    212: raise r28
    213: raise r18
    214: trap confirm, "confirm failed"
    215: fail.caused r28, r27, r18
    216: raise r28
    217: raise r18
    218: trap confirm, "confirm failed"
    219: trap confirm, "confirm failed"
    220: fail.caused r28, r27, r18
    221: raise r28
    222: raise r18
    223: trap confirm, "confirm failed"
    224: trap confirm, "confirm failed"
}
function f20 "time.duration::Duration.seconds" {
    registers [t3, t9]
    params [r0]
    result t9
    0: obj.get r1, r0, 0
    1: ret r1
}
function f21 "time.clock::Instant.unix_duration" {
    registers [t1, t3]
    params [r0]
    result t3
    0: obj.get r1, r0, 0
    1: ret r1
}
function f22 "time.clock::MonotonicInstant.domain" {
    registers [t2, t6]
    params [r0]
    result t6
    0: obj.get r1, r0, 1
    1: ret r1
}
function f23 "time.clock::IncompatibleClock.text" {
    registers [t14, t6]
    params [r0]
    result t6
    0: const r1, str "incompatible monotonic clock domains"
    1: ret r1
}
function f24 "time.clock::IncompatibleClock.new#0" {
    registers [t14]
    params []
    result t14
    0: obj.new r0, obj11, []
    1: ret r0
}
function f25 "time.clock::MonotonicInstant.reading" {
    registers [t2, t3]
    params [r0]
    result t3
    0: obj.get r1, r0, 0
    1: ret r1
}
function f26 "time.duration::Duration.new#0" {
    registers [t9, t16, t16, t0, t11, t11, t9, t9, t6, t6, t3, t26]
    params [r0, r1]
    result t3
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
    59: call r11, f43, [r8]
    60: throw r11
}
function f27 "time.clock::Instant.new#0" {
    registers [t3, t1]
    params [r0]
    result t1
    0: obj.new r1, obj3, [r0]
    1: ret r1
}
function f28 "time.clock::MonotonicInstant.new#0" {
    registers [t3, t6, t2]
    params [r0]
    result t2
    0: const r1, str "explicit"
    1: obj.new r2, obj5, [r0, r1]
    2: ret r2
}
function f29 "async.atomic::Atomic<int>.replace" intrinsic "okkama.atomic.Atomic.replace" [obj16] [] {
    registers [t7, t11]
    params [r0, r1]
    result void
}
function f30 "async.atomic::Atomic<int>.value" intrinsic "okkama.atomic.Atomic.value" [obj16] [] {
    registers [t7]
    params [r0]
    result t11
}
function f31 "runtime.time::sleep_nanoseconds" async native "runtime/time@1::sleep_nanoseconds/1" {
    registers [t6]
    params [r0]
    result t0
}
function f32 "async.event::Event.set" intrinsic "okkama.event.Event.set" [] [] {
    registers [t8]
    params [r0]
    result void
}
function f33 "async.timeout::Within<int>.result" async intrinsic "okkama.timeout.Within.result" [obj12] [f45, f46] {
    registers [t10]
    params [r0]
    result t11
}
function f34 "async.atomic::Atomic<int>.new#0" intrinsic "okkama.atomic.Atomic.new_value" [obj1] [] {
    registers [t11]
    params [r0]
    result t7
}
function f35 "async.event::Event.new#0" intrinsic "okkama.event.Event.new" [obj2] [] {
    registers []
    params []
    result t8
}
function f36 "async.timeout::Within<int>.new#0" {
    registers [t12, t3, t13, t9, t11, t9, t0, t10, t6, t27]
    params [r0, r1, r2]
    result t10
    0: call r3, f20, [r1]
    1: const r4, int 0
    2: cvt r5, r4
    3: rat.cmp ge, r6, r3, r5
    4: br r6, @5, @7
    5: obj.new r7, obj9, [r0, r1, r2]
    6: ret r7
    7: const r8, str "timeout duration cannot be negative"
    8: call r9, f49, [r8]
    9: throw r9
}
function f37 "async.timeout::TimedOut.text" {
    registers [t23, t6]
    params [r0]
    result t6
    0: const r1, str "future timed out"
    1: ret r1
}
function f38 "async.event::Event.ready" intrinsic "okkama.event.Event.ready" [] [] {
    registers [t8]
    params [r0]
    result t0
}
function f39 "async.timeout::Before<int>.new#0" {
    registers [t12, t2, t13, t24]
    params [r0, r1, r2]
    result t24
    0: obj.new r3, obj13, [r0, r1, r2]
    1: ret r3
}
function f40 "async.timeout::Before<int>.result" async intrinsic "okkama.timeout.Before.result" [obj12] [f50, f51, f45, f46] {
    registers [t24]
    params [r0]
    result t11
}
function f41 "Cancelled.text" {
    registers [t25, t6]
    params [r0]
    result t6
    0: const r1, str "Operation was cancelled"
    1: ret r1
}
function f42 "time.duration::InvalidDuration.text" {
    registers [t26, t6]
    params [r0]
    result t6
    0: obj.get r1, r0, 0
    1: ret r1
}
function f43 "time.duration::InvalidDuration.new#0" {
    registers [t6, t26]
    params [r0]
    result t26
    0: obj.new r1, obj15, [r0]
    1: ret r1
}
function f44 "async.atomic::AtomicClosed.text" {
    registers [t28, t6]
    params [r0]
    result t6
    0: const r1, str "atomic state is closed"
    1: ret r1
}
function f45 "member.seconds#45" {
    registers [t3, t9]
    params [r0]
    result t9
    0: call r1, f20, [r0]
    1: ret r1
}
function f46 "member.wait#46" {
    registers [t13, t3, t4]
    params [r0, r1]
    result t4
    0: call.role r2, r0, role1, 2, [r1]
    1: ret r2
}
function f47 "error::InvalidArgument.default.message" {
    registers [t6]
    params []
    result t6
    0: const r0, str "invalid argument"
    1: ret r0
}
function f48 "error::InvalidArgument.text" {
    registers [t27, t6]
    params [r0]
    result t6
    0: obj.get r1, r0, 0
    1: ret r1
}
function f49 "error::InvalidArgument.new#1" {
    registers [t6, t6, t27]
    params [r0]
    result t27
    0: call r1, f47, []
    1: obj.new r2, obj17, [r0]
    2: ret r2
}
function f50 "member.monotonic_instant#50" {
    registers [t13, t2]
    params [r0]
    result t2
    0: call.role r1, r0, role1, 1, []
    1: ret r1
}
function f51 "member.minus#51" {
    registers [t2, t2, t3]
    params [r0, r1]
    result t3
    0: call r2, f52, [r0, r1]
    1: ret r2
}
function f52 "time.clock::MonotonicInstant.minus" {
    registers [t2, t2, t6, t6, t0, t3, t3, t3, t14]
    params [r0, r1]
    result t3
    0: obj.get r2, r0, 1
    1: call r3, f22, [r1]
    2: str.cmp eq, r4, r2, r3
    3: br r4, @4, @8
    4: obj.get r5, r0, 0
    5: call r6, f25, [r1]
    6: call r7, f53, [r5, r6]
    7: ret r7
    8: call r8, f24, []
    9: throw r8
}
function f53 "time.duration::Duration.minus" {
    registers [t3, t3, t9, t9, t9, t16]
    params [r0, r1]
    result t3
    0: obj.get r2, r0, 0
    1: call r3, f20, [r1]
    2: rat.sub r4, r2, r3
    3: enum.case r5, enum1, 3
    4: call r0, f26, [r4, r5]
    5: ret r0
}
entry f19
