sparrow 9
module "main" "v1.0.0"
type t0 object obj6
type t1 enum enum0
type t2 role role2
type t3 role role1
type t4 future t3
type t5 byte
type t6 array t5
type t7 int
type t8 future t6
type t9 str
type t10 object obj2
type t11 handle 6
type t12 object obj3
type t13 object obj4
type t14 object obj5
type t15 array t14
type t16 enum enum1
type t17 enum enum4
type t18 object obj8
type t19 enum enum5
type t20 region
type t21 object obj0
type t22 object obj1
type t23 future t22
type t24 object obj7
type t25 role role3
type t26 bool
type t27 failure
type t28 handle 9
type t29 role role4
type t30 object obj9
type t31 object obj10
type t32 object obj11
type t33 object obj12
type t34 object obj13
type t35 object obj15
type t36 object obj16
type t37 object obj14
type t38 future t26
type t39 object obj17
type t40 object obj18
type t41 object obj19
type t42 object obj20
type t43 object obj21
type t44 object obj23
type t45 object obj22
type t46 object obj24
enum enum0 "compare::Order" ["less", "equal", "greater"]
enum enum1 "fs.path::PathStyle" ["posix", "windows"]
enum enum2 "encoding.utf::ByteOrder" ["big", "little"]
enum enum3 "time.duration::DurationUnit" ["nanosecond", "microsecond", "millisecond", "second", "minute", "hour", "day"]
enum enum4 "net.failure::NetworkOperation" ["resolving", "binding", "listening", "accepting", "connecting", "reading", "writing", "shutting_down", "closing"]
enum enum5 "net.failure::NetworkFailureKind" ["refused", "reset", "aborted", "unreachable", "timed_out", "address_in_use", "permission_denied", "resource_exhausted", "unsupported", "other"]
enum enum6 "net.ip::IpVersion" ["v4", "v6"]
enum enum7 "net.ip::IpProperty" ["unspecified", "loopback", "link_local", "private_network", "global", "multicast", "reserved", "documentation"]
role role0 "compare::Comparing<Path>" {
    method "order" [t0] -> t1
}
role role1 "net.stream::AsyncWriting" {
    method "write" [t2] -> t4
}
role role2 "io.stream::ByteSource" {
    method "bytes" [] -> t6
}
role role3 "net.stream::AsyncReading" {
    method "bytes" [t7] -> t8
}
role role4 "Error" builtin {
    method "text" [] -> t9
}
object obj0 "net.unix::UnixPaired" {
}
object obj1 "net.unix::UnixPair" {
    field "left" t10
    field "right" t10
}
object obj2 "net.unix::UnixConnection" {
    field "state" t11
    field "ends" t12
    destroy f4
}
object obj3 "net.stream::ConnectionEndpoints<UnixEndpoint>" {
    field "local" t13
    field "remote" t13
}
object obj4 "net.unix::UnixEndpoint" {
    field "addresses" t15
}
object obj5 "net.unix::UnixAddress" {
    field "origin" t0
}
object obj6 "fs.path::Path" {
    field "origin" t9
    field "path_style" t16
    does role0 [f3]
}
object obj7 "io.decorators::ByteSnapshot" {
    field "value" t6
    does role2 [f8]
}
object obj8 "NativeFailure" builtin {
    field "domain" t9
    field "code" t9
    field "operation" t9
    field "message" t9
    does role4 [f12]
}
object obj9 "net.unix::UnixError" {
    field "operation" t17
    field "failure" t18
}
object obj10 "net.unix::UnixWriting" {
    field "owner" t10
    does role1 [f24]
}
object obj11 "net.unix::UnixReading" {
    field "owner" t10
    does role3 [f26]
}
object obj12 "net.unix::UnixEndpointBytes" {
    field "bytes" t6
}
object obj13 "net.failure::NetworkError" {
    field "operation" t17
    field "failure" t18
}
object obj14 "error::InvalidArgument" {
    field "message" t9
    does role4 [f35]
}
object obj15 "net.unix::UnixSocketsUnsupported" {
    does role4 [f38]
}
object obj16 "net.failure::NetworkClosed" {
    does role4 [f40]
}
object obj17 "encoding.utf::Utf8Decoded" {
    field "origin" t6
}
object obj18 "async.timeout::TimedOut" {
    does role4 [f53]
}
object obj19 "net.failure::NetworkFailure" {
    field "operation" t17
    field "kind" t19
    field "message" t9
    does role4 [f55]
}
object obj20 "io.stream::InvalidReadLimit" {
    field "value" t7
    does role4 [f60]
}
object obj21 "encoding.utf::InvalidUtf8" {
    field "invalid_offset" t7
    does role4 [f64]
}
object obj22 "fs.path::InvalidPath" {
    field "message" t9
    does role4 [f67]
}
object obj23 "fs.path::NativePathStyle" {
}
object obj24 "encoding.utf::Utf8Encoded" {
    field "origin" t9
}
handle "runtime.fs::FileHandle"
handle "runtime.fs::DirectoryHandle"
handle "runtime.fs::FileState"
handle "runtime.fs::FileLockState"
handle "runtime.fs::TemporaryDirectoryState"
handle "runtime.fs::FileWatchState"
handle "runtime.network::SocketState"
handle "runtime.network::MembershipState"
handle "runtime.network::ListenerState"
handle "runtime.network::PairState"
handle "runtime.network::TcpListenerState"
handle "runtime.tls_transport::TlsState"
handle "runtime.buffer::BufferState"
handle "runtime.io_decorators::LimitedReadingState"
handle "runtime.io_decorators::ChainedReadingState"
function f0 "main" async {
    registers [t20, t21, t23, t22, t10, t10, t3, t7, t5, t5, t5, t6, t24, t2, t4, t25, t8, t6, t26, t7, t9, t27, t27, t27]
    params []
    result t7
    handler 3 14 @72 r21
    handler 14 41 @64 r21
    handler 41 51 @64 r21
    handler 51 53 @64 r21
    handler 53 55 @68 r21
    handler 55 57 @72 r21
    handler 60 62 @64 r21
    handler 62 64 @64 r21
    handler 64 66 @66 r22
    handler 68 70 @70 r22
    0: jmp @1
    1: region.enter r0
    2: jmp @3
    3: call r1, f1, []
    4: call r2, f2, [r1]
    5: await r3, r2
    6: region.adopt r0, r3
    7: call r4, f5, [r3]
    8: region.track r0, r4
    9: region.adopt r0, r4
    10: call r5, f6, [r3]
    11: region.track r0, r5
    12: region.adopt r0, r5
    13: jmp @14
    14: call r6, f7, [r4]
    15: region.track r0, r6
    16: const r7, int 0
    17: byte.from r8, r7, _
    18: const r7, int 255
    19: byte.from r9, r7, _
    20: const r7, int 42
    21: byte.from r10, r7, _
    22: arr.new r11, [r8, r9, r10]
    23: call r12, f9, [r11]
    24: role.wrap r13, r12, role2
    25: call.role r14, r6, role1, 0, [r13]
    26: await r6, r14
    27: call r15, f10, [r5]
    28: region.track r0, r15
    29: const r7, int 3
    30: call.role r16, r15, role3, 0, [r7]
    31: await r11, r16
    32: const r7, int 0
    33: byte.from r8, r7, _
    34: const r7, int 255
    35: byte.from r9, r7, _
    36: const r7, int 42
    37: byte.from r10, r7, _
    38: arr.new r17, [r8, r9, r10]
    39: val.cmp eq, r18, r11, r17
    40: br r18, @41, @62
    41: call _, f11, [r4]
    42: call r15, f10, [r5]
    43: region.track r0, r15
    44: const r7, int 1
    45: call.role r16, r15, role3, 0, [r7]
    46: await r11, r16
    47: arr.len r7, r11
    48: const r19, int 0
    49: int.cmp eq, r18, r7, r19
    50: br r18, @51, @60
    51: const r7, int 42
    52: jmp @53
    53: call _, f11, [r5]
    54: jmp @55
    55: call _, f11, [r4]
    56: jmp @57
    57: region.leave r0, _
    58: jmp @59
    59: ret r7
    60: const r20, str "Unix EOF missing"
    61: panic r20
    62: const r20, str "Unix payload changed"
    63: panic r20
    64: call _, f11, [r5]
    65: jmp @68
    66: fail.secondary r23, r21, r22
    67: jmp @68(r21 <- r23)
    68: call _, f11, [r4]
    69: jmp @72
    70: fail.secondary r23, r21, r22
    71: jmp @72(r21 <- r23)
    72: region.unwind r22, r0, r21
    73: raise r22
}
function f1 "net.unix::UnixPaired.new#0" {
    registers [t21]
    params []
    result t21
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj0, []
    3: jmp @4
    4: ret r0
}
function f2 "net.unix::UnixPaired.result" async {
    registers [t21, t20, t20, t20, t28, t20, t26, t11, t10, t10, t22, t27, t27, t18, t27, t29, t17, t30]
    params [r0]
    result t22
    handler 2 4 @69 r11
    handler 4 6 @54 r11
    handler 6 8 @52 r11
    handler 8 10 @39 r11
    handler 10 23 @37 r11
    handler 23 25 @39 r11
    handler 25 27 @39 r11
    handler 27 29 @54 r11
    handler 29 31 @54 r11
    handler 31 33 @69 r11
    handler 33 35 @69 r11
    handler 37 39 @39 r11
    handler 40 41 @43 r12
    handler 41 43 @43 r12
    handler 43 45 @52 r11
    handler 46 47 @49 r12
    handler 47 49 @49 r12
    handler 49 51 @52 r11
    handler 51 52 @52 r11
    handler 52 54 @54 r11
    handler 55 56 @64 r12
    handler 56 60 @64 r12
    handler 60 61 @64 r12
    handler 61 63 @64 r12
    handler 63 64 @64 r12
    handler 64 66 @67 r11
    handler 66 67 @67 r11
    handler 67 69 @69 r11
    0: region.enter r1
    1: jmp @2
    2: region.enter r2
    3: jmp @4
    4: region.enter r3
    5: jmp @6
    6: call r4, f13, []
    7: jmp @8
    8: region.enter r5
    9: jmp @10
    10: const r6, bool false
    11: call r7, f14, [r4, r6]
    12: call r8, f15, [r7]
    13: region.track r5, r8
    14: const r6, bool true
    15: call r7, f14, [r4, r6]
    16: call r9, f15, [r7]
    17: region.track r5, r9
    18: call r10, f16, [r8, r9]
    19: region.track r5, r10
    20: region.adopt r5, r10
    21: call r6, f17, [r4]
    22: jmp @23
    23: region.leave r5, r10
    24: jmp @25
    25: region.track r3, r10
    26: jmp @27
    27: region.leave r3, r10
    28: jmp @29
    29: region.track r2, r10
    30: jmp @31
    31: region.leave r2, r10
    32: jmp @33
    33: region.track r1, r10
    34: jmp @35
    35: region.leave r1, r10
    36: ret r10
    37: region.unwind r12, r5, r11
    38: raise r12
    39: catch r11, object, 8, r13, @40, @45
    40: jmp @41
    41: call r6, f18, [r4]
    42: raise r11
    43: fail.caused r14, r12, r11
    44: raise r14
    45: catch r11, role, 4, r15, @46, @51
    46: jmp @47
    47: call r6, f18, [r4]
    48: raise r11
    49: fail.caused r14, r12, r11
    50: raise r14
    51: raise r11
    52: region.unwind r12, r3, r11
    53: raise r12
    54: catch r11, object, 8, r13, @55, @66
    55: jmp @56
    56: enum.case r16, enum4, 4
    57: call r17, f19, [r16, r13]
    58: call r6, f20, [r17]
    59: br r6, @60, @63
    60: jmp @61
    61: call r15, f21, [r17]
    62: throw r15
    63: raise r11
    64: fail.caused r14, r12, r11
    65: raise r14
    66: raise r11
    67: region.unwind r12, r2, r11
    68: raise r12
    69: region.unwind r12, r1, r11
    70: raise r12
}
function f3 "fs.path::Path.order" {
    registers [t0, t0, t9, t9, t26, t1]
    params [r0, r1]
    result t1
    0: jmp @1
    1: jmp @2
    2: obj.get r2, r0, 0
    3: call r3, f22, [r1]
    4: str.cmp lt, r4, r2, r3
    5: br r4, @6, @8
    6: enum.case r5, enum0, 0
    7: jmp @14
    8: str.cmp eq, r4, r2, r3
    9: br r4, @10, @12
    10: enum.case r5, enum0, 1
    11: jmp @14
    12: enum.case r5, enum0, 2
    13: jmp @14
    14: jmp @15
    15: ret r5
}
function f4 "net.unix::UnixConnection.destroy" {
    registers [t10, t11, t26]
    params [r0]
    result void
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: call r2, f23, [r1]
    4: jmp @5
    5: ret _
}
function f5 "net.unix::UnixPair.first" {
    registers [t22, t20, t10, t27, t27]
    params [r0]
    result t10
    handler 2 4 @10 r3
    handler 4 6 @10 r3
    handler 6 8 @10 r3
    0: region.enter r1
    1: jmp @2
    2: region.adopt r1, r0
    3: jmp @4
    4: obj.get r2, r0, 0
    5: jmp @6
    6: region.track r1, r2
    7: jmp @8
    8: region.leave r1, r2
    9: ret r2
    10: region.unwind r4, r1, r3
    11: raise r4
}
function f6 "net.unix::UnixPair.second" {
    registers [t22, t20, t10, t27, t27]
    params [r0]
    result t10
    handler 2 4 @10 r3
    handler 4 6 @10 r3
    handler 6 8 @10 r3
    0: region.enter r1
    1: jmp @2
    2: region.adopt r1, r0
    3: jmp @4
    4: obj.get r2, r0, 1
    5: jmp @6
    6: region.track r1, r2
    7: jmp @8
    8: region.leave r1, r2
    9: ret r2
    10: region.unwind r4, r1, r3
    11: raise r4
}
function f7 "net.unix::UnixConnection.output" {
    registers [t10, t20, t20, t31, t3, t27, t27]
    params [r0]
    result t3
    handler 2 5 @17 r5
    handler 5 9 @15 r5
    handler 9 11 @17 r5
    handler 11 13 @17 r5
    handler 15 17 @17 r5
    0: region.enter r1
    1: jmp @2
    2: region.adopt r1, r0
    3: region.enter r2
    4: jmp @5
    5: call r3, f25, [r0]
    6: region.track r2, r3
    7: role.wrap r4, r3, role1
    8: jmp @9
    9: region.leave r2, r4
    10: jmp @11
    11: region.track r1, r4
    12: jmp @13
    13: region.leave r1, r4
    14: ret r4
    15: region.unwind r6, r2, r5
    16: raise r6
    17: region.unwind r6, r1, r5
    18: raise r6
}
function f8 "io.decorators::ByteSnapshot.bytes" {
    registers [t24, t6]
    params [r0]
    result t6
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: jmp @4
    4: ret r1
}
function f9 "io.decorators::ByteSnapshot.new#0" {
    registers [t6, t24]
    params [r0]
    result t24
    0: jmp @1
    1: jmp @2
    2: obj.new r1, obj7, [r0]
    3: jmp @4
    4: ret r1
}
function f10 "net.unix::UnixConnection.input" {
    registers [t10, t20, t20, t32, t25, t27, t27]
    params [r0]
    result t25
    handler 2 5 @17 r5
    handler 5 9 @15 r5
    handler 9 11 @17 r5
    handler 11 13 @17 r5
    handler 15 17 @17 r5
    0: region.enter r1
    1: jmp @2
    2: region.adopt r1, r0
    3: region.enter r2
    4: jmp @5
    5: call r3, f27, [r0]
    6: region.track r2, r3
    7: role.wrap r4, r3, role3
    8: jmp @9
    9: region.leave r2, r4
    10: jmp @11
    11: region.track r1, r4
    12: jmp @13
    13: region.leave r1, r4
    14: ret r4
    15: region.unwind r6, r2, r5
    16: raise r6
    17: region.unwind r6, r1, r5
    18: raise r6
}
function f11 "net.unix::UnixConnection.close" {
    registers [t10, t20, t11, t26, t27, t18, t17, t30, t29, t27, t27]
    params [r0]
    result void
    handler 2 4 @23 r4
    handler 4 5 @10 r4
    handler 5 8 @10 r4
    handler 11 12 @20 r9
    handler 12 16 @20 r9
    handler 16 17 @20 r9
    handler 17 19 @20 r9
    handler 19 20 @20 r9
    handler 20 22 @23 r4
    handler 22 23 @23 r4
    0: region.enter r1
    1: jmp @2
    2: region.adopt r1, r0
    3: jmp @4
    4: jmp @5
    5: obj.get r2, r0, 0
    6: call r3, f23, [r2]
    7: jmp @8
    8: region.leave r1, _
    9: ret _
    10: catch r4, object, 8, r5, @11, @22
    11: jmp @12
    12: enum.case r6, enum4, 8
    13: call r7, f19, [r6, r5]
    14: call r3, f20, [r7]
    15: br r3, @16, @19
    16: jmp @17
    17: call r8, f21, [r7]
    18: throw r8
    19: raise r4
    20: fail.caused r10, r9, r4
    21: raise r10
    22: raise r4
    23: region.unwind r9, r1, r4
    24: raise r9
}
function f12 "NativeFailure.text" {
    registers [t18, t9]
    params [r0]
    result t9
    0: obj.get r1, r0, 3
    1: ret r1
}
function f13 "runtime.network::unix_pair" native "runtime/network@1::unix_pair/0" {
    registers []
    params []
    result t28
}
function f14 "runtime.network::unix_pair_socket" native "runtime/network@1::unix_pair_socket/2" {
    registers [t28, t26]
    params [r0, r1]
    result t11
}
function f15 "net.unix::UnixConnection.new#0" {
    registers [t11, t26, t6, t33, t13, t13, t12, t10, t27, t18, t27, t27, t29]
    params [r0]
    result t10
    handler 2 3 @16 r8
    handler 3 13 @16 r8
    handler 17 18 @20 r10
    handler 18 20 @20 r10
    handler 23 24 @26 r10
    handler 24 26 @26 r10
    0: jmp @1
    1: jmp @2
    2: jmp @3
    3: const r1, bool false
    4: call r2, f28, [r0, r1]
    5: call r3, f29, [r2]
    6: call r4, f30, [r3]
    7: const r1, bool true
    8: call r2, f28, [r0, r1]
    9: call r3, f29, [r2]
    10: call r5, f30, [r3]
    11: call r6, f31, [r4, r5]
    12: jmp @13
    13: obj.new r7, obj2, [r0, r6]
    14: jmp @15
    15: ret r7
    16: catch r8, object, 8, r9, @17, @22
    17: jmp @18
    18: call r1, f23, [r0]
    19: raise r8
    20: fail.caused r11, r10, r8
    21: raise r11
    22: catch r8, role, 4, r12, @23, @28
    23: jmp @24
    24: call r1, f23, [r0]
    25: raise r8
    26: fail.caused r11, r10, r8
    27: raise r11
    28: raise r8
}
function f16 "net.unix::UnixPair.new#0" {
    registers [t10, t10, t20, t20, t22, t27, t27]
    params [r0, r1]
    result t22
    handler 2 6 @19 r5
    handler 6 11 @17 r5
    handler 11 13 @19 r5
    handler 13 15 @19 r5
    handler 17 19 @19 r5
    0: region.enter r2
    1: jmp @2
    2: region.adopt r2, r0
    3: region.adopt r2, r1
    4: region.enter r3
    5: jmp @6
    6: region.transfer r3, r2, r0
    7: region.adopt r2, r0
    8: region.transfer r3, r2, r1
    9: region.adopt r2, r1
    10: jmp @11
    11: region.leave r3, _
    12: jmp @13
    13: obj.new r4, obj1, [r0, r1]
    14: jmp @15
    15: region.leave r2, r4
    16: ret r4
    17: region.unwind r6, r3, r5
    18: raise r6
    19: region.unwind r6, r2, r5
    20: raise r6
}
function f17 "runtime.network::unix_pair_release" native "runtime/network@1::unix_pair_release/1" {
    registers [t28]
    params [r0]
    result t26
}
function f18 "runtime.network::unix_pair_close" native "runtime/network@1::unix_pair_close/1" {
    registers [t28]
    params [r0]
    result t26
}
function f19 "net.unix::UnixError.new#0" {
    registers [t17, t18, t30]
    params [r0, r1]
    result t30
    0: jmp @1
    1: jmp @2
    2: obj.new r2, obj9, [r0, r1]
    3: jmp @4
    4: ret r2
}
function f20 "net.unix::UnixError.applicable" {
    registers [t30, t17, t18, t34, t26]
    params [r0]
    result t26
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: obj.get r2, r0, 1
    4: call r3, f32, [r1, r2]
    5: call r4, f33, [r3]
    6: jmp @7
    7: ret r4
}
function f21 "net.unix::UnixError.value" {
    registers [t30, t26, t18, t9, t9, t35, t29, t36, t37, t17, t34]
    params [r0]
    result t29
    0: jmp @1
    1: jmp @2
    2: call r1, f20, [r0]
    3: br r1, @4, @42
    4: obj.get r2, r0, 1
    5: call r3, f37, [r2]
    6: const r4, str "unsupported"
    7: str.cmp eq, r1, r3, r4
    8: br r1, @9, @14
    9: jmp @10
    10: call r5, f39, []
    11: role.wrap r6, r5, role4
    12: jmp @13
    13: ret r6
    14: obj.get r2, r0, 1
    15: call r3, f37, [r2]
    16: const r4, str "already_closed"
    17: str.cmp eq, r1, r3, r4
    18: br r1, @19, @24
    19: jmp @20
    20: call r7, f41, []
    21: role.wrap r6, r7, role4
    22: jmp @23
    23: ret r6
    24: obj.get r2, r0, 1
    25: call r3, f37, [r2]
    26: const r4, str "invalid_input"
    27: str.cmp eq, r1, r3, r4
    28: br r1, @29, @36
    29: jmp @30
    30: obj.get r2, r0, 1
    31: call r3, f42, [r2]
    32: call r8, f36, [r3]
    33: role.wrap r6, r8, role4
    34: jmp @35
    35: ret r6
    36: obj.get r9, r0, 0
    37: obj.get r2, r0, 1
    38: call r10, f32, [r9, r2]
    39: call r6, f43, [r10]
    40: jmp @41
    41: ret r6
    42: const r3, str "not a terminal Unix failure"
    43: call r8, f36, [r3]
    44: throw r8
}
function f22 "fs.path::Path.string" {
    registers [t0, t9]
    params [r0]
    result t9
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: jmp @4
    4: ret r1
}
function f23 "runtime.network::close" native "runtime/network@1::close/1" {
    registers [t11]
    params [r0]
    result t26
}
function f24 "net.unix::UnixWriting.write" async {
    registers [t31, t2, t20, t10, t38, t26, t3, t27, t27]
    params [r0, r1]
    result t3
    handler 2 4 @13 r7
    handler 4 9 @13 r7
    handler 9 11 @13 r7
    0: region.enter r2
    1: jmp @2
    2: region.adopt r2, r0
    3: jmp @4
    4: obj.get r3, r0, 0
    5: call r4, f44, [r3, r1]
    6: await r5, r4
    7: role.wrap r6, r0, role1
    8: jmp @9
    9: region.track r2, r6
    10: jmp @11
    11: region.leave r2, r6
    12: ret r6
    13: region.unwind r8, r2, r7
    14: raise r8
}
function f25 "net.unix::UnixWriting.new#0" {
    registers [t10, t20, t20, t31, t27, t27]
    params [r0]
    result t31
    handler 2 5 @16 r4
    handler 5 8 @14 r4
    handler 8 10 @16 r4
    handler 10 12 @16 r4
    handler 14 16 @16 r4
    0: region.enter r1
    1: jmp @2
    2: region.adopt r1, r0
    3: region.enter r2
    4: jmp @5
    5: region.transfer r2, r1, r0
    6: region.adopt r1, r0
    7: jmp @8
    8: region.leave r2, _
    9: jmp @10
    10: obj.new r3, obj10, [r0]
    11: jmp @12
    12: region.leave r1, r3
    13: ret r3
    14: region.unwind r5, r2, r4
    15: raise r5
    16: region.unwind r5, r1, r4
    17: raise r5
}
function f26 "net.unix::UnixReading.bytes" async {
    registers [t32, t7, t20, t10, t8, t6, t27, t27]
    params [r0, r1]
    result t6
    handler 2 4 @10 r6
    handler 4 8 @10 r6
    0: region.enter r2
    1: jmp @2
    2: region.adopt r2, r0
    3: jmp @4
    4: obj.get r3, r0, 0
    5: call r4, f45, [r3, r1]
    6: await r5, r4
    7: jmp @8
    8: region.leave r2, _
    9: ret r5
    10: region.unwind r7, r2, r6
    11: raise r7
}
function f27 "net.unix::UnixReading.new#0" {
    registers [t10, t20, t20, t32, t27, t27]
    params [r0]
    result t32
    handler 2 5 @16 r4
    handler 5 8 @14 r4
    handler 8 10 @16 r4
    handler 10 12 @16 r4
    handler 14 16 @16 r4
    0: region.enter r1
    1: jmp @2
    2: region.adopt r1, r0
    3: region.enter r2
    4: jmp @5
    5: region.transfer r2, r1, r0
    6: region.adopt r1, r0
    7: jmp @8
    8: region.leave r2, _
    9: jmp @10
    10: obj.new r3, obj11, [r0]
    11: jmp @12
    12: region.leave r1, r3
    13: ret r3
    14: region.unwind r5, r2, r4
    15: raise r5
    16: region.unwind r5, r1, r4
    17: raise r5
}
function f28 "runtime.network::unix_endpoint" native "runtime/network@1::unix_endpoint/2" {
    registers [t11, t26]
    params [r0, r1]
    result t6
}
function f29 "net.unix::UnixEndpointBytes.new#0" {
    registers [t6, t33]
    params [r0]
    result t33
    0: jmp @1
    1: jmp @2
    2: obj.new r1, obj12, [r0]
    3: jmp @4
    4: ret r1
}
function f30 "net.unix::UnixEndpointBytes.endpoint" {
    registers [t33, t6, t7, t7, t26, t13, t39, t9, t0, t14]
    params [r0]
    result t13
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: arr.len r2, r1
    4: const r3, int 0
    5: int.cmp eq, r4, r2, r3
    6: br r4, @7, @10
    7: jmp @8
    8: call r5, f46, []
    9: jmp @18
    10: jmp @11
    11: obj.get r1, r0, 0
    12: call r6, f47, [r1]
    13: call r7, f48, [r6]
    14: call r8, f49, [r7]
    15: call r9, f50, [r8]
    16: call r5, f51, [r9]
    17: jmp @18
    18: jmp @19
    19: ret r5
}
function f31 "net.stream::ConnectionEndpoints<UnixEndpoint>.new#0" {
    registers [t13, t13, t12]
    params [r0, r1]
    result t12
    0: jmp @1
    1: jmp @2
    2: obj.new r2, obj3, [r0, r1]
    3: jmp @4
    4: ret r2
}
function f32 "net.failure::NetworkError.new#0" {
    registers [t17, t18, t34]
    params [r0, r1]
    result t34
    0: jmp @1
    1: jmp @2
    2: obj.new r2, obj13, [r0, r1]
    3: jmp @4
    4: ret r2
}
function f33 "net.failure::NetworkError.applicable" {
    registers [t34, t18, t9, t9, t9, t26]
    params [r0]
    result t26
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 1
    3: call r2, f37, [r1]
    4: obj.get r1, r0, 1
    5: call r3, f52, [r1]
    6: const r4, str "core.net"
    7: str.cmp eq, r5, r3, r4
    8: br r5, @9, @12
    9: const r3, str "would_block"
    10: str.cmp ne, r5, r2, r3
    11: jmp @14
    12: const r5, bool false
    13: jmp @14
    14: br r5, @15, @18
    15: const r3, str "interrupted"
    16: str.cmp ne, r5, r2, r3
    17: jmp @20
    18: const r5, bool false
    19: jmp @20
    20: jmp @21
    21: ret r5
}
function f34 "error::InvalidArgument.default.message" {
    registers [t9]
    params []
    result t9
    0: jmp @1
    1: const r0, str "invalid argument"
    2: jmp @3
    3: ret r0
}
function f35 "error::InvalidArgument.text" {
    registers [t37, t9]
    params [r0]
    result t9
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: jmp @4
    4: ret r1
}
function f36 "error::InvalidArgument.new#1" {
    registers [t9, t9, t37]
    params [r0]
    result t37
    0: jmp @1
    1: call r1, f34, []
    2: jmp @3
    3: obj.new r2, obj14, [r0]
    4: jmp @5
    5: ret r2
}
function f37 "NativeFailure.code" {
    registers [t18, t9]
    params [r0]
    result t9
    0: obj.get r1, r0, 1
    1: ret r1
}
function f38 "net.unix::UnixSocketsUnsupported.text" {
    registers [t35, t9]
    params [r0]
    result t9
    0: jmp @1
    1: jmp @2
    2: const r1, str "Unix sockets are unsupported by this provider"
    3: jmp @4
    4: ret r1
}
function f39 "net.unix::UnixSocketsUnsupported.new#0" {
    registers [t35]
    params []
    result t35
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj15, []
    3: jmp @4
    4: ret r0
}
function f40 "net.failure::NetworkClosed.text" {
    registers [t36, t9]
    params [r0]
    result t9
    0: jmp @1
    1: jmp @2
    2: const r1, str "network closed"
    3: jmp @4
    4: ret r1
}
function f41 "net.failure::NetworkClosed.new#0" {
    registers [t36]
    params []
    result t36
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj16, []
    3: jmp @4
    4: ret r0
}
function f42 "NativeFailure.message" {
    registers [t18, t9]
    params [r0]
    result t9
    0: obj.get r1, r0, 3
    1: ret r1
}
function f43 "net.failure::NetworkError.value" {
    registers [t34, t26, t18, t9, t9, t40, t29, t17, t19, t41, t37]
    params [r0]
    result t29
    0: jmp @1
    1: jmp @2
    2: call r1, f33, [r0]
    3: br r1, @4, @22
    4: obj.get r2, r0, 1
    5: call r3, f37, [r2]
    6: const r4, str "timed_out"
    7: str.cmp eq, r1, r3, r4
    8: br r1, @9, @14
    9: jmp @10
    10: call r5, f54, []
    11: role.wrap r6, r5, role4
    12: jmp @13
    13: ret r6
    14: obj.get r7, r0, 0
    15: call r8, f56, [r0]
    16: obj.get r2, r0, 1
    17: call r3, f42, [r2]
    18: call r9, f57, [r7, r8, r3]
    19: role.wrap r6, r9, role4
    20: jmp @21
    21: ret r6
    22: const r3, str "native failure is not a terminal core.net failure"
    23: call r10, f36, [r3]
    24: throw r10
}
function f44 "net.unix::UnixConnection.write_bytes" async {
    registers [t10, t2, t20, t26, t26, t6, t11, t38, t27, t18, t17, t30, t29, t27, t27, t36]
    params [r0, r1]
    result t26
    handler 2 4 @33 r8
    handler 4 7 @33 r8
    handler 7 9 @33 r8
    handler 9 10 @18 r8
    handler 10 14 @18 r8
    handler 14 16 @33 r8
    handler 19 20 @28 r13
    handler 20 24 @28 r13
    handler 24 25 @28 r13
    handler 25 27 @28 r13
    handler 27 28 @28 r13
    handler 28 30 @33 r8
    handler 30 31 @33 r8
    handler 31 33 @33 r8
    0: region.enter r2
    1: jmp @2
    2: region.adopt r2, r0
    3: jmp @4
    4: call r3, f58, [r0]
    5: bool.not r4, r3
    6: br r4, @7, @31
    7: call.role r5, r1, role2, 0, []
    8: jmp @9
    9: jmp @10
    10: obj.get r6, r0, 0
    11: call r7, f59, [r6, r5]
    12: await r3, r7
    13: jmp @14
    14: const r3, bool true
    15: jmp @16
    16: region.leave r2, _
    17: ret r3
    18: catch r8, object, 8, r9, @19, @30
    19: jmp @20
    20: enum.case r10, enum4, 6
    21: call r11, f19, [r10, r9]
    22: call r3, f20, [r11]
    23: br r3, @24, @27
    24: jmp @25
    25: call r12, f21, [r11]
    26: throw r12
    27: raise r8
    28: fail.caused r14, r13, r8
    29: raise r14
    30: raise r8
    31: call r15, f41, []
    32: throw r15
    33: region.unwind r13, r2, r8
    34: raise r13
}
function f45 "net.unix::UnixConnection.read_bytes" async {
    registers [t10, t7, t20, t7, t26, t11, t8, t6, t27, t18, t17, t30, t29, t27, t27, t42]
    params [r0, r1]
    result t6
    handler 2 4 @29 r8
    handler 4 7 @29 r8
    handler 7 8 @14 r8
    handler 8 12 @14 r8
    handler 15 16 @24 r13
    handler 16 20 @24 r13
    handler 20 21 @24 r13
    handler 21 23 @24 r13
    handler 23 24 @24 r13
    handler 24 26 @29 r8
    handler 26 27 @29 r8
    handler 27 29 @29 r8
    0: region.enter r2
    1: jmp @2
    2: region.adopt r2, r0
    3: jmp @4
    4: const r3, int 0
    5: int.cmp gt, r4, r1, r3
    6: br r4, @7, @27
    7: jmp @8
    8: obj.get r5, r0, 0
    9: call r6, f62, [r5, r1]
    10: await r7, r6
    11: jmp @12
    12: region.leave r2, _
    13: ret r7
    14: catch r8, object, 8, r9, @15, @26
    15: jmp @16
    16: enum.case r10, enum4, 5
    17: call r11, f19, [r10, r9]
    18: call r4, f20, [r11]
    19: br r4, @20, @23
    20: jmp @21
    21: call r12, f21, [r11]
    22: throw r12
    23: raise r8
    24: fail.caused r14, r13, r8
    25: raise r14
    26: raise r8
    27: call r15, f61, [r1]
    28: throw r15
    29: region.unwind r13, r2, r8
    30: raise r13
}
function f46 "net.unix::UnixEndpoint.new#0" {
    registers [t15, t13]
    params []
    result t13
    0: jmp @1
    1: jmp @2
    2: arr.new r0, []
    3: jmp @4
    4: obj.new r1, obj4, [r0]
    5: jmp @6
    6: ret r1
}
function f47 "encoding.utf::Utf8Decoded.new#0" {
    registers [t6, t39]
    params [r0]
    result t39
    0: jmp @1
    1: jmp @2
    2: obj.new r1, obj17, [r0]
    3: jmp @4
    4: ret r1
}
function f48 "encoding.utf::Utf8Decoded.string" {
    registers [t39, t6, t7, t7, t26, t9, t43]
    params [r0]
    result t9
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: call r2, f63, [r1]
    4: obj.get r1, r0, 0
    5: arr.len r3, r1
    6: int.cmp eq, r4, r2, r3
    7: br r4, @8, @12
    8: obj.get r1, r0, 0
    9: call r5, f66, [r1]
    10: jmp @11
    11: ret r5
    12: call r6, f65, [r2]
    13: throw r6
}
function f49 "fs.path::Path.new#0" {
    registers [t9, t7, t7, t26, t26, t44, t16, t0, t45]
    params [r0]
    result t0
    0: jmp @1
    1: jmp @2
    2: str.len r1, r0
    3: const r2, int 0
    4: int.cmp eq, r3, r1, r2
    5: bool.not r4, r3
    6: br r4, @7, @13
    7: call r5, f69, []
    8: call r6, f70, [r5]
    9: jmp @10
    10: obj.new r7, obj6, [r0, r6]
    11: jmp @12
    12: ret r7
    13: const r0, str "path cannot be empty"
    14: call r8, f68, [r0]
    15: throw r8
}
function f50 "net.unix::UnixAddress.new#0" {
    registers [t0, t9, t46, t6, t26, t14, t27, t18, t9, t37, t35, t27, t27]
    params [r0]
    result t14
    handler 2 3 @11 r6
    handler 3 8 @11 r6
    handler 12 13 @43 r11
    handler 13 17 @43 r11
    handler 17 21 @43 r11
    handler 21 23 @43 r11
    handler 24 25 @43 r11
    handler 25 28 @43 r11
    handler 28 32 @43 r11
    handler 32 36 @43 r11
    handler 36 38 @43 r11
    handler 39 40 @43 r11
    handler 40 42 @43 r11
    handler 42 43 @43 r11
    0: jmp @1
    1: jmp @2
    2: jmp @3
    3: call r1, f22, [r0]
    4: call r2, f72, [r1]
    5: call r3, f73, [r2]
    6: call r4, f71, [r3]
    7: jmp @8
    8: obj.new r5, obj5, [r0]
    9: jmp @10
    10: ret r5
    11: catch r6, object, 8, r7, @12, @45
    12: jmp @13
    13: call r1, f52, [r7]
    14: const r8, str "core.net"
    15: str.cmp eq, r4, r1, r8
    16: br r4, @17, @21
    17: call r1, f37, [r7]
    18: const r8, str "invalid_input"
    19: str.cmp eq, r4, r1, r8
    20: jmp @23
    21: const r4, bool false
    22: jmp @23
    23: br r4, @24, @28
    24: jmp @25
    25: call r1, f42, [r7]
    26: call r9, f36, [r1]
    27: throw r9
    28: call r1, f52, [r7]
    29: const r8, str "core.net"
    30: str.cmp eq, r4, r1, r8
    31: br r4, @32, @36
    32: call r1, f37, [r7]
    33: const r8, str "unsupported"
    34: str.cmp eq, r4, r1, r8
    35: jmp @38
    36: const r4, bool false
    37: jmp @38
    38: br r4, @39, @42
    39: jmp @40
    40: call r10, f39, []
    41: throw r10
    42: raise r6
    43: fail.caused r12, r11, r6
    44: raise r12
    45: raise r6
}
function f51 "net.unix::UnixEndpoint.new#1" {
    registers [t14, t15, t13]
    params [r0]
    result t13
    0: jmp @1
    1: jmp @2
    2: arr.new r1, [r0]
    3: jmp @4
    4: obj.new r2, obj4, [r1]
    5: jmp @6
    6: ret r2
}
function f52 "NativeFailure.domain" {
    registers [t18, t9]
    params [r0]
    result t9
    0: obj.get r1, r0, 0
    1: ret r1
}
function f53 "async.timeout::TimedOut.text" {
    registers [t40, t9]
    params [r0]
    result t9
    0: jmp @1
    1: jmp @2
    2: const r1, str "future timed out"
    3: jmp @4
    4: ret r1
}
function f54 "async.timeout::TimedOut.new#0" {
    registers [t40]
    params []
    result t40
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj18, []
    3: jmp @4
    4: ret r0
}
function f55 "net.failure::NetworkFailure.text" {
    registers [t41, t9]
    params [r0]
    result t9
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 2
    3: jmp @4
    4: ret r1
}
function f56 "net.failure::NetworkError.category" {
    registers [t34, t18, t9, t9, t26, t19]
    params [r0]
    result t19
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 1
    3: call r2, f37, [r1]
    4: const r3, str "connection_refused"
    5: str.cmp eq, r4, r2, r3
    6: br r4, @7, @11
    7: jmp @8
    8: enum.case r5, enum5, 0
    9: jmp @10
    10: ret r5
    11: const r3, str "connection_reset"
    12: str.cmp eq, r4, r2, r3
    13: br r4, @14, @18
    14: jmp @15
    15: enum.case r5, enum5, 1
    16: jmp @17
    17: ret r5
    18: const r3, str "network_unreachable"
    19: str.cmp eq, r4, r2, r3
    20: br r4, @21, @25
    21: jmp @22
    22: enum.case r5, enum5, 3
    23: jmp @24
    24: ret r5
    25: const r3, str "address_in_use"
    26: str.cmp eq, r4, r2, r3
    27: br r4, @28, @32
    28: jmp @29
    29: enum.case r5, enum5, 5
    30: jmp @31
    31: ret r5
    32: const r3, str "permission_denied"
    33: str.cmp eq, r4, r2, r3
    34: br r4, @35, @39
    35: jmp @36
    36: enum.case r5, enum5, 6
    37: jmp @38
    38: ret r5
    39: const r3, str "resource_exhausted"
    40: str.cmp eq, r4, r2, r3
    41: br r4, @42, @46
    42: jmp @43
    43: enum.case r5, enum5, 7
    44: jmp @45
    45: ret r5
    46: const r3, str "unsupported"
    47: str.cmp eq, r4, r2, r3
    48: br r4, @49, @53
    49: jmp @50
    50: enum.case r5, enum5, 8
    51: jmp @52
    52: ret r5
    53: enum.case r5, enum5, 9
    54: jmp @55
    55: ret r5
}
function f57 "net.failure::NetworkFailure.new#0" {
    registers [t17, t19, t9, t41]
    params [r0, r1, r2]
    result t41
    0: jmp @1
    1: jmp @2
    2: obj.new r3, obj19, [r0, r1, r2]
    3: jmp @4
    4: ret r3
}
function f58 "net.unix::UnixConnection.closed" {
    registers [t10, t20, t11, t26, t27, t27]
    params [r0]
    result t26
    handler 2 4 @9 r4
    handler 4 7 @9 r4
    0: region.enter r1
    1: jmp @2
    2: region.adopt r1, r0
    3: jmp @4
    4: obj.get r2, r0, 0
    5: call r3, f74, [r2]
    6: jmp @7
    7: region.leave r1, _
    8: ret r3
    9: region.unwind r5, r1, r4
    10: raise r5
}
function f59 "runtime.network::stream_write" async native "runtime/network@1::stream_write/2" {
    registers [t11, t6]
    params [r0, r1]
    result t26
}
function f60 "io.stream::InvalidReadLimit.text" {
    registers [t42, t9, t7, t9, t9]
    params [r0]
    result t9
    0: jmp @1
    1: jmp @2
    2: const r1, str "read limit must be positive, got "
    3: obj.get r2, r0, 0
    4: cvt r3, r2
    5: str.concat r4, r1, r3
    6: jmp @7
    7: ret r4
}
function f61 "io.stream::InvalidReadLimit.new#0" {
    registers [t7, t42]
    params [r0]
    result t42
    0: jmp @1
    1: jmp @2
    2: obj.new r1, obj20, [r0]
    3: jmp @4
    4: ret r1
}
function f62 "runtime.network::stream_read" async native "runtime/network@1::stream_read/2" {
    registers [t11, t7]
    params [r0, r1]
    result t6
}
function f63 "runtime.utf::utf8_valid_prefix" native "runtime/utf@1::utf8_valid_prefix/1" {
    registers [t6]
    params [r0]
    result t7
}
function f64 "encoding.utf::InvalidUtf8.text" {
    registers [t43, t9, t7, t9, t9]
    params [r0]
    result t9
    0: jmp @1
    1: jmp @2
    2: const r1, str "invalid UTF-8 sequence at byte offset "
    3: obj.get r2, r0, 0
    4: cvt r3, r2
    5: str.concat r4, r1, r3
    6: jmp @7
    7: ret r4
}
function f65 "encoding.utf::InvalidUtf8.new#0" {
    registers [t7, t43]
    params [r0]
    result t43
    0: jmp @1
    1: jmp @2
    2: obj.new r1, obj21, [r0]
    3: jmp @4
    4: ret r1
}
function f66 "runtime.utf::utf8_decode" native "runtime/utf@1::utf8_decode/1" {
    registers [t6]
    params [r0]
    result t9
}
function f67 "fs.path::InvalidPath.text" {
    registers [t45, t9]
    params [r0]
    result t9
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: jmp @4
    4: ret r1
}
function f68 "fs.path::InvalidPath.new#0" {
    registers [t9, t45]
    params [r0]
    result t45
    0: jmp @1
    1: jmp @2
    2: obj.new r1, obj22, [r0]
    3: jmp @4
    4: ret r1
}
function f69 "fs.path::NativePathStyle.new#0" {
    registers [t44]
    params []
    result t44
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj23, []
    3: jmp @4
    4: ret r0
}
function f70 "fs.path::NativePathStyle.value" {
    registers [t44, t9, t9, t26, t16]
    params [r0]
    result t16
    0: jmp @1
    1: jmp @2
    2: call r1, f75, []
    3: const r2, str "windows"
    4: str.cmp eq, r3, r1, r2
    5: br r3, @6, @9
    6: jmp @7
    7: enum.case r4, enum1, 1
    8: jmp @12
    9: jmp @10
    10: enum.case r4, enum1, 0
    11: jmp @12
    12: jmp @13
    13: ret r4
}
function f71 "runtime.network::unix_path_valid" native "runtime/network@1::unix_path_valid/1" {
    registers [t6]
    params [r0]
    result t26
}
function f72 "encoding.utf::Utf8Encoded.new#0" {
    registers [t9, t46]
    params [r0]
    result t46
    0: jmp @1
    1: jmp @2
    2: obj.new r1, obj24, [r0]
    3: jmp @4
    4: ret r1
}
function f73 "encoding.utf::Utf8Encoded.bytes" {
    registers [t46, t9, t6]
    params [r0]
    result t6
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: call r2, f76, [r1]
    4: jmp @5
    5: ret r2
}
function f74 "runtime.network::closed" native "runtime/network@1::closed/1" {
    registers [t11]
    params [r0]
    result t26
}
function f75 "runtime.system::system_platform" native "runtime/system@1::system_platform/0" {
    registers []
    params []
    result t9
}
function f76 "runtime.utf::utf8_encode" native "runtime/utf@1::utf8_encode/1" {
    registers [t9]
    params [r0]
    result t6
}
entry f0
