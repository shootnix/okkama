sparrow 9
module "async_atomic" "v1.0.0"
type t0 bool
type t1 str
type t2 object obj0
type t3 failure
type t4 object obj1
type t5 int
role role0 "io.stream::Closing" {
    method "closed" [] -> t0
    method "close" [] -> void
}
role role1 "Error" builtin {
    method "text" [] -> t1
}
object obj0 "async.atomic::Atomic<string>" {
    does role0 [f1, f2]
}
object obj1 "async.atomic::AtomicClosed" {
    does role1 [f7]
}
function f0 "main" {
    registers [t1, t2, t1, t1, t0, t0, t0, t3, t4, t0, t1, t0, t0, t5, t3, t3]
    params []
    result t5
    handler 15 18 @18 r7
    handler 19 21 @84 r14
    handler 22 26 @26 r7
    handler 27 29 @81 r14
    handler 30 35 @35 r7
    handler 36 38 @78 r14
    0: const r0, str "one"
    1: call r1, f3, [r0]
    2: call r0, f4, [r1]
    3: const r2, str "two"
    4: call _, f5, [r1, r2]
    5: const r2, str "one"
    6: const r3, str "wrong"
    7: call r4, f6, [r1, r2, r3]
    8: const r2, str "two"
    9: const r3, str "three"
    10: call r5, f6, [r1, r2, r3]
    11: call r2, f4, [r1]
    12: call _, f2, [r1]
    13: call _, f2, [r1]
    14: jmp @15
    15: call r3, f4, [r1]
    16: const r6, bool false
    17: jmp @21
    18: catch r7, object, 1, r8, @19, @86
    19: const r6, bool true
    20: jmp @21
    21: jmp @22
    22: const r3, str "four"
    23: call _, f5, [r1, r3]
    24: const r9, bool false
    25: jmp @29
    26: catch r7, object, 1, r8, @27, @83
    27: const r9, bool true
    28: jmp @29
    29: jmp @30
    30: const r3, str "three"
    31: const r10, str "four"
    32: call r11, f6, [r1, r3, r10]
    33: const r11, bool false
    34: jmp @38
    35: catch r7, object, 1, r8, @36, @80
    36: const r11, bool true
    37: jmp @38
    38: const r3, str "one"
    39: str.cmp eq, r12, r0, r3
    40: br r12, @41, @44
    41: const r0, str "three"
    42: str.cmp eq, r12, r2, r0
    43: jmp @46
    44: const r12, bool false
    45: jmp @46
    46: br r12, @47, @49
    47: bool.not r12, r4
    48: jmp @51
    49: const r4, bool false
    50: jmp @51(r12 <- r4)
    51: br r12, @52, @53
    52: jmp @55
    53: const r4, bool false
    54: jmp @55(r5 <- r4)
    55: br r5, @56, @58
    56: call r4, f1, [r1]
    57: jmp @60
    58: const r4, bool false
    59: jmp @60
    60: br r4, @61, @62
    61: jmp @64
    62: const r4, bool false
    63: jmp @64(r6 <- r4)
    64: br r6, @65, @66
    65: jmp @68
    66: const r4, bool false
    67: jmp @68(r9 <- r4)
    68: br r9, @69, @70
    69: jmp @72
    70: const r4, bool false
    71: jmp @72(r11 <- r4)
    72: br r11, @73, @75
    73: const r13, int 42
    74: jmp @77
    75: const r13, int 0
    76: jmp @77
    77: ret r13
    78: fail.caused r15, r14, r7
    79: raise r15
    80: raise r7
    81: fail.caused r15, r14, r7
    82: raise r15
    83: raise r7
    84: fail.caused r15, r14, r7
    85: raise r15
    86: raise r7
}
function f1 "async.atomic::Atomic<string>.closed" intrinsic "okkama.atomic.Atomic.closed" [] [] {
    registers [t2]
    params [r0]
    result t0
}
function f2 "async.atomic::Atomic<string>.close" intrinsic "okkama.atomic.Atomic.close" [] [] {
    registers [t2]
    params [r0]
    result void
}
function f3 "async.atomic::Atomic<string>.new#0" intrinsic "okkama.atomic.Atomic.new_value" [obj0] [] {
    registers [t1]
    params [r0]
    result t2
}
function f4 "async.atomic::Atomic<string>.value" intrinsic "okkama.atomic.Atomic.value" [obj1] [] {
    registers [t2]
    params [r0]
    result t1
}
function f5 "async.atomic::Atomic<string>.replace" intrinsic "okkama.atomic.Atomic.replace" [obj1] [] {
    registers [t2, t1]
    params [r0, r1]
    result void
}
function f6 "async.atomic::Atomic<string>.compare_and_replace" intrinsic "okkama.atomic.Atomic.compare_and_replace" [obj1] [] {
    registers [t2, t1, t1]
    params [r0, r1, r2]
    result t0
}
function f7 "async.atomic::AtomicClosed.text" {
    registers [t4, t1]
    params [r0]
    result t1
    0: const r1, str "atomic state is closed"
    1: ret r1
}
entry f0
