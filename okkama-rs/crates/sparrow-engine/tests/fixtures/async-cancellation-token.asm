sparrow 9
module "async_cancellation_token" "v1.0.0"
type t0 bool
type t1 role role0
type t2 future t1
type t3 str
type t4 object obj0
type t5 int
type t6 object obj1
type t7 future t5
type t8 failure
type t9 object obj2
type t10 object obj3
role role0 "async.cancellation::Cancelling" {
    method "requested" [] -> t0
    method "check" [] -> void
    method "wait" [] -> t2
}
role role1 "Error" builtin {
    method "text" [] -> t3
}
object obj0 "Reader" {
    field "token" t1
}
object obj1 "async.cancellation::Cancellation" {
}
object obj2 "Cancelled" builtin {
    does role1 [f6]
}
object obj3 "async.cancellation::CancellationToken" {
    does role0 [f7, f8, f9]
}
function f0 "Reader.new#0" {
    registers [t1, t4]
    params [r0]
    result t4
    0: obj.new r1, obj0, [r0]
    1: ret r1
}
function f1 "Reader.run" async {
    registers [t4, t1, t2, t5]
    params [r0]
    result t5
    0: obj.get r1, r0, 0
    1: call.role r2, r1, role0, 2, []
    2: await r1, r2
    3: const r3, int 21
    4: ret r3
}
function f2 "main" async {
    registers [t6, t1, t4, t7, t7, t1, t7, t0, t0, t5, t8, t9, t5, t2, t5, t8, t8]
    params []
    result t5
    handler 16 19 @19 r10
    handler 20 22 @50 r15
    handler 23 26 @26 r10
    handler 27 29 @47 r15
    0: call r0, f3, []
    1: call r1, f4, [r0]
    2: call.role _, r1, role0, 1, []
    3: call r2, f0, [r1]
    4: call r3, f1, [r2]
    5: fut.spawn r4, r3
    6: call r5, f4, [r0]
    7: call r2, f0, [r5]
    8: call r3, f1, [r2]
    9: fut.spawn r6, r3
    10: fut cancel, _, r4
    11: call.role r7, r1, role0, 0, []
    12: bool.not r8, r7
    13: call _, f5, [r0]
    14: call _, f5, [r0]
    15: jmp @16
    16: call.role _, r1, role0, 1, []
    17: const r9, int 0
    18: jmp @22
    19: catch r10, object, 2, r11, @20, @52
    20: const r9, int 21
    21: jmp @22
    22: jmp @23
    23: await r12, r4
    24: const r7, bool false
    25: jmp @29
    26: catch r10, object, 2, r11, @27, @49
    27: const r7, bool true
    28: jmp @29
    29: await r12, r6
    30: call.role r13, r1, role0, 2, []
    31: await r5, r13
    32: br r8, @33, @34
    33: jmp @36
    34: const r7, bool false
    35: jmp @36
    36: br r7, @37, @39
    37: call.role r7, r1, role0, 0, []
    38: jmp @41
    39: const r7, bool false
    40: jmp @41
    41: br r7, @42, @44
    42: int.add r14, r9, r12
    43: jmp @46
    44: const r9, int 0
    45: jmp @46(r14 <- r9)
    46: ret r14
    47: fail.caused r16, r15, r10
    48: raise r16
    49: raise r10
    50: fail.caused r16, r15, r10
    51: raise r16
    52: raise r10
}
function f3 "async.cancellation::Cancellation.new#0" intrinsic "okkama.cancellation.Cancellation.new" [obj1] [] {
    registers []
    params []
    result t6
}
function f4 "async.cancellation::Cancellation.cancelling" intrinsic "okkama.cancellation.Cancellation.cancelling" [obj3] [] {
    registers [t6]
    params [r0]
    result t1
}
function f5 "async.cancellation::Cancellation.cancel" intrinsic "okkama.cancellation.Cancellation.cancel" [] [] {
    registers [t6]
    params [r0]
    result void
}
function f6 "Cancelled.text" {
    registers [t9, t3]
    params [r0]
    result t3
    0: const r1, str "Operation was cancelled"
    1: ret r1
}
function f7 "async.cancellation::CancellationToken.requested" intrinsic "okkama.cancellation.CancellationToken.requested" [] [] {
    registers [t10]
    params [r0]
    result t0
}
function f8 "async.cancellation::CancellationToken.check" intrinsic "okkama.cancellation.CancellationToken.check" [] [] {
    registers [t10]
    params [r0]
    result void
}
function f9 "async.cancellation::CancellationToken.wait" async intrinsic "okkama.cancellation.CancellationToken.wait" [] [] {
    registers [t10]
    params [r0]
    result t1
}
entry f2
