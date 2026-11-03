sparrow 9
module "main" "v1.0.0"
type t0 str
type t1 array t0
type t2 array t1
type t3 future t2
type t4 int
type t5 bool
type t6 object obj0
type t7 future t6
type t8 failure
type t9 object obj1
type t10 future t0
type t11 object obj2
role role0 "async.scope::Supervising<@@string>" {
    method "start" [t3] -> void
    method "active" [] -> t4
    method "cancel" [] -> void
}
role role1 "Error" builtin {
    method "text" [] -> t0
}
object obj0 "async.scope::FutureGroup<@@string>" {
    does role0 [f2, f3, f4]
}
object obj1 "NativeFailure" builtin {
    field "domain" t0
    field "code" t0
    field "operation" t0
    field "message" t0
    does role1 [f7]
}
object obj2 "async.scope::FinishedFutureGroup" {
    does role1 [f11]
}
handle "runtime.network::SocketState"
handle "runtime.network::MembershipState"
handle "runtime.network::ListenerState"
handle "runtime.network::PairState"
handle "runtime.network::TcpListenerState"
function f0 "main" async {
    registers [t0, t4, t3, t5, t6, t2, t2, t4, t1, t0, t7, t8, t9, t10, t8, t8]
    params []
    result t4
    handler 50 56 @56 r11
    handler 57 61 @89 r14
    handler 61 65 @89 r14
    handler 65 67 @89 r14
    handler 69 74 @74 r11
    handler 75 79 @84 r14
    0: const r0, str "must-not-resolve.invalid"
    1: const r1, int 42
    2: call r2, f1, [r0, r1]
    3: fut cancel, _, r2
    4: fut cancelled, r3, r2
    5: br r3, @6, @94
    6: const r0, str "127.0.0.1"
    7: const r1, int 4242
    8: call r2, f1, [r0, r1]
    9: call r4, f5, []
    10: call _, f2, [r4, r2]
    11: await r5, r2
    12: await r6, r2
    13: arr.len r1, r5
    14: const r7, int 1
    15: int.cmp eq, r3, r1, r7
    16: br r3, @17, @24
    17: const r1, int 0
    18: arr.get r8, r6, r1
    19: const r1, int 0
    20: arr.get r0, r8, r1
    21: const r9, str "127.0.0.1"
    22: str.cmp eq, r3, r0, r9
    23: jmp @26
    24: const r3, bool false
    25: jmp @26
    26: br r3, @27, @34
    27: const r1, int 0
    28: arr.get r8, r5, r1
    29: const r1, int 1
    30: arr.get r0, r8, r1
    31: const r9, str "4242"
    32: str.cmp eq, r3, r0, r9
    33: jmp @36
    34: const r3, bool false
    35: jmp @36
    36: br r3, @37, @44
    37: const r1, int 0
    38: arr.get r8, r5, r1
    39: const r1, int 2
    40: arr.get r0, r8, r1
    41: const r9, str ""
    42: str.cmp eq, r3, r0, r9
    43: jmp @46
    44: const r3, bool false
    45: jmp @46
    46: br r3, @47, @92
    47: call r10, f6, [r4]
    48: await r4, r10
    49: jmp @50
    50: const r0, str "bad name"
    51: const r1, int 0
    52: call r2, f1, [r0, r1]
    53: await r5, r2
    54: const r3, bool false
    55: jmp @68
    56: catch r11, object, 1, r12, @57, @91
    57: call r0, f8, [r12]
    58: const r9, str "core.net"
    59: str.cmp eq, r3, r0, r9
    60: br r3, @61, @65
    61: call r0, f9, [r12]
    62: const r9, str "invalid_input"
    63: str.cmp eq, r3, r0, r9
    64: jmp @67
    65: const r3, bool false
    66: jmp @67
    67: jmp @68
    68: br r3, @69, @87
    69: const r0, str "not-an-ip"
    70: call r13, f10, [r0]
    71: await r0, r13
    72: const r3, bool false
    73: jmp @79
    74: catch r11, object, 1, r12, @75, @86
    75: call r0, f9, [r12]
    76: const r9, str "invalid_input"
    77: str.cmp eq, r3, r0, r9
    78: jmp @79
    79: br r3, @80, @82
    80: const r1, int 42
    81: ret r1
    82: const r0, str "invalid reverse lookup"
    83: panic r0
    84: fail.caused r15, r14, r11
    85: raise r15
    86: raise r11
    87: const r0, str "invalid name escaped native failure"
    88: panic r0
    89: fail.caused r15, r14, r11
    90: raise r15
    91: raise r11
    92: const r0, str "bad resolver answer"
    93: panic r0
    94: const r0, str "unstarted resolution did not cancel"
    95: panic r0
}
function f1 "runtime.network::dns_addresses" async native "runtime/network@1::dns_addresses/2" {
    registers [t0, t4]
    params [r0, r1]
    result t2
}
function f2 "async.scope::FutureGroup<@@string>.start" intrinsic "okkama.scope.FutureGroup.start" [obj2] [] {
    registers [t6, t3]
    params [r0, r1]
    result void
}
function f3 "async.scope::FutureGroup<@@string>.active" intrinsic "okkama.scope.FutureGroup.active" [] [] {
    registers [t6]
    params [r0]
    result t4
}
function f4 "async.scope::FutureGroup<@@string>.cancel" intrinsic "okkama.scope.FutureGroup.cancel" [] [] {
    registers [t6]
    params [r0]
    result void
}
function f5 "async.scope::FutureGroup<@@string>.new#0" intrinsic "okkama.scope.FutureGroup.new" [obj0] [] {
    registers []
    params []
    result t6
}
function f6 "async.scope::FutureGroup<@@string>.wait" async intrinsic "okkama.scope.FutureGroup.wait" [] [] {
    registers [t6]
    params [r0]
    result t6
}
function f7 "NativeFailure.text" {
    registers [t9, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 3
    1: ret r1
}
function f8 "NativeFailure.domain" {
    registers [t9, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 0
    1: ret r1
}
function f9 "NativeFailure.code" {
    registers [t9, t0]
    params [r0]
    result t0
    0: obj.get r1, r0, 1
    1: ret r1
}
function f10 "runtime.network::dns_name" async native "runtime/network@1::dns_name/1" {
    registers [t0]
    params [r0]
    result t0
}
function f11 "async.scope::FinishedFutureGroup.text" {
    registers [t11, t0]
    params [r0]
    result t0
    0: const r1, str "future group is already finishing"
    1: ret r1
}
entry f0
