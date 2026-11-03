sparrow 9
module "main" "v1.0.0"
type t0 object obj1
type t1 str
type t2 array t1
type t3 object obj2
type t4 role role2
type t5 enum enum0
type t6 role role0
type t7 handle 7
type t8 enum enum1
type t9 map t1 t1
type t10 object obj4
type t11 object obj3
type t12 object obj0
type t13 int
type t14 bool
type t15 object obj6
type t16 object obj5
type t17 failure
type t18 object obj7
type t19 object obj8
type t20 object obj9
enum enum0 "compare::Order" ["less", "equal", "greater"]
enum enum1 "fs.path::PathStyle" ["posix", "windows"]
enum enum2 "time.duration::DurationUnit" ["nanosecond", "microsecond", "millisecond", "second", "minute", "hour", "day"]
enum enum3 "fs.stat::FileKind" ["other", "file", "directory", "symbolic_link", "block_device", "character_device", "fifo", "socket"]
enum enum4 "encoding.utf::ByteOrder" ["big", "little"]
enum enum5 "fs.file::FileMode" ["reading", "writing", "appending", "reading_and_writing", "reading_and_appending"]
enum enum6 "fs.file::SeekOrigin" ["start", "current", "end"]
enum enum7 "os.process::Signal" ["interrupt", "terminate", "kill"]
enum enum8 "os.process::ExitKind" ["exited", "interrupted", "terminated", "killed", "other"]
role role0 "os.command::Commanding" {
    method "program" [] -> t0
    method "arguments" [] -> t2
    method "environment" [] -> t3
    method "directory" [] -> t4
    method "string" [] -> t1
}
role role1 "compare::Comparing<Path>" {
    method "order" [t0] -> t5
}
role role2 "fs.directory::DirectoryLocating" {
    method "path" [] -> t0
}
role role3 "Error" builtin {
    method "text" [] -> t1
}
object obj0 "os.process::Process" {
    field "origin" t6
    field "state" t7
}
object obj1 "fs.path::Path" {
    field "origin" t1
    field "path_style" t8
    does role1 [f1]
}
object obj2 "os.environment::Environment" {
    field "origin" t9
}
object obj3 "os.command::Command" {
    field "command_program" t0
    field "command_arguments" t2
    field "command_environment" t3
    field "command_directory" t4
    does role0 [f2, f3, f4, f5, f6]
}
object obj4 "fs.directory::Directory" {
    field "origin" t0
    does role2 [f9]
}
object obj5 "fs.path::InvalidPath" {
    field "message" t1
    does role3 [f18]
}
object obj6 "fs.path::NativePathStyle" {
}
object obj7 "NativeFailure" builtin {
    field "domain" t1
    field "code" t1
    field "operation" t1
    field "message" t1
    does role3 [f22]
}
object obj8 "os.environment::InvalidEnvironmentVariable" {
    field "message" t1
    does role3 [f24]
}
object obj9 "os.process::ProcessFailure" {
    field "failure_code" t1
    field "message" t1
    does role3 [f30]
}
handle "runtime.fs::FileHandle"
handle "runtime.fs::DirectoryHandle"
handle "runtime.fs::FileState"
handle "runtime.fs::FileLockState"
handle "runtime.fs::TemporaryDirectoryState"
handle "runtime.fs::FileWatchState"
handle "runtime.input_state::Utf8InputState"
handle "runtime.process::ProcessState"
handle "runtime.process::CapturedProcess"
handle "runtime.process::PipelineState"
handle "runtime.process::CapturedPipeline"
function f0 "main" {
    registers [t1, t0, t2, t3, t0, t10, t4, t11, t6, t12, t13, t13, t14]
    params []
    result t13
    0: jmp @1
    1: jmp @2
    2: const r0, str "/bin/sleep"
    3: call r1, f7, [r0]
    4: const r0, str "60"
    5: arr.new r2, [r0]
    6: call r3, f8, []
    7: const r0, str "."
    8: call r4, f7, [r0]
    9: call r5, f10, [r4]
    10: role.wrap r6, r5, role2
    11: call r7, f11, [r1, r2, r3, r6]
    12: role.wrap r8, r7, role0
    13: call r9, f12, [r8]
    14: call r10, f13, [r9]
    15: const r11, int 0
    16: int.cmp gt, r12, r10, r11
    17: br r12, @18, @23
    18: call _, f14, [r9]
    19: call _, f14, [r9]
    20: const r10, int 42
    21: jmp @22
    22: ret r10
    23: const r0, str "missing process ID"
    24: panic r0
}
function f1 "fs.path::Path.order" {
    registers [t0, t0, t1, t1, t14, t5]
    params [r0, r1]
    result t5
    0: jmp @1
    1: jmp @2
    2: obj.get r2, r0, 0
    3: call r3, f15, [r1]
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
function f2 "os.command::Command.program" {
    registers [t11, t0]
    params [r0]
    result t0
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: jmp @4
    4: ret r1
}
function f3 "os.command::Command.arguments" {
    registers [t11, t2]
    params [r0]
    result t2
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 1
    3: jmp @4
    4: ret r1
}
function f4 "os.command::Command.environment" {
    registers [t11, t3]
    params [r0]
    result t3
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 2
    3: jmp @4
    4: ret r1
}
function f5 "os.command::Command.directory" {
    registers [t11, t4]
    params [r0]
    result t4
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 3
    3: jmp @4
    4: ret r1
}
function f6 "os.command::Command.string" {
    registers [t11, t1, t0, t1, t1, t4]
    params [r0]
    result t1
    0: jmp @1
    1: jmp @2
    2: const r1, str "Command(program: "
    3: obj.get r2, r0, 0
    4: call r3, f15, [r2]
    5: call r4, f16, [r0, r3]
    6: str.concat r3, r1, r4
    7: const r1, str ", arguments: ["
    8: str.concat r4, r3, r1
    9: call r1, f17, [r0]
    10: str.concat r3, r4, r1
    11: const r1, str "], directory: "
    12: str.concat r4, r3, r1
    13: obj.get r5, r0, 3
    14: call.role r2, r5, role2, 0, []
    15: call r1, f15, [r2]
    16: call r3, f16, [r0, r1]
    17: str.concat r1, r4, r3
    18: const r3, str ")"
    19: str.concat r4, r1, r3
    20: jmp @21
    21: ret r4
}
function f7 "fs.path::Path.new#0" {
    registers [t1, t13, t13, t14, t14, t15, t8, t0, t16]
    params [r0]
    result t0
    0: jmp @1
    1: jmp @2
    2: str.len r1, r0
    3: const r2, int 0
    4: int.cmp eq, r3, r1, r2
    5: bool.not r4, r3
    6: br r4, @7, @13
    7: call r5, f20, []
    8: call r6, f21, [r5]
    9: jmp @10
    10: obj.new r7, obj1, [r0, r6]
    11: jmp @12
    12: ret r7
    13: const r0, str "path cannot be empty"
    14: call r8, f19, [r0]
    15: throw r8
}
function f8 "os.environment::Environment.new#0" {
    registers [t9, t3, t17, t18, t1, t19, t17, t17]
    params []
    result t3
    handler 2 3 @8 r2
    handler 3 5 @8 r2
    handler 9 10 @13 r6
    handler 10 13 @13 r6
    0: jmp @1
    1: jmp @2
    2: jmp @3
    3: call r0, f23, []
    4: jmp @5
    5: obj.new r1, obj2, [r0]
    6: jmp @7
    7: ret r1
    8: catch r2, object, 7, r3, @9, @15
    9: jmp @10
    10: call r4, f25, [r3]
    11: call r5, f26, [r4]
    12: throw r5
    13: fail.caused r7, r6, r2
    14: raise r7
    15: raise r2
}
function f9 "fs.directory::Directory.path" {
    registers [t10, t0]
    params [r0]
    result t0
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: jmp @4
    4: ret r1
}
function f10 "fs.directory::Directory.new#0" {
    registers [t0, t10]
    params [r0]
    result t10
    0: jmp @1
    1: jmp @2
    2: obj.new r1, obj4, [r0]
    3: jmp @4
    4: ret r1
}
function f11 "os.command::Command.new#0" {
    registers [t0, t2, t3, t4, t11]
    params [r0, r1, r2, r3]
    result t11
    0: jmp @1
    1: jmp @2
    2: obj.new r4, obj3, [r0, r1, r2, r3]
    3: jmp @4
    4: ret r4
}
function f12 "os.process::Process.new#0" {
    registers [t6, t7, t12]
    params [r0]
    result t12
    0: jmp @1
    1: jmp @2
    2: call r1, f27, []
    3: jmp @4
    4: obj.new r2, obj0, [r0, r1]
    5: jmp @6
    6: ret r2
}
function f13 "os.process::Process.id" {
    registers [t12, t7, t13, t17, t18, t1, t1, t20, t17, t17]
    params [r0]
    result t13
    handler 4 5 @9 r3
    handler 5 8 @9 r3
    handler 10 11 @15 r8
    handler 11 15 @15 r8
    0: jmp @1
    1: jmp @2
    2: call _, f28, [r0]
    3: jmp @4
    4: jmp @5
    5: obj.get r1, r0, 1
    6: call r2, f29, [r1]
    7: jmp @8
    8: ret r2
    9: catch r3, object, 7, r4, @10, @17
    10: jmp @11
    11: call r5, f31, [r4]
    12: call r6, f25, [r4]
    13: call r7, f32, [r5, r6]
    14: throw r7
    15: fail.caused r9, r8, r3
    16: raise r9
    17: raise r3
}
function f14 "os.process::Process.close" {
    registers [t12, t7, t14, t17, t18, t1, t1, t20, t17, t17]
    params [r0]
    result void
    handler 2 3 @7 r3
    handler 3 6 @7 r3
    handler 8 9 @13 r8
    handler 9 13 @13 r8
    0: jmp @1
    1: jmp @2
    2: jmp @3
    3: obj.get r1, r0, 1
    4: call r2, f33, [r1]
    5: jmp @6
    6: ret _
    7: catch r3, object, 7, r4, @8, @15
    8: jmp @9
    9: call r5, f31, [r4]
    10: call r6, f25, [r4]
    11: call r7, f32, [r5, r6]
    12: throw r7
    13: fail.caused r9, r8, r3
    14: raise r9
    15: raise r3
}
function f15 "fs.path::Path.string" {
    registers [t0, t1]
    params [r0]
    result t1
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: jmp @4
    4: ret r1
}
function f16 "os.command::Command.quoted" {
    registers [t11, t1, t1, t1, t1, t1]
    params [r0, r1]
    result t1
    0: jmp @1
    1: jmp @2
    2: const r2, str "\""
    3: const r3, str "\\"
    4: const r4, str "\\\\"
    5: str.replace r5, r1, r3, r4
    6: const r1, str "\""
    7: const r3, str "\\\""
    8: str.replace r4, r5, r1, r3
    9: const r1, str "\n"
    10: const r3, str "\\n"
    11: str.replace r5, r4, r1, r3
    12: const r1, str "\r"
    13: const r3, str "\\r"
    14: str.replace r4, r5, r1, r3
    15: const r1, str "\t"
    16: const r3, str "\\t"
    17: str.replace r5, r4, r1, r3
    18: str.concat r1, r2, r5
    19: const r2, str "\""
    20: str.concat r3, r1, r2
    21: jmp @22
    22: ret r3
}
function f17 "os.command::Command.rendered_arguments" {
    registers [t11, t13, t1]
    params [r0]
    result t1
    0: jmp @1
    1: jmp @2
    2: const r1, int 0
    3: call r2, f34, [r0, r1]
    4: jmp @5
    5: ret r2
}
function f18 "fs.path::InvalidPath.text" {
    registers [t16, t1]
    params [r0]
    result t1
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: jmp @4
    4: ret r1
}
function f19 "fs.path::InvalidPath.new#0" {
    registers [t1, t16]
    params [r0]
    result t16
    0: jmp @1
    1: jmp @2
    2: obj.new r1, obj5, [r0]
    3: jmp @4
    4: ret r1
}
function f20 "fs.path::NativePathStyle.new#0" {
    registers [t15]
    params []
    result t15
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj6, []
    3: jmp @4
    4: ret r0
}
function f21 "fs.path::NativePathStyle.value" {
    registers [t15, t1, t1, t14, t8]
    params [r0]
    result t8
    0: jmp @1
    1: jmp @2
    2: call r1, f35, []
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
function f22 "NativeFailure.text" {
    registers [t18, t1]
    params [r0]
    result t1
    0: obj.get r1, r0, 3
    1: ret r1
}
function f23 "runtime.environment::environment_snapshot" native "runtime/environment@1::environment_snapshot/0" {
    registers []
    params []
    result t9
}
function f24 "os.environment::InvalidEnvironmentVariable.text" {
    registers [t19, t1]
    params [r0]
    result t1
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: jmp @4
    4: ret r1
}
function f25 "NativeFailure.message" {
    registers [t18, t1]
    params [r0]
    result t1
    0: obj.get r1, r0, 3
    1: ret r1
}
function f26 "os.environment::InvalidEnvironmentVariable.new#0" {
    registers [t1, t19]
    params [r0]
    result t19
    0: jmp @1
    1: jmp @2
    2: obj.new r1, obj8, [r0]
    3: jmp @4
    4: ret r1
}
function f27 "runtime.process::process_state_create" native "runtime/process@1::process_state_create/0" {
    registers []
    params []
    result t7
}
function f28 "os.process::Process.start" {
    registers [t12, t14, t14, t7, t6, t0, t1, t2, t4, t1, t3, t9, t17, t18, t20, t17, t17]
    params [r0]
    result void
    handler 6 7 @22 r12
    handler 7 22 @22 r12
    handler 23 24 @28 r15
    handler 24 28 @28 r15
    0: jmp @1
    1: jmp @2
    2: call r1, f36, [r0]
    3: bool.not r2, r1
    4: br r2, @5, @31
    5: jmp @6
    6: jmp @7
    7: obj.get r3, r0, 1
    8: obj.get r4, r0, 0
    9: call.role r5, r4, role0, 0, []
    10: call r6, f15, [r5]
    11: obj.get r4, r0, 0
    12: call.role r7, r4, role0, 1, []
    13: obj.get r4, r0, 0
    14: call.role r8, r4, role0, 3, []
    15: call.role r5, r8, role2, 0, []
    16: call r9, f15, [r5]
    17: obj.get r4, r0, 0
    18: call.role r10, r4, role0, 2, []
    19: call r11, f38, [r10]
    20: call r1, f37, [r3, r6, r7, r9, r11]
    21: jmp @31
    22: catch r12, object, 7, r13, @23, @30
    23: jmp @24
    24: call r6, f31, [r13]
    25: call r9, f25, [r13]
    26: call r14, f32, [r6, r9]
    27: throw r14
    28: fail.caused r16, r15, r12
    29: raise r16
    30: raise r12
    31: ret _
}
function f29 "runtime.process::process_state_id" native "runtime/process@1::process_state_id/1" {
    registers [t7]
    params [r0]
    result t13
}
function f30 "os.process::ProcessFailure.text" {
    registers [t20, t1]
    params [r0]
    result t1
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 1
    3: jmp @4
    4: ret r1
}
function f31 "NativeFailure.code" {
    registers [t18, t1]
    params [r0]
    result t1
    0: obj.get r1, r0, 1
    1: ret r1
}
function f32 "os.process::ProcessFailure.new#0" {
    registers [t1, t1, t20]
    params [r0, r1]
    result t20
    0: jmp @1
    1: jmp @2
    2: obj.new r2, obj9, [r0, r1]
    3: jmp @4
    4: ret r2
}
function f33 "runtime.process::process_state_close" native "runtime/process@1::process_state_close/1" {
    registers [t7]
    params [r0]
    result t14
}
function f34 "os.command::Command.rendered_arguments_from" {
    registers [t11, t13, t2, t13, t14, t1, t1, t13, t1, t1]
    params [r0, r1]
    result t1
    0: jmp @1
    1: jmp @2
    2: obj.get r2, r0, 1
    3: arr.len r3, r2
    4: int.cmp ge, r4, r1, r3
    5: br r4, @6, @9
    6: jmp @7
    7: const r5, str ""
    8: jmp @31
    9: jmp @10
    10: obj.get r2, r0, 1
    11: arr.get r5, r2, r1
    12: call r6, f16, [r0, r5]
    13: const r3, int 1
    14: int.add r7, r1, r3
    15: obj.get r2, r0, 1
    16: arr.len r3, r2
    17: int.cmp lt, r4, r7, r3
    18: br r4, @19, @26
    19: jmp @20
    20: const r5, str ", "
    21: const r3, int 1
    22: int.add r7, r1, r3
    23: call r8, f34, [r0, r7]
    24: str.concat r9, r5, r8
    25: jmp @29
    26: jmp @27
    27: const r5, str ""
    28: jmp @29(r9 <- r5)
    29: str.concat r5, r6, r9
    30: jmp @31
    31: jmp @32
    32: ret r5
}
function f35 "runtime.system::system_platform" native "runtime/system@1::system_platform/0" {
    registers []
    params []
    result t1
}
function f36 "os.process::Process.started" {
    registers [t12, t7, t14, t17, t18, t1, t1, t20, t17, t17]
    params [r0]
    result t14
    handler 2 3 @7 r3
    handler 3 6 @7 r3
    handler 8 9 @13 r8
    handler 9 13 @13 r8
    0: jmp @1
    1: jmp @2
    2: jmp @3
    3: obj.get r1, r0, 1
    4: call r2, f39, [r1]
    5: jmp @6
    6: ret r2
    7: catch r3, object, 7, r4, @8, @15
    8: jmp @9
    9: call r5, f31, [r4]
    10: call r6, f25, [r4]
    11: call r7, f32, [r5, r6]
    12: throw r7
    13: fail.caused r9, r8, r3
    14: raise r9
    15: raise r3
}
function f37 "runtime.process::process_state_start" native "runtime/process@1::process_state_start/5" {
    registers [t7, t1, t2, t1, t9]
    params [r0, r1, r2, r3, r4]
    result t14
}
function f38 "os.environment::Environment.variables" {
    registers [t3, t9]
    params [r0]
    result t9
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: jmp @4
    4: ret r1
}
function f39 "runtime.process::process_state_started" native "runtime/process@1::process_state_started/1" {
    registers [t7]
    params [r0]
    result t14
}
entry f0
