sparrow 9
module "test_profile" "v1.0.0"
type t0 bool
type t1 str
type t2 int
type t3 future t0
type t4 array t2
handle "runtime.testing::TestState"
function f0 "test#0" {
    registers [t0, t1, t1, t2, t2]
    params []
    result void
    0: call r0, f2, []
    1: call _, f3, [r0]
    2: const r0, bool true
    3: call r1, f4, [r0]
    4: const r0, bool true
    5: call r2, f4, [r0]
    6: call _, f5, [r1, r2]
    7: const r0, bool false
    8: call _, f3, [r0]
    9: const r3, int 1
    10: const r4, int 2
    11: call _, f6, [r3, r4]
    12: const r1, str "third failure"
    13: call _, f7, [r1]
    14: ret _
}
function f1 "test#1" async {
    registers [t1, t3, t0, t2, t2, t4, t4]
    params []
    result void
    0: const r0, str "1000"
    1: call r1, f8, [r0]
    2: await r2, r1
    3: const r3, int 1
    4: const r4, int 2
    5: arr.new r5, [r3, r4]
    6: const r3, int 1
    7: const r4, int 2
    8: arr.new r6, [r3, r4]
    9: call _, f9, [r5, r6]
    10: ret _
}
function f2 "runtime.testing::test_context_active" native "runtime/testing@1::test_context_active/0" {
    registers []
    params []
    result t0
}
function f3 "okkama.test.ok#3" intrinsic "okkama.test.ok" [] [] {
    registers [t0]
    params [r0]
    result void
}
function f4 "runtime.time::clock_domain" native "runtime/time@1::clock_domain/1" {
    registers [t0]
    params [r0]
    result t1
}
function f5 "okkama.test.isnt#5" intrinsic "okkama.test.isnt" [] [] {
    registers [t1, t1]
    params [r0, r1]
    result void
}
function f6 "okkama.test.is#6" intrinsic "okkama.test.is" [] [] {
    registers [t2, t2]
    params [r0, r1]
    result void
}
function f7 "okkama.test.fail#7" intrinsic "okkama.test.fail" [] [] {
    registers [t1]
    params [r0]
    result void
}
function f8 "runtime.time::sleep_nanoseconds" async native "runtime/time@1::sleep_nanoseconds/1" {
    registers [t1]
    params [r0]
    result t0
}
function f9 "okkama.test.is#9" intrinsic "okkama.test.is" [] [] {
    registers [t4, t4]
    params [r0, r1]
    result void
}
tests {
    test "assertions accumulate" f0
    test "async test continues after another failed test" f1
}
