sparrow 9
module "app" "v1.0.0"
type t0 role role1
type t1 byte
type t2 array t1
type t3 str
type t4 array t3
type t5 int
type t6 object obj0
type t7 array t5
type t8 object obj1
type t9 any
type t10 array t9
type t11 object obj2
type t12 never
type t13 array t12
type t14 bool
type t15 object obj3
type t16 object obj4
type t17 object obj5
enum enum0 "encoding.utf::ByteOrder" ["big", "little"]
role role0 "io.stream::Writing" {
    method "write" [t0] -> void
    method "flush" [] -> void
}
role role1 "io.stream::ByteSource" {
    method "bytes" [] -> t2
}
role role2 "io.stream::WritingText" {
    method "write" [t4] -> void
    method "writeln" [t4] -> void
}
role role3 "Error" builtin {
    method "text" [] -> t3
}
object obj0 "Taker" {
}
object obj1 "Box<int>" {
    field "value" t5
}
object obj2 "io.stdout::Stdout" {
    does role0 [f6, f7]
    does role2 [f8, f9]
}
object obj3 "io.stream::IncompleteWrite" {
    field "amount" t5
    does role3 [f14]
}
object obj4 "encoding.utf::Utf8Encoded" {
    field "origin" t3
}
object obj5 "error::InvalidArgument" {
    field "message" t3
    does role3 [f19]
}
function f0 "Taker.new#0" {
    registers [t6]
    params []
    result t6
    0: obj.new r0, obj0, []
    1: ret r0
}
function f1 "Taker.ints" {
    registers [t6, t7, t5]
    params [r0, r1]
    result t5
    0: arr.len r2, r1
    1: ret r2
}
function f2 "Taker.boxed" {
    registers [t6, t8, t5]
    params [r0, r1]
    result t5
    0: call r2, f5, [r1]
    1: ret r2
}
function f3 "Taker.anything" {
    registers [t6, t10, t5]
    params [r0, r1]
    result t5
    0: arr.len r2, r1
    1: ret r2
}
function f4 "main" {
    registers [t11, t5, t5, t5, t7, t6, t3, t4, t13]
    params []
    result t5
    0: call r0, f10, []
    1: const r1, int 1
    2: const r2, int 2
    3: const r3, int 3
    4: arr.new r4, [r1, r2, r3]
    5: call r5, f0, []
    6: call r1, f1, [r5, r4]
    7: cvt r6, r1
    8: arr.new r7, [r6]
    9: call _, f9, [r0, r7]
    10: arr.new r8, []
    11: call r5, f0, []
    12: move r4, r8
    13: call r1, f1, [r5, r4]
    14: cvt r6, r1
    15: arr.new r7, [r6]
    16: call _, f9, [r0, r7]
    17: const r1, int 0
    18: ret r1
}
function f5 "Box<int>.get" {
    registers [t8, t5]
    params [r0]
    result t5
    0: obj.get r1, r0, 0
    1: ret r1
}
function f6 "io.stdout::Stdout.write#0" {
    registers [t11, t0, t2]
    params [r0, r1]
    result void
    0: call.role r2, r1, role1, 0, []
    1: call _, f11, [r0, r2]
    2: ret _
}
function f7 "io.stdout::Stdout.flush" {
    registers [t11]
    params [r0]
    result void
    0: ret _
}
function f8 "io.stdout::Stdout.write#1" {
    registers [t11, t4, t5, t5, t14, t3, t5, t5]
    params [r0, r1]
    result void
    0: const r2, int 0
    1: arr.len r3, r1
    2: const r6, int 1
    3: jmp @4
    4: int.cmp lt, r4, r2, r3
    5: br r4, @6, @11
    6: arr.get r5, r1, r2
    7: call _, f12, [r0, r5]
    8: jmp @9
    9: int.add r7, r2, r6
    10: jmp @4(r2 <- r7)
    11: ret _
}
function f9 "io.stdout::Stdout.writeln" {
    registers [t11, t4, t5, t5, t14, t3, t5, t5]
    params [r0, r1]
    result void
    0: const r2, int 0
    1: arr.len r3, r1
    2: const r6, int 1
    3: jmp @4
    4: int.cmp lt, r4, r2, r3
    5: br r4, @6, @11
    6: arr.get r5, r1, r2
    7: call _, f12, [r0, r5]
    8: jmp @9
    9: int.add r7, r2, r6
    10: jmp @4(r2 <- r7)
    11: const r5, str "\n"
    12: call _, f12, [r0, r5]
    13: ret _
}
function f10 "io.stdout::Stdout.new#0" {
    registers [t11]
    params []
    result t11
    0: obj.new r0, obj2, []
    1: ret r0
}
function f11 "io.stdout::Stdout.write_bytes" {
    registers [t11, t2, t5, t5, t14, t15]
    params [r0, r1]
    result void
    0: call r2, f13, [r1]
    1: arr.len r3, r1
    2: int.cmp eq, r4, r2, r3
    3: br r4, @4, @5
    4: ret _
    5: call r5, f15, [r2]
    6: throw r5
}
function f12 "io.stdout::Stdout.write_text" {
    registers [t11, t3, t16, t2]
    params [r0, r1]
    result void
    0: call r2, f16, [r1]
    1: call r3, f17, [r2]
    2: call _, f11, [r0, r3]
    3: ret _
}
function f13 "runtime.stdio::stdout_write" native "runtime/stdio@1::stdout_write/1" {
    registers [t2]
    params [r0]
    result t5
}
function f14 "io.stream::IncompleteWrite.text" {
    registers [t15, t3, t5, t3, t3]
    params [r0]
    result t3
    0: const r1, str "incomplete write after "
    1: obj.get r2, r0, 0
    2: cvt r3, r2
    3: str.concat r4, r1, r3
    4: const r1, str " bytes"
    5: str.concat r3, r4, r1
    6: ret r3
}
function f15 "io.stream::IncompleteWrite.new#0" {
    registers [t5, t5, t14, t15, t3, t17]
    params [r0]
    result t15
    0: const r1, int 0
    1: int.cmp ge, r2, r0, r1
    2: br r2, @3, @5
    3: obj.new r3, obj3, [r0]
    4: ret r3
    5: const r4, str "written byte count cannot be negative"
    6: call r5, f20, [r4]
    7: throw r5
}
function f16 "encoding.utf::Utf8Encoded.new#0" {
    registers [t3, t16]
    params [r0]
    result t16
    0: obj.new r1, obj4, [r0]
    1: ret r1
}
function f17 "encoding.utf::Utf8Encoded.bytes" {
    registers [t16, t3, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 0
    1: call r2, f21, [r1]
    2: ret r2
}
function f18 "error::InvalidArgument.default.message" {
    registers [t3]
    params []
    result t3
    0: const r0, str "invalid argument"
    1: ret r0
}
function f19 "error::InvalidArgument.text" {
    registers [t17, t3]
    params [r0]
    result t3
    0: obj.get r1, r0, 0
    1: ret r1
}
function f20 "error::InvalidArgument.new#1" {
    registers [t3, t3, t17]
    params [r0]
    result t17
    0: call r1, f18, []
    1: obj.new r2, obj5, [r0]
    2: ret r2
}
function f21 "runtime.utf::utf8_encode" native "runtime/utf@1::utf8_encode/1" {
    registers [t3]
    params [r0]
    result t2
}
entry f4
