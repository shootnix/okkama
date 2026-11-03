sparrow 9
module "main" "v1.0.0"
type t0 str
type t1 int
type t2 byte
type t3 array t2
type t4 bool
type t5 handle 0
type t6 array t0
type t7 future t3
type t8 future t1
type t9 future t4
type t10 failure
type t11 object obj0
role role0 "Error" builtin {
    method "text" [] -> t0
}
object obj0 "NativeFailure" builtin {
    field "domain" t0
    field "code" t0
    field "operation" t0
    field "message" t0
    does role0 [f14]
}
handle "runtime.network::SocketState"
handle "runtime.network::MembershipState"
handle "runtime.network::ListenerState"
handle "runtime.network::PairState"
handle "runtime.network::TcpListenerState"
function f0 "main" async {
    registers [t1, t0, t1, t3, t4, t4, t4, t5, t5, t6, t0, t1, t7, t2, t2, t2, t3, t8, t3, t3, t9, t10, t11, t10, t10]
    params []
    result t1
    handler 94 97 @97 r21
    handler 98 102 @108 r23
    0: const r0, int 1
    1: const r1, str "127.0.0.1"
    2: const r2, int 0
    3: call r3, f2, [r0, r1, r2]
    4: const r1, str ""
    5: const r4, bool false
    6: const r5, bool false
    7: const r6, bool false
    8: call r7, f1, [r3, r1, r4, r5, r6]
    9: const r0, int 1
    10: const r1, str "127.0.0.1"
    11: const r2, int 0
    12: call r3, f2, [r0, r1, r2]
    13: const r1, str ""
    14: const r4, bool false
    15: const r5, bool false
    16: const r6, bool false
    17: call r8, f1, [r3, r1, r4, r5, r6]
    18: call r9, f3, [r8]
    19: const r0, int 1
    20: const r2, int 0
    21: arr.get r1, r9, r2
    22: const r2, int 1
    23: arr.get r10, r9, r2
    24: const r2, int 10
    25: call r11, f4, [r10, r2]
    26: call r3, f2, [r0, r1, r11]
    27: const r0, int 1
    28: call r12, f5, [r8, r0]
    29: fut cancel, _, r12
    30: call r4, f6, [r8]
    31: bool.not r5, r4
    32: br r5, @33, @120
    33: const r0, int 2
    34: call r12, f5, [r8, r0]
    35: const r1, str ""
    36: const r0, int 0
    37: byte.from r13, r0, _
    38: const r0, int 255
    39: byte.from r14, r0, _
    40: const r0, int 42
    41: byte.from r15, r0, _
    42: arr.new r16, [r13, r14, r15]
    43: call r17, f7, [r7, r3, r1, r16]
    44: await r0, r17
    45: const r2, int 3
    46: int.cmp eq, r4, r0, r2
    47: br r4, @48, @118
    48: await r16, r12
    49: call r4, f8, [r16]
    50: br r4, @51, @59
    51: call r18, f9, [r16]
    52: const r0, int 0
    53: byte.from r13, r0, _
    54: const r0, int 255
    55: byte.from r14, r0, _
    56: arr.new r19, [r13, r14]
    57: val.cmp eq, r4, r18, r19
    58: jmp @61
    59: const r4, bool false
    60: jmp @61
    61: br r4, @62, @116
    62: call r9, f10, [r16]
    63: const r0, int 1
    64: arr.get r1, r9, r0
    65: call r9, f3, [r7]
    66: const r0, int 1
    67: arr.lookup r9, r0, r10, @68, @114
    68: str.cmp eq, r4, r1, r10
    69: br r4, @70, @113
    70: const r1, str ""
    71: call r20, f11, [r7, r3, r1]
    72: await r4, r20
    73: arr.new r3, []
    74: call r17, f12, [r7, r3]
    75: await r0, r17
    76: const r0, int 1
    77: call r12, f5, [r8, r0]
    78: await r3, r12
    79: call r4, f8, [r3]
    80: bool.not r5, r4
    81: br r5, @82, @87
    82: call r16, f9, [r3]
    83: arr.len r0, r16
    84: const r2, int 0
    85: int.cmp eq, r4, r0, r2
    86: jmp @89
    87: const r4, bool false
    88: jmp @89
    89: br r4, @90, @111
    90: const r0, int 1
    91: call r12, f5, [r8, r0]
    92: call r4, f13, [r8]
    93: jmp @94
    94: await r3, r12
    95: const r4, bool false
    96: jmp @102
    97: catch r21, object, 0, r22, @98, @110
    98: call r1, f15, [r22]
    99: const r10, str "already_closed"
    100: str.cmp eq, r4, r1, r10
    101: jmp @102
    102: br r4, @103, @106
    103: call r4, f13, [r7]
    104: const r0, int 42
    105: ret r0
    106: const r1, str "UDP close did not wake receiver"
    107: panic r1
    108: fail.caused r24, r23, r21
    109: raise r24
    110: raise r21
    111: const r1, str "empty packet lost"
    112: panic r1
    113: trap confirm, "confirm failed"
    114: const r1, str "source changed"
    115: panic r1
    116: const r1, str "UDP truncation changed"
    117: panic r1
    118: const r1, str "partial packet"
    119: panic r1
    120: const r1, str "receive cancellation closed UDP"
    121: panic r1
}
function f1 "runtime.network::udp_bound" native "runtime/network@1::udp_bound/5" {
    registers [t3, t0, t4, t4, t4]
    params [r0, r1, r2, r3, r4]
    result t5
}
function f2 "runtime.net::ip_parse" native "runtime/net@1::ip_parse/3" {
    registers [t1, t0, t1]
    params [r0, r1, r2]
    result t3
}
function f3 "runtime.network::local_endpoint" native "runtime/network@1::local_endpoint/1" {
    registers [t5]
    params [r0]
    result t6
}
function f4 "runtime.number_text::int_parse" native "runtime/number_text@1::int_parse/2" {
    registers [t0, t1]
    params [r0, r1]
    result t1
}
function f5 "runtime.network::udp_receive" async native "runtime/network@1::udp_receive/2" {
    registers [t5, t1]
    params [r0, r1]
    result t3
}
function f6 "runtime.network::closed" native "runtime/network@1::closed/1" {
    registers [t5]
    params [r0]
    result t4
}
function f7 "runtime.network::udp_send" async native "runtime/network@1::udp_send/4" {
    registers [t5, t3, t0, t3]
    params [r0, r1, r2, r3]
    result t1
}
function f8 "runtime.network::packet_truncated" native "runtime/network@1::packet_truncated/1" {
    registers [t3]
    params [r0]
    result t4
}
function f9 "runtime.network::packet_bytes" native "runtime/network@1::packet_bytes/1" {
    registers [t3]
    params [r0]
    result t3
}
function f10 "runtime.network::packet_source" native "runtime/network@1::packet_source/1" {
    registers [t3]
    params [r0]
    result t6
}
function f11 "runtime.network::udp_connect" async native "runtime/network@1::udp_connect/3" {
    registers [t5, t3, t0]
    params [r0, r1, r2]
    result t4
}
function f12 "runtime.network::udp_connected_send" async native "runtime/network@1::udp_connected_send/2" {
    registers [t5, t3]
    params [r0, r1]
    result t1
}
function f13 "runtime.network::close" native "runtime/network@1::close/1" {
    registers [t5]
    params [r0]
    result t4
}
function f14 "NativeFailure.text" {
    registers [t11, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 3
    1: ret r1
}
function f15 "NativeFailure.code" {
    registers [t11, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 1
    1: ret r1
}
entry f0
