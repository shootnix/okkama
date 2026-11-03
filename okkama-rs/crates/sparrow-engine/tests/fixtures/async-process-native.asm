sparrow 9
module "main" "v1.0.0"
type t0 object obj2
type t1 enum enum0
type t2 str
type t3 array t2
type t4 object obj0
type t5 role role1
type t6 map t2 t2
type t7 enum enum1
type t8 role role2
type t9 byte
type t10 array t9
type t11 int
type t12 handle 7
type t13 object obj6
type t14 enum enum8
type t15 array t8
type t16 handle 9
type t17 array t13
type t18 array t10
type t19 enum enum9
type t20 enum enum10
type t21 object obj1
type t22 object obj3
type t23 object obj4
type t24 object obj5
type t25 future t24
type t26 bool
type t27 object obj7
type t28 object obj8
type t29 object obj9
type t30 future t29
type t31 failure
type t32 object obj10
type t33 object obj11
type t34 object obj13
type t35 object obj12
type t36 handle 8
type t37 future t36
type t38 object obj15
type t39 map t2 t11
type t40 object obj16
type t41 object obj14
type t42 object obj17
type t43 handle 10
type t44 future t43
type t45 object obj20
type t46 array t39
type t47 builder t13
type t48 object obj19
type t49 object obj18
enum enum0 "compare::Order" ["less", "equal", "greater"]
enum enum1 "fs.path::PathStyle" ["posix", "windows"]
enum enum2 "time.duration::DurationUnit" ["nanosecond", "microsecond", "millisecond", "second", "minute", "hour", "day"]
enum enum3 "fs.stat::FileKind" ["other", "file", "directory", "symbolic_link", "block_device", "character_device", "fifo", "socket"]
enum enum4 "encoding.utf::ByteOrder" ["big", "little"]
enum enum5 "fs.file::FileMode" ["reading", "writing", "appending", "reading_and_writing", "reading_and_appending"]
enum enum6 "fs.file::SeekOrigin" ["start", "current", "end"]
enum enum7 "os.process::Signal" ["interrupt", "terminate", "kill"]
enum enum8 "os.process::ExitKind" ["exited", "interrupted", "terminated", "killed", "other"]
enum enum9 "os.command_capture::CapturedStream" ["output", "errors"]
enum enum10 "os.pipeline_capture::PipelineStream" ["output", "errors"]
role role0 "compare::Comparing<Path>" {
    method "order" [t0] -> t1
}
role role1 "fs.directory::DirectoryLocating" {
    method "path" [] -> t0
}
role role2 "os.command::Commanding" {
    method "program" [] -> t0
    method "arguments" [] -> t3
    method "environment" [] -> t4
    method "directory" [] -> t5
    method "string" [] -> t2
}
role role3 "Error" builtin {
    method "text" [] -> t2
}
object obj0 "os.environment::Environment" {
    field "origin" t6
}
object obj1 "fs.directory::Directory" {
    field "origin" t0
    does role1 [f3]
}
object obj2 "fs.path::Path" {
    field "origin" t2
    field "path_style" t7
    does role0 [f2]
}
object obj3 "os.command::Command" {
    field "command_program" t0
    field "command_arguments" t3
    field "command_environment" t4
    field "command_directory" t5
    does role2 [f6, f7, f8, f9, f10]
}
object obj4 "os.command_capture::CommandCapture" {
    field "command" t8
    field "input" t10
    field "limit" t11
    field "state" t12
}
object obj5 "os.command_capture::CompletedCommand" {
    field "completed_exit" t13
    field "captured_output" t10
    field "captured_errors" t10
}
object obj6 "os.process::Exit" {
    field "classification" t14
    field "exit_code" t11
}
object obj7 "encoding.utf::Utf8Decoded" {
    field "origin" t10
}
object obj8 "os.pipeline_capture::PipelineCapture" {
    field "commands" t15
    field "input" t10
    field "limit" t11
    field "state" t16
}
object obj9 "os.pipeline_capture::CompletedPipeline" {
    field "completed_exits" t17
    field "captured_output" t10
    field "captured_errors" t18
}
object obj10 "NativeFailure" builtin {
    field "domain" t2
    field "code" t2
    field "operation" t2
    field "message" t2
    does role3 [f24]
}
object obj11 "os.environment::InvalidEnvironmentVariable" {
    field "message" t2
    does role3 [f26]
}
object obj12 "fs.path::InvalidPath" {
    field "message" t2
    does role3 [f30]
}
object obj13 "fs.path::NativePathStyle" {
}
object obj14 "os.command_capture::InvalidCaptureLimit" {
    field "origin" t11
    does role3 [f37]
}
object obj15 "os.command_capture::CaptureLimitExceeded" {
    field "captured_stream" t19
    field "capture_limit" t11
    does role3 [f43]
}
object obj16 "os.process::ProcessFailure" {
    field "failure_code" t2
    field "message" t2
    does role3 [f51]
}
object obj17 "encoding.utf::InvalidUtf8" {
    field "invalid_offset" t11
    does role3 [f55]
}
object obj18 "os.pipeline_capture::EmptyPipeline" {
    does role3 [f59]
}
object obj19 "os.pipeline_capture::InvalidPipelineCaptureLimit" {
    field "origin" t11
    does role3 [f61]
}
object obj20 "os.pipeline_capture::PipelineCaptureLimitExceeded" {
    field "origin_index" t11
    field "origin_stream" t20
    field "origin_limit" t11
    does role3 [f66]
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
function f0 "main" async {
    registers [t4, t2, t0, t21, t2, t2, t3, t5, t22, t11, t8, t23, t25, t24, t13, t26, t10, t27, t22, t8, t15, t28, t30, t29, t17, t11, t18]
    params []
    result t11
    0: jmp @1
    1: jmp @2
    2: call r0, f1, []
    3: const r1, str "."
    4: call r2, f4, [r1]
    5: call r3, f5, [r2]
    6: const r1, str "printf"
    7: call r2, f4, [r1]
    8: const r1, str "%s|%s"
    9: const r4, str "a b"
    10: const r5, str "c;d"
    11: arr.new r6, [r1, r4, r5]
    12: role.wrap r7, r3, role1
    13: call r8, f11, [r2, r6, r0, r7]
    14: const r9, int 4096
    15: role.wrap r10, r8, role2
    16: call r11, f12, [r10, r9]
    17: call r12, f13, [r11]
    18: await r13, r12
    19: call r14, f14, [r13]
    20: call r15, f15, [r14]
    21: br r15, @22, @84
    22: call r16, f16, [r13]
    23: call r17, f17, [r16]
    24: call r1, f18, [r17]
    25: const r4, str "a b|c;d"
    26: str.cmp eq, r15, r1, r4
    27: br r15, @28, @82
    28: const r1, str "tr"
    29: call r2, f4, [r1]
    30: const r1, str "a-z"
    31: const r4, str "A-Z"
    32: arr.new r6, [r1, r4]
    33: role.wrap r7, r3, role1
    34: call r18, f11, [r2, r6, r0, r7]
    35: role.wrap r10, r8, role2
    36: role.wrap r19, r18, role2
    37: arr.new r20, [r10, r19]
    38: const r9, int 4096
    39: call r21, f19, [r20, r9]
    40: call r22, f20, [r21]
    41: await r23, r22
    42: call r16, f21, [r23]
    43: call r17, f17, [r16]
    44: call r1, f18, [r17]
    45: const r4, str "A B|C;D"
    46: str.cmp eq, r15, r1, r4
    47: br r15, @48, @80
    48: call r24, f22, [r23]
    49: arr.len r9, r24
    50: const r25, int 2
    51: int.cmp eq, r15, r9, r25
    52: br r15, @53, @78
    53: call r26, f23, [r23]
    54: arr.len r9, r26
    55: const r25, int 2
    56: int.cmp eq, r15, r9, r25
    57: br r15, @58, @76
    58: call r24, f22, [r23]
    59: const r9, int 0
    60: arr.get r14, r24, r9
    61: call r15, f15, [r14]
    62: br r15, @63, @68
    63: call r24, f22, [r23]
    64: const r9, int 1
    65: arr.get r14, r24, r9
    66: call r15, f15, [r14]
    67: jmp @70
    68: const r15, bool false
    69: jmp @70
    70: br r15, @71, @74
    71: const r9, int 42
    72: jmp @73
    73: ret r9
    74: const r1, str "pipeline failed"
    75: panic r1
    76: const r1, str "pipeline stderr count"
    77: panic r1
    78: const r1, str "pipeline status count"
    79: panic r1
    80: const r1, str "pipeline changed bytes"
    81: panic r1
    82: const r1, str "arguments changed"
    83: panic r1
    84: const r1, str "command failed"
    85: panic r1
}
function f1 "os.environment::Environment.new#0" {
    registers [t6, t4, t31, t32, t2, t33, t31, t31]
    params []
    result t4
    handler 2 3 @8 r2
    handler 3 5 @8 r2
    handler 9 10 @13 r6
    handler 10 13 @13 r6
    0: jmp @1
    1: jmp @2
    2: jmp @3
    3: call r0, f25, []
    4: jmp @5
    5: obj.new r1, obj0, [r0]
    6: jmp @7
    7: ret r1
    8: catch r2, object, 10, r3, @9, @15
    9: jmp @10
    10: call r4, f27, [r3]
    11: call r5, f28, [r4]
    12: throw r5
    13: fail.caused r7, r6, r2
    14: raise r7
    15: raise r2
}
function f2 "fs.path::Path.order" {
    registers [t0, t0, t2, t2, t26, t1]
    params [r0, r1]
    result t1
    0: jmp @1
    1: jmp @2
    2: obj.get r2, r0, 0
    3: call r3, f29, [r1]
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
function f3 "fs.directory::Directory.path" {
    registers [t21, t0]
    params [r0]
    result t0
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: jmp @4
    4: ret r1
}
function f4 "fs.path::Path.new#0" {
    registers [t2, t11, t11, t26, t26, t34, t7, t0, t35]
    params [r0]
    result t0
    0: jmp @1
    1: jmp @2
    2: str.len r1, r0
    3: const r2, int 0
    4: int.cmp eq, r3, r1, r2
    5: bool.not r4, r3
    6: br r4, @7, @13
    7: call r5, f32, []
    8: call r6, f33, [r5]
    9: jmp @10
    10: obj.new r7, obj2, [r0, r6]
    11: jmp @12
    12: ret r7
    13: const r0, str "path cannot be empty"
    14: call r8, f31, [r0]
    15: throw r8
}
function f5 "fs.directory::Directory.new#0" {
    registers [t0, t21]
    params [r0]
    result t21
    0: jmp @1
    1: jmp @2
    2: obj.new r1, obj1, [r0]
    3: jmp @4
    4: ret r1
}
function f6 "os.command::Command.program" {
    registers [t22, t0]
    params [r0]
    result t0
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: jmp @4
    4: ret r1
}
function f7 "os.command::Command.arguments" {
    registers [t22, t3]
    params [r0]
    result t3
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 1
    3: jmp @4
    4: ret r1
}
function f8 "os.command::Command.environment" {
    registers [t22, t4]
    params [r0]
    result t4
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 2
    3: jmp @4
    4: ret r1
}
function f9 "os.command::Command.directory" {
    registers [t22, t5]
    params [r0]
    result t5
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 3
    3: jmp @4
    4: ret r1
}
function f10 "os.command::Command.string" {
    registers [t22, t2, t0, t2, t2, t5]
    params [r0]
    result t2
    0: jmp @1
    1: jmp @2
    2: const r1, str "Command(program: "
    3: obj.get r2, r0, 0
    4: call r3, f29, [r2]
    5: call r4, f34, [r0, r3]
    6: str.concat r3, r1, r4
    7: const r1, str ", arguments: ["
    8: str.concat r4, r3, r1
    9: call r1, f35, [r0]
    10: str.concat r3, r4, r1
    11: const r1, str "], directory: "
    12: str.concat r4, r3, r1
    13: obj.get r5, r0, 3
    14: call.role r2, r5, role1, 0, []
    15: call r1, f29, [r2]
    16: call r3, f34, [r0, r1]
    17: str.concat r1, r4, r3
    18: const r3, str ")"
    19: str.concat r4, r1, r3
    20: jmp @21
    21: ret r4
}
function f11 "os.command::Command.new#0" {
    registers [t0, t3, t4, t5, t22]
    params [r0, r1, r2, r3]
    result t22
    0: jmp @1
    1: jmp @2
    2: obj.new r4, obj3, [r0, r1, r2, r3]
    3: jmp @4
    4: ret r4
}
function f12 "os.command_capture::CommandCapture.new#0" {
    registers [t8, t11, t10, t12, t23]
    params [r0, r1]
    result t23
    0: jmp @1
    1: jmp @2
    2: arr.new r2, []
    3: call r3, f36, []
    4: jmp @5
    5: obj.new r4, obj4, [r0, r2, r1, r3]
    6: jmp @7
    7: ret r4
}
function f13 "os.command_capture::CommandCapture.completion" async {
    registers [t23, t11, t11, t26, t12, t8, t0, t2, t3, t5, t2, t4, t6, t10, t37, t36, t19, t38, t39, t14, t13, t10, t24, t31, t32, t40, t31, t31, t41]
    params [r0]
    result t24
    handler 6 7 @53 r23
    handler 7 30 @53 r23
    handler 30 31 @53 r23
    handler 31 35 @53 r23
    handler 35 38 @53 r23
    handler 38 39 @53 r23
    handler 39 43 @53 r23
    handler 43 52 @53 r23
    handler 54 55 @59 r26
    handler 55 59 @59 r26
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 2
    3: const r2, int 0
    4: int.cmp gt, r3, r1, r2
    5: br r3, @6, @62
    6: jmp @7
    7: obj.get r4, r0, 3
    8: obj.get r5, r0, 0
    9: call.role r6, r5, role2, 0, []
    10: call r7, f29, [r6]
    11: obj.get r5, r0, 0
    12: call.role r8, r5, role2, 1, []
    13: obj.get r5, r0, 0
    14: call.role r9, r5, role2, 3, []
    15: call.role r6, r9, role1, 0, []
    16: call r10, f29, [r6]
    17: obj.get r5, r0, 0
    18: call.role r11, r5, role2, 2, []
    19: call r12, f40, [r11]
    20: call r3, f39, [r4, r7, r8, r10, r12]
    21: obj.get r4, r0, 3
    22: obj.get r13, r0, 1
    23: obj.get r1, r0, 2
    24: call r14, f41, [r4, r13, r1]
    25: await r15, r14
    26: call r1, f42, [r15]
    27: const r2, int 1
    28: int.cmp eq, r3, r1, r2
    29: br r3, @30, @35
    30: jmp @31
    31: enum.case r16, enum9, 0
    32: obj.get r1, r0, 2
    33: call r17, f44, [r16, r1]
    34: throw r17
    35: const r2, int 2
    36: int.cmp eq, r3, r1, r2
    37: br r3, @38, @43
    38: jmp @39
    39: enum.case r16, enum9, 1
    40: obj.get r1, r0, 2
    41: call r17, f44, [r16, r1]
    42: throw r17
    43: call r18, f45, [r15]
    44: call r19, f46, [r0, r18]
    45: const r7, str "code"
    46: map.get r1, r18, r7
    47: call r20, f47, [r19, r1]
    48: call r13, f48, [r15]
    49: call r21, f49, [r15]
    50: call r22, f50, [r20, r13, r21]
    51: jmp @52
    52: ret r22
    53: catch r23, object, 10, r24, @54, @61
    54: jmp @55
    55: call r7, f52, [r24]
    56: call r10, f27, [r24]
    57: call r25, f53, [r7, r10]
    58: throw r25
    59: fail.caused r27, r26, r23
    60: raise r27
    61: raise r23
    62: obj.get r1, r0, 2
    63: call r28, f38, [r1]
    64: throw r28
}
function f14 "os.command_capture::CompletedCommand.exit" {
    registers [t24, t13]
    params [r0]
    result t13
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: jmp @4
    4: ret r1
}
function f15 "os.process::Exit.success" {
    registers [t13, t14, t14, t26, t11, t11]
    params [r0]
    result t26
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: enum.case r2, enum8, 0
    4: enum.cmp eq, r3, r1, r2
    5: br r3, @6, @10
    6: obj.get r4, r0, 1
    7: const r5, int 0
    8: int.cmp eq, r3, r4, r5
    9: jmp @12
    10: const r3, bool false
    11: jmp @12
    12: jmp @13
    13: ret r3
}
function f16 "os.command_capture::CompletedCommand.output" {
    registers [t24, t10]
    params [r0]
    result t10
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 1
    3: jmp @4
    4: ret r1
}
function f17 "encoding.utf::Utf8Decoded.new#0" {
    registers [t10, t27]
    params [r0]
    result t27
    0: jmp @1
    1: jmp @2
    2: obj.new r1, obj7, [r0]
    3: jmp @4
    4: ret r1
}
function f18 "encoding.utf::Utf8Decoded.string" {
    registers [t27, t10, t11, t11, t26, t2, t42]
    params [r0]
    result t2
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: call r2, f54, [r1]
    4: obj.get r1, r0, 0
    5: arr.len r3, r1
    6: int.cmp eq, r4, r2, r3
    7: br r4, @8, @12
    8: obj.get r1, r0, 0
    9: call r5, f57, [r1]
    10: jmp @11
    11: ret r5
    12: call r6, f56, [r2]
    13: throw r6
}
function f19 "os.pipeline_capture::PipelineCapture.new#0" {
    registers [t15, t11, t10, t16, t28]
    params [r0, r1]
    result t28
    0: jmp @1
    1: jmp @2
    2: arr.new r2, []
    3: call r3, f58, []
    4: jmp @5
    5: obj.new r4, obj8, [r0, r2, r1, r3]
    6: jmp @7
    7: ret r4
}
function f20 "os.pipeline_capture::PipelineCapture.completion" async {
    registers [t28, t15, t11, t11, t26, t8, t16, t0, t2, t3, t5, t2, t4, t6, t11, t11, t10, t44, t43, t20, t45, t46, t47, t39, t14, t2, t13, t47, t11, t17, t18, t29, t31, t32, t40, t31, t31, t48, t49]
    params [r0]
    result t29
    handler 14 15 @82 r32
    handler 15 19 @82 r32
    handler 19 21 @82 r32
    handler 21 23 @82 r32
    handler 23 34 @82 r32
    handler 34 36 @82 r32
    handler 36 45 @82 r32
    handler 45 46 @82 r32
    handler 46 50 @82 r32
    handler 50 51 @82 r32
    handler 51 53 @82 r32
    handler 53 54 @82 r32
    handler 54 56 @82 r32
    handler 56 59 @82 r32
    handler 59 64 @82 r32
    handler 64 66 @82 r32
    handler 66 68 @82 r32
    handler 68 72 @82 r32
    handler 72 74 @82 r32
    handler 74 76 @82 r32
    handler 76 81 @82 r32
    handler 83 84 @88 r35
    handler 84 88 @88 r35
    0: const r14, int 1
    1: const r25, str "code"
    2: const r28, int 1
    3: jmp @4
    4: jmp @5
    5: obj.get r1, r0, 0
    6: arr.len r2, r1
    7: const r3, int 0
    8: int.cmp gt, r4, r2, r3
    9: br r4, @10, @94
    10: obj.get r2, r0, 2
    11: const r3, int 0
    12: int.cmp gt, r4, r2, r3
    13: br r4, @14, @91
    14: jmp @15
    15: obj.get r1, r0, 0
    16: const r2, int 0
    17: arr.len r3, r1
    18: jmp @19
    19: int.cmp lt, r4, r2, r3
    20: br r4, @21, @36
    21: arr.get r5, r1, r2
    22: jmp @23
    23: obj.get r6, r0, 3
    24: call.role r7, r5, role2, 0, []
    25: call r8, f29, [r7]
    26: call.role r9, r5, role2, 1, []
    27: call.role r10, r5, role2, 3, []
    28: call.role r7, r10, role1, 0, []
    29: call r11, f29, [r7]
    30: call.role r12, r5, role2, 2, []
    31: call r13, f40, [r12]
    32: call r4, f63, [r6, r8, r9, r11, r13]
    33: jmp @34
    34: int.add r15, r2, r14
    35: jmp @19(r2 <- r15)
    36: obj.get r6, r0, 3
    37: obj.get r16, r0, 1
    38: obj.get r2, r0, 2
    39: call r17, f64, [r6, r16, r2]
    40: await r18, r17
    41: call r2, f65, [r18]
    42: const r3, int 0
    43: int.cmp gt, r4, r2, r3
    44: br r4, @45, @59
    45: jmp @46
    46: call r3, f67, [r18]
    47: const r14, int 1
    48: int.cmp eq, r4, r2, r14
    49: br r4, @50, @53
    50: jmp @51
    51: enum.case r19, enum10, 0
    52: jmp @56
    53: jmp @54
    54: enum.case r19, enum10, 1
    55: jmp @56
    56: obj.get r2, r0, 2
    57: call r20, f68, [r3, r19, r2]
    58: throw r20
    59: call r21, f69, [r18]
    60: builder.new r22
    61: const r2, int 0
    62: arr.len r3, r21
    63: jmp @64
    64: int.cmp lt, r4, r2, r3
    65: br r4, @66, @76
    66: arr.get r23, r21, r2
    67: jmp @68
    68: call r24, f70, [r0, r23]
    69: map.get r14, r23, r25
    70: call r26, f47, [r24, r14]
    71: jmp @72
    72: builder.push r27, r22, r26
    73: jmp @74
    74: int.add r14, r2, r28
    75: jmp @64(r2 <- r14, r22 <- r27)
    76: builder.finish r29, r22
    77: call r16, f71, [r18]
    78: call r30, f72, [r18]
    79: call r31, f73, [r29, r16, r30]
    80: jmp @81
    81: ret r31
    82: catch r32, object, 10, r33, @83, @90
    83: jmp @84
    84: call r8, f52, [r33]
    85: call r11, f27, [r33]
    86: call r34, f53, [r8, r11]
    87: throw r34
    88: fail.caused r36, r35, r32
    89: raise r36
    90: raise r32
    91: obj.get r2, r0, 2
    92: call r37, f62, [r2]
    93: throw r37
    94: call r38, f60, []
    95: throw r38
}
function f21 "os.pipeline_capture::CompletedPipeline.output" {
    registers [t29, t10]
    params [r0]
    result t10
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 1
    3: jmp @4
    4: ret r1
}
function f22 "os.pipeline_capture::CompletedPipeline.exits" {
    registers [t29, t17]
    params [r0]
    result t17
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: jmp @4
    4: ret r1
}
function f23 "os.pipeline_capture::CompletedPipeline.errors" {
    registers [t29, t18]
    params [r0]
    result t18
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 2
    3: jmp @4
    4: ret r1
}
function f24 "NativeFailure.text" {
    registers [t32, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 3
    1: ret r1
}
function f25 "runtime.environment::environment_snapshot" native "runtime/environment@1::environment_snapshot/0" {
    registers []
    params []
    result t6
}
function f26 "os.environment::InvalidEnvironmentVariable.text" {
    registers [t33, t2]
    params [r0]
    result t2
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: jmp @4
    4: ret r1
}
function f27 "NativeFailure.message" {
    registers [t32, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 3
    1: ret r1
}
function f28 "os.environment::InvalidEnvironmentVariable.new#0" {
    registers [t2, t33]
    params [r0]
    result t33
    0: jmp @1
    1: jmp @2
    2: obj.new r1, obj11, [r0]
    3: jmp @4
    4: ret r1
}
function f29 "fs.path::Path.string" {
    registers [t0, t2]
    params [r0]
    result t2
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: jmp @4
    4: ret r1
}
function f30 "fs.path::InvalidPath.text" {
    registers [t35, t2]
    params [r0]
    result t2
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: jmp @4
    4: ret r1
}
function f31 "fs.path::InvalidPath.new#0" {
    registers [t2, t35]
    params [r0]
    result t35
    0: jmp @1
    1: jmp @2
    2: obj.new r1, obj12, [r0]
    3: jmp @4
    4: ret r1
}
function f32 "fs.path::NativePathStyle.new#0" {
    registers [t34]
    params []
    result t34
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj13, []
    3: jmp @4
    4: ret r0
}
function f33 "fs.path::NativePathStyle.value" {
    registers [t34, t2, t2, t26, t7]
    params [r0]
    result t7
    0: jmp @1
    1: jmp @2
    2: call r1, f74, []
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
function f34 "os.command::Command.quoted" {
    registers [t22, t2, t2, t2, t2, t2]
    params [r0, r1]
    result t2
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
function f35 "os.command::Command.rendered_arguments" {
    registers [t22, t11, t2]
    params [r0]
    result t2
    0: jmp @1
    1: jmp @2
    2: const r1, int 0
    3: call r2, f75, [r0, r1]
    4: jmp @5
    5: ret r2
}
function f36 "runtime.process::process_state_create" native "runtime/process@1::process_state_create/0" {
    registers []
    params []
    result t12
}
function f37 "os.command_capture::InvalidCaptureLimit.text" {
    registers [t41, t2, t11, t2, t2]
    params [r0]
    result t2
    0: jmp @1
    1: jmp @2
    2: const r1, str "capture limit must be positive, got "
    3: obj.get r2, r0, 0
    4: cvt r3, r2
    5: str.concat r4, r1, r3
    6: jmp @7
    7: ret r4
}
function f38 "os.command_capture::InvalidCaptureLimit.new#0" {
    registers [t11, t41]
    params [r0]
    result t41
    0: jmp @1
    1: jmp @2
    2: obj.new r1, obj14, [r0]
    3: jmp @4
    4: ret r1
}
function f39 "runtime.process::process_state_start" native "runtime/process@1::process_state_start/5" {
    registers [t12, t2, t3, t2, t6]
    params [r0, r1, r2, r3, r4]
    result t26
}
function f40 "os.environment::Environment.variables" {
    registers [t4, t6]
    params [r0]
    result t6
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 0
    3: jmp @4
    4: ret r1
}
function f41 "runtime.process::process_state_capture" async native "runtime/process@1::process_state_capture/3" {
    registers [t12, t10, t11]
    params [r0, r1, r2]
    result t36
}
function f42 "runtime.process::captured_process_overflow" native "runtime/process@1::captured_process_overflow/1" {
    registers [t36]
    params [r0]
    result t11
}
function f43 "os.command_capture::CaptureLimitExceeded.text" {
    registers [t38, t2]
    params [r0]
    result t2
    0: jmp @1
    1: jmp @2
    2: const r1, str "command capture limit exceeded"
    3: jmp @4
    4: ret r1
}
function f44 "os.command_capture::CaptureLimitExceeded.new#0" {
    registers [t19, t11, t38]
    params [r0, r1]
    result t38
    0: jmp @1
    1: jmp @2
    2: obj.new r2, obj15, [r0, r1]
    3: jmp @4
    4: ret r2
}
function f45 "runtime.process::captured_process_status" native "runtime/process@1::captured_process_status/1" {
    registers [t36]
    params [r0]
    result t39
}
function f46 "os.command_capture::CommandCapture.exit_kind" {
    registers [t23, t39, t2, t11, t11, t26, t14]
    params [r0, r1]
    result t14
    0: jmp @1
    1: jmp @2
    2: const r2, str "state"
    3: map.get r3, r1, r2
    4: const r4, int 1
    5: int.cmp eq, r5, r3, r4
    6: br r5, @7, @10
    7: jmp @8
    8: enum.case r6, enum8, 0
    9: jmp @35
    10: jmp @11
    11: const r2, str "code"
    12: map.get r3, r1, r2
    13: const r4, int 1
    14: int.cmp eq, r5, r3, r4
    15: br r5, @16, @19
    16: jmp @17
    17: enum.case r6, enum8, 2
    18: jmp @34
    19: const r4, int 2
    20: int.cmp eq, r5, r3, r4
    21: br r5, @22, @25
    22: jmp @23
    23: enum.case r6, enum8, 3
    24: jmp @34
    25: const r4, int 3
    26: int.cmp eq, r5, r3, r4
    27: br r5, @28, @31
    28: jmp @29
    29: enum.case r6, enum8, 1
    30: jmp @34
    31: jmp @32
    32: enum.case r6, enum8, 4
    33: jmp @34
    34: jmp @35
    35: jmp @36
    36: ret r6
}
function f47 "os.process::Exit.new#0" {
    registers [t14, t11, t13]
    params [r0, r1]
    result t13
    0: jmp @1
    1: jmp @2
    2: obj.new r2, obj6, [r0, r1]
    3: jmp @4
    4: ret r2
}
function f48 "runtime.process::captured_process_output" native "runtime/process@1::captured_process_output/1" {
    registers [t36]
    params [r0]
    result t10
}
function f49 "runtime.process::captured_process_errors" native "runtime/process@1::captured_process_errors/1" {
    registers [t36]
    params [r0]
    result t10
}
function f50 "os.command_capture::CompletedCommand.new#0" {
    registers [t13, t10, t10, t24]
    params [r0, r1, r2]
    result t24
    0: jmp @1
    1: jmp @2
    2: obj.new r3, obj5, [r0, r1, r2]
    3: jmp @4
    4: ret r3
}
function f51 "os.process::ProcessFailure.text" {
    registers [t40, t2]
    params [r0]
    result t2
    0: jmp @1
    1: jmp @2
    2: obj.get r1, r0, 1
    3: jmp @4
    4: ret r1
}
function f52 "NativeFailure.code" {
    registers [t32, t2]
    params [r0]
    result t2
    0: obj.get r1, r0, 1
    1: ret r1
}
function f53 "os.process::ProcessFailure.new#0" {
    registers [t2, t2, t40]
    params [r0, r1]
    result t40
    0: jmp @1
    1: jmp @2
    2: obj.new r2, obj16, [r0, r1]
    3: jmp @4
    4: ret r2
}
function f54 "runtime.utf::utf8_valid_prefix" native "runtime/utf@1::utf8_valid_prefix/1" {
    registers [t10]
    params [r0]
    result t11
}
function f55 "encoding.utf::InvalidUtf8.text" {
    registers [t42, t2, t11, t2, t2]
    params [r0]
    result t2
    0: jmp @1
    1: jmp @2
    2: const r1, str "invalid UTF-8 sequence at byte offset "
    3: obj.get r2, r0, 0
    4: cvt r3, r2
    5: str.concat r4, r1, r3
    6: jmp @7
    7: ret r4
}
function f56 "encoding.utf::InvalidUtf8.new#0" {
    registers [t11, t42]
    params [r0]
    result t42
    0: jmp @1
    1: jmp @2
    2: obj.new r1, obj17, [r0]
    3: jmp @4
    4: ret r1
}
function f57 "runtime.utf::utf8_decode" native "runtime/utf@1::utf8_decode/1" {
    registers [t10]
    params [r0]
    result t2
}
function f58 "runtime.process::pipeline_state_create" native "runtime/process@1::pipeline_state_create/0" {
    registers []
    params []
    result t16
}
function f59 "os.pipeline_capture::EmptyPipeline.text" {
    registers [t49, t2]
    params [r0]
    result t2
    0: jmp @1
    1: jmp @2
    2: const r1, str "pipeline must contain at least one command"
    3: jmp @4
    4: ret r1
}
function f60 "os.pipeline_capture::EmptyPipeline.new#0" {
    registers [t49]
    params []
    result t49
    0: jmp @1
    1: jmp @2
    2: obj.new r0, obj18, []
    3: jmp @4
    4: ret r0
}
function f61 "os.pipeline_capture::InvalidPipelineCaptureLimit.text" {
    registers [t48, t2, t11, t2, t2]
    params [r0]
    result t2
    0: jmp @1
    1: jmp @2
    2: const r1, str "pipeline capture limit must be positive, got "
    3: obj.get r2, r0, 0
    4: cvt r3, r2
    5: str.concat r4, r1, r3
    6: jmp @7
    7: ret r4
}
function f62 "os.pipeline_capture::InvalidPipelineCaptureLimit.new#0" {
    registers [t11, t48]
    params [r0]
    result t48
    0: jmp @1
    1: jmp @2
    2: obj.new r1, obj19, [r0]
    3: jmp @4
    4: ret r1
}
function f63 "runtime.process::pipeline_state_add" native "runtime/process@1::pipeline_state_add/5" {
    registers [t16, t2, t3, t2, t6]
    params [r0, r1, r2, r3, r4]
    result t26
}
function f64 "runtime.process::pipeline_state_capture" async native "runtime/process@1::pipeline_state_capture/3" {
    registers [t16, t10, t11]
    params [r0, r1, r2]
    result t43
}
function f65 "runtime.process::captured_pipeline_overflow_stream" native "runtime/process@1::captured_pipeline_overflow_stream/1" {
    registers [t43]
    params [r0]
    result t11
}
function f66 "os.pipeline_capture::PipelineCaptureLimitExceeded.text" {
    registers [t45, t2]
    params [r0]
    result t2
    0: jmp @1
    1: jmp @2
    2: const r1, str "pipeline capture limit exceeded"
    3: jmp @4
    4: ret r1
}
function f67 "runtime.process::captured_pipeline_overflow_index" native "runtime/process@1::captured_pipeline_overflow_index/1" {
    registers [t43]
    params [r0]
    result t11
}
function f68 "os.pipeline_capture::PipelineCaptureLimitExceeded.new#0" {
    registers [t11, t20, t11, t45]
    params [r0, r1, r2]
    result t45
    0: jmp @1
    1: jmp @2
    2: obj.new r3, obj20, [r0, r1, r2]
    3: jmp @4
    4: ret r3
}
function f69 "runtime.process::captured_pipeline_statuses" native "runtime/process@1::captured_pipeline_statuses/1" {
    registers [t43]
    params [r0]
    result t46
}
function f70 "os.pipeline_capture::PipelineCapture.exit_kind" {
    registers [t28, t39, t2, t11, t11, t26, t14]
    params [r0, r1]
    result t14
    0: jmp @1
    1: jmp @2
    2: const r2, str "state"
    3: map.get r3, r1, r2
    4: const r4, int 1
    5: int.cmp eq, r5, r3, r4
    6: br r5, @7, @10
    7: jmp @8
    8: enum.case r6, enum8, 0
    9: jmp @35
    10: jmp @11
    11: const r2, str "code"
    12: map.get r3, r1, r2
    13: const r4, int 1
    14: int.cmp eq, r5, r3, r4
    15: br r5, @16, @19
    16: jmp @17
    17: enum.case r6, enum8, 2
    18: jmp @34
    19: const r4, int 2
    20: int.cmp eq, r5, r3, r4
    21: br r5, @22, @25
    22: jmp @23
    23: enum.case r6, enum8, 3
    24: jmp @34
    25: const r4, int 3
    26: int.cmp eq, r5, r3, r4
    27: br r5, @28, @31
    28: jmp @29
    29: enum.case r6, enum8, 1
    30: jmp @34
    31: jmp @32
    32: enum.case r6, enum8, 4
    33: jmp @34
    34: jmp @35
    35: jmp @36
    36: ret r6
}
function f71 "runtime.process::captured_pipeline_output" native "runtime/process@1::captured_pipeline_output/1" {
    registers [t43]
    params [r0]
    result t10
}
function f72 "runtime.process::captured_pipeline_errors" native "runtime/process@1::captured_pipeline_errors/1" {
    registers [t43]
    params [r0]
    result t18
}
function f73 "os.pipeline_capture::CompletedPipeline.new#0" {
    registers [t17, t10, t18, t29]
    params [r0, r1, r2]
    result t29
    0: jmp @1
    1: jmp @2
    2: obj.new r3, obj9, [r0, r1, r2]
    3: jmp @4
    4: ret r3
}
function f74 "runtime.system::system_platform" native "runtime/system@1::system_platform/0" {
    registers []
    params []
    result t2
}
function f75 "os.command::Command.rendered_arguments_from" {
    registers [t22, t11, t3, t11, t26, t2, t2, t11, t2, t2]
    params [r0, r1]
    result t2
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
    12: call r6, f34, [r0, r5]
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
    23: call r8, f75, [r0, r7]
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
entry f0
