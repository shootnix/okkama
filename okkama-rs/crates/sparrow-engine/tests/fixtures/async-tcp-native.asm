sparrow 9
module "main" "v1.0.0"
type t0 handle 0
type t1 future t0
type t2 int
type t3 str
type t4 byte
type t5 array t4
type t6 bool
type t7 handle 4
type t8 array t3
type t9 object obj0
type t10 future t9
type t11 future t5
type t12 failure
type t13 object obj1
type t14 future t6
type t15 object obj2
role role0 "async.scope::Supervising<SocketState>" {
    method "start" [t1] -> void
    method "active" [] -> t2
    method "cancel" [] -> void
}
role role1 "Error" builtin {
    method "text" [] -> t3
}
object obj0 "async.scope::FutureGroup<SocketState>" {
    does role0 [f5, f6, f7]
}
object obj1 "NativeFailure" builtin {
    field "domain" t3
    field "code" t3
    field "operation" t3
    field "message" t3
    does role1 [f15]
}
object obj2 "async.scope::FinishedFutureGroup" {
    does role1 [f22]
}
handle "runtime.network::SocketState"
handle "runtime.network::MembershipState"
handle "runtime.network::ListenerState"
handle "runtime.network::PairState"
handle "runtime.network::TcpListenerState"
function f0 "main" async {
    registers [t2, t3, t2, t5, t6, t6, t7, t8, t9, t1, t3, t2, t1, t0, t0, t10, t11, t11, t12, t13, t4, t4, t4, t14, t5, t12, t12]
    params []
    result t2
    handler 46 51 @51 r18
    handler 52 56 @118 r25
    handler 89 92 @92 r18
    handler 93 97 @109 r25
    0: const r0, int 1
    1: const r1, str "127.0.0.1"
    2: const r2, int 0
    3: call r3, f2, [r0, r1, r2]
    4: const r1, str ""
    5: const r4, bool false
    6: const r5, bool false
    7: call r6, f1, [r3, r1, r4, r5]
    8: const r0, int 32
    9: call r4, f3, [r6, r0]
    10: call r7, f4, [r6]
    11: call r8, f8, []
    12: const r4, bool false
    13: const r5, bool true
    14: call r9, f9, [r6, r4, r5]
    15: call _, f5, [r8, r9]
    16: const r0, int 1
    17: const r2, int 0
    18: arr.get r1, r7, r2
    19: const r2, int 1
    20: arr.get r10, r7, r2
    21: const r2, int 10
    22: call r11, f11, [r10, r2]
    23: call r3, f2, [r0, r1, r11]
    24: const r1, str ""
    25: const r4, bool false
    26: const r5, bool true
    27: call r12, f10, [r3, r1, r4, r5]
    28: await r13, r12
    29: await r14, r9
    30: call r15, f12, [r8]
    31: await r8, r15
    32: const r0, int 3
    33: call r16, f13, [r14, r0]
    34: fut cancel, _, r16
    35: fut cancelled, r4, r16
    36: br r4, @37, @40
    37: call r4, f14, [r14]
    38: bool.not r5, r4
    39: jmp @42
    40: const r4, bool false
    41: jmp @42(r5 <- r4)
    42: br r5, @43, @121
    43: const r0, int 3
    44: call r16, f13, [r14, r0]
    45: jmp @46
    46: const r0, int 1
    47: call r17, f13, [r14, r0]
    48: await r3, r17
    49: const r4, bool false
    50: jmp @56
    51: catch r18, object, 1, r19, @52, @120
    52: call r1, f16, [r19]
    53: const r10, str "resource_exhausted"
    54: str.cmp eq, r4, r1, r10
    55: jmp @56
    56: br r4, @57, @116
    57: const r0, int 0
    58: byte.from r20, r0, _
    59: const r0, int 128
    60: byte.from r21, r0, _
    61: const r0, int 255
    62: byte.from r22, r0, _
    63: arr.new r3, [r20, r21, r22]
    64: call r23, f17, [r13, r3]
    65: await r4, r23
    66: await r3, r16
    67: const r0, int 0
    68: byte.from r20, r0, _
    69: const r0, int 128
    70: byte.from r21, r0, _
    71: const r0, int 255
    72: byte.from r22, r0, _
    73: arr.new r24, [r20, r21, r22]
    74: val.cmp eq, r4, r3, r24
    75: br r4, @76, @114
    76: const r4, bool true
    77: call r5, f18, [r13, r4]
    78: const r0, int 1
    79: call r16, f13, [r14, r0]
    80: await r3, r16
    81: arr.len r0, r3
    82: const r2, int 0
    83: int.cmp eq, r4, r0, r2
    84: br r4, @85, @112
    85: const r0, int 1
    86: call r16, f13, [r13, r0]
    87: call r4, f19, [r13]
    88: jmp @89
    89: await r3, r16
    90: const r4, bool false
    91: jmp @97
    92: catch r18, object, 1, r19, @93, @111
    93: call r1, f16, [r19]
    94: const r10, str "already_closed"
    95: str.cmp eq, r4, r1, r10
    96: jmp @97
    97: br r4, @98, @107
    98: call r4, f19, [r13]
    99: call r4, f19, [r14]
    100: call r4, f20, [r6]
    101: call r4, f21, [r6]
    102: br r4, @103, @105
    103: const r0, int 42
    104: ret r0
    105: const r1, str "listener remained open"
    106: panic r1
    107: const r1, str "close failed to wake reader"
    108: panic r1
    109: fail.caused r26, r25, r18
    110: raise r26
    111: raise r18
    112: const r1, str "EOF missing"
    113: panic r1
    114: const r1, str "payload changed"
    115: panic r1
    116: const r1, str "duplicate reader accepted"
    117: panic r1
    118: fail.caused r26, r25, r18
    119: raise r26
    120: raise r18
    121: const r1, str "read cancellation closed stream"
    122: panic r1
}
function f1 "runtime.network::tcp_bound" native "runtime/network@1::tcp_bound/4" {
    registers [t5, t3, t6, t6]
    params [r0, r1, r2, r3]
    result t7
}
function f2 "runtime.net::ip_parse" native "runtime/net@1::ip_parse/3" {
    registers [t2, t3, t2]
    params [r0, r1, r2]
    result t5
}
function f3 "runtime.network::tcp_start_listening" native "runtime/network@1::tcp_start_listening/2" {
    registers [t7, t2]
    params [r0, r1]
    result t6
}
function f4 "runtime.network::tcp_listener_endpoint" native "runtime/network@1::tcp_listener_endpoint/1" {
    registers [t7]
    params [r0]
    result t8
}
function f5 "async.scope::FutureGroup<SocketState>.start" intrinsic "okkama.scope.FutureGroup.start" [obj2] [] {
    registers [t9, t1]
    params [r0, r1]
    result void
}
function f6 "async.scope::FutureGroup<SocketState>.active" intrinsic "okkama.scope.FutureGroup.active" [] [] {
    registers [t9]
    params [r0]
    result t2
}
function f7 "async.scope::FutureGroup<SocketState>.cancel" intrinsic "okkama.scope.FutureGroup.cancel" [] [] {
    registers [t9]
    params [r0]
    result void
}
function f8 "async.scope::FutureGroup<SocketState>.new#0" intrinsic "okkama.scope.FutureGroup.new" [obj0] [] {
    registers []
    params []
    result t9
}
function f9 "runtime.network::tcp_accept" async native "runtime/network@1::tcp_accept/3" {
    registers [t7, t6, t6]
    params [r0, r1, r2]
    result t0
}
function f10 "runtime.network::tcp_connect" async native "runtime/network@1::tcp_connect/4" {
    registers [t5, t3, t6, t6]
    params [r0, r1, r2, r3]
    result t0
}
function f11 "runtime.number_text::int_parse" native "runtime/number_text@1::int_parse/2" {
    registers [t3, t2]
    params [r0, r1]
    result t2
}
function f12 "async.scope::FutureGroup<SocketState>.wait" async intrinsic "okkama.scope.FutureGroup.wait" [] [] {
    registers [t9]
    params [r0]
    result t9
}
function f13 "runtime.network::stream_read" async native "runtime/network@1::stream_read/2" {
    registers [t0, t2]
    params [r0, r1]
    result t5
}
function f14 "runtime.network::closed" native "runtime/network@1::closed/1" {
    registers [t0]
    params [r0]
    result t6
}
function f15 "NativeFailure.text" {
    registers [t13, t3]
    params [r0]
    result t3
    0: obj.get r1, r0, 3
    1: ret r1
}
function f16 "NativeFailure.code" {
    registers [t13, t3]
    params [r0]
    result t3
    0: obj.get r1, r0, 1
    1: ret r1
}
function f17 "runtime.network::stream_write" async native "runtime/network@1::stream_write/2" {
    registers [t0, t5]
    params [r0, r1]
    result t6
}
function f18 "runtime.network::stream_shutdown" native "runtime/network@1::stream_shutdown/2" {
    registers [t0, t6]
    params [r0, r1]
    result t6
}
function f19 "runtime.network::close" native "runtime/network@1::close/1" {
    registers [t0]
    params [r0]
    result t6
}
function f20 "runtime.network::tcp_listener_close" native "runtime/network@1::tcp_listener_close/1" {
    registers [t7]
    params [r0]
    result t6
}
function f21 "runtime.network::tcp_listener_closed" native "runtime/network@1::tcp_listener_closed/1" {
    registers [t7]
    params [r0]
    result t6
}
function f22 "async.scope::FinishedFutureGroup.text" {
    registers [t15, t3]
    params [r0]
    result t3
    0: const r1, str "future group is already finishing"
    1: ret r1
}
entry f0
