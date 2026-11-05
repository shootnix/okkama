//! Independent provider boundary; only shared ABI metadata crosses from the
//! language toolchain. No legacy Value, runtime, executor or native trampoline.
use crate::{Code, Error, FunctionPlan, Program, Type, scalar::Kind, value::*};
use okkama_runtime_abi::{Native, NativeFailure};

pub(crate) fn supported(native: Native) -> bool {
    let unix = {
        #[cfg(unix)]
        {
            crate::native_unix::handles(native)
                || crate::native_root::handles(native)
                || crate::native_process::handles(native)
        }
        #[cfg(not(unix))]
        {
            false
        }
    };
    unix || crate::native_input::handles(native)
        || crate::native_fs::handles(native)
        || crate::native_environment::handles(native)
        || crate::native_multicast::handles(native)
        || crate::native_udp::handles(native)
        || crate::native_network::handles(native)
        || crate::native_dns::handles(native)
        || crate::native_db::handles(native)
        || crate::native_sqlite::handles(native)
        || crate::native_json::handles(native)
        || crate::native_regex::handles(native)
        || crate::native_idna::handles(native)
        || crate::native_net::handles(native)
        || crate::native_binary::handles(native)
        || crate::native_secret::handles(native)
        || crate::native_encoding::handles(native)
        || crate::native_numbers::handles(native)
        || crate::native_buffer::handles(native)
        || crate::native_crypto::handles(native)
        || crate::native_text::handles(native)
        || matches!(
            native,
            Native::SystemSystemPlatform
                | Native::SystemSystemArchitecture
                | Native::SystemSystemByteOrder
                | Native::TimeSleepNanoseconds
                | Native::TimeWallClock
                | Native::TimeMonotonicClock
                | Native::TestingTestStateOpen
                | Native::TestingTestStateNext
                | Native::TestingTestStateSize
                | Native::TestingTestStateClose
                | Native::TestingTestContextActive
                | Native::TimeClockDomain
                | Native::BigintValid
                | Native::BigintCanonical
                | Native::BigintOrder
                | Native::Utf8Encode
                | Native::StdoutWrite
                | Native::StderrWrite
        )
}

pub(crate) fn lower(program: &Program, index: usize) -> Result<FunctionPlan, Error> {
    let f = &program.functions[index];
    let native = f
        .native
        .as_deref()
        .and_then(Native::resolve)
        .ok_or_else(|| Error("unknown native provider symbol".into()))?;
    if !supported(native) {
        return Err(Error(format!(
            "unsupported native provider {} in new engine",
            native.symbol()
        )));
    }
    let signature = native.signature();
    if f.is_async != signature.is_async
        || f.params.len() != signature.params.len()
        || !f
            .params
            .iter()
            .zip(&signature.params)
            .all(|(r, expected)| matches_type(program, Some(f.registers[r.0 as usize]), expected))
        || !matches_type(program, f.result, &signature.result)
    {
        return Err(Error(format!(
            "invalid native provider signature: {}",
            native.symbol()
        )));
    }
    Ok(FunctionPlan {
        symbol: f.symbol.clone(),
        is_async: f.is_async,
        cold_entry: 0,
        cached: false,
        native: Some(native),
        recurrence_loop: false,
        elided_objects: 0,
        params: f.params.iter().map(|r| r.0 as usize).collect(),
        plan: vec![],
        registers: f.registers.len(),
        compiled: Default::default(),
        jit_kinds: None,
        live: vec![],
        call_saved: vec![],
        handlers: vec![],
    })
}
fn matches_type(
    program: &Program,
    actual: Option<okkama_sparrow::TypeRef>,
    expected: &okkama_runtime_abi::Type,
) -> bool {
    use okkama_runtime_abi::Type as Abi;
    match (actual.and_then(|t| program.ty(t)), expected) {
        (None, Abi::Void)
        | (Some(Type::Str), Abi::Str)
        | (Some(Type::Bool), Abi::Bool)
        | (Some(Type::Int), Abi::Int)
        | (Some(Type::Byte), Abi::Byte)
        | (Some(Type::Float), Abi::Float) => true,
        (Some(Type::Handle(handle)), Abi::Handle(expected)) => {
            let Some(module) = handle_module(expected) else {
                return false;
            };
            program
                .handles
                .get(*handle as usize)
                .is_some_and(|name| nominal_name_matches(&program.module, name, module, expected))
        }
        (Some(Type::Map(key, value)), Abi::Map(expected_key, expected_value)) => {
            matches_type(program, Some(*key), expected_key)
                && matches_type(program, Some(*value), expected_value)
        }
        (Some(Type::Array(element)), Abi::Array(expected)) => {
            matches_type(program, Some(*element), expected)
        }
        _ => false,
    }
}
/// The runtime module that declares each native handle type.
fn handle_module(name: &str) -> Option<&'static str> {
    Some(match name {
        "FileHandle"
        | "DirectoryHandle"
        | "FileState"
        | "FileLockState"
        | "TemporaryDirectoryState"
        | "FileWatchState" => "runtime/fs",
        "FileRootState" | "RootedEntryState" => "runtime/fs_root",
        "SnapshotRootState" => "runtime/snapshot",
        "ProcessState" | "PipelineState" | "CapturedProcess" | "CapturedPipeline" => {
            "runtime/process"
        }
        "Utf8InputState" => "runtime/input_state",
        "TestState" => "runtime/testing",
        "SocketState" | "TcpListenerState" | "ListenerState" | "PairState" | "MembershipState" => {
            "runtime/network"
        }
        "DbCellState" | "DbSchemaState" | "DbCaptureState" => "runtime/db",
        "SqliteState" | "SqliteReplyState" => "runtime/database",
        "DigestState" => "runtime/host_crypto",
        "BufferState" => "runtime/buffer",
        "SecretState" => "runtime/secret",
        "RegexState"
        | "RegexQueryState"
        | "RegexMatchState"
        | "RegexReplacementState"
        | "RegexSubstitutionState" => "runtime/regex",
        "JsonParseState" | "JsonNodeState" | "JsonOutputState" => "runtime/json",
        _ => return None,
    })
}
// Root-module declarations are unqualified in SPB; imported declarations carry
// the dotted module prefix. A short name is valid only in its owning module.
fn nominal_name_matches(root: &str, actual: &str, module: &str, expected: &str) -> bool {
    (root == module && actual == expected)
        || actual == format!("{}::{expected}", module.replace('/', "."))
}

/// Provider of each native, resolved once instead of testing every provider's
/// symbol prefix on each call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Provider {
    #[cfg(unix)]
    Unix,
    #[cfg(unix)]
    Root,
    #[cfg(unix)]
    Process,
    Input,
    Fs,
    Environment,
    Multicast,
    Udp,
    Network,
    Dns,
    Sqlite,
    Db,
    Json,
    Regex,
    Idna,
    Net,
    Binary,
    Secret,
    Encoding,
    Numbers,
    Buffer,
    Crypto,
    Text,
    Core,
}
fn classify(native: Native) -> Provider {
    #[cfg(unix)]
    if crate::native_unix::handles(native) {
        return Provider::Unix;
    }
    #[cfg(unix)]
    if crate::native_root::handles(native) {
        return Provider::Root;
    }
    #[cfg(unix)]
    if crate::native_process::handles(native) {
        return Provider::Process;
    }
    if crate::native_input::handles(native) {
        return Provider::Input;
    }
    if crate::native_fs::handles(native) {
        return Provider::Fs;
    }
    if crate::native_environment::handles(native) {
        return Provider::Environment;
    }
    if crate::native_multicast::handles(native) {
        return Provider::Multicast;
    }
    if crate::native_udp::handles(native) {
        return Provider::Udp;
    }
    if crate::native_network::handles(native) {
        return Provider::Network;
    }
    if crate::native_dns::handles(native) {
        return Provider::Dns;
    }
    if crate::native_sqlite::handles(native) {
        return Provider::Sqlite;
    }
    if crate::native_db::handles(native) {
        return Provider::Db;
    }
    if crate::native_json::handles(native) {
        return Provider::Json;
    }
    if crate::native_regex::handles(native) {
        return Provider::Regex;
    }
    if crate::native_idna::handles(native) {
        return Provider::Idna;
    }
    if crate::native_net::handles(native) {
        return Provider::Net;
    }
    if crate::native_binary::handles(native) {
        return Provider::Binary;
    }
    if crate::native_secret::handles(native) {
        return Provider::Secret;
    }
    if crate::native_encoding::handles(native) {
        return Provider::Encoding;
    }
    if crate::native_numbers::handles(native) {
        return Provider::Numbers;
    }
    if crate::native_buffer::handles(native) {
        return Provider::Buffer;
    }
    if crate::native_crypto::handles(native) {
        return Provider::Crypto;
    }
    if crate::native_text::handles(native) {
        return Provider::Text;
    }
    Provider::Core
}
fn provider(native: Native) -> Provider {
    static TABLE: std::sync::OnceLock<Vec<Provider>> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let size = Native::ALL
            .iter()
            .map(|n| *n as usize + 1)
            .max()
            .unwrap_or(0);
        let mut table = vec![Provider::Core; size];
        for &native in Native::ALL {
            table[native as usize] = classify(native);
        }
        table
    });
    table[native as usize]
}

#[derive(Debug)]
pub(crate) enum ProviderError {
    Engine(Error),
    Native(NativeFailure),
}
impl From<Error> for ProviderError {
    fn from(error: Error) -> Self {
        Self::Engine(error)
    }
}

#[derive(Debug)]
pub(crate) enum Outcome {
    Value(Slot),
    Failure(Slot),
}

impl Heap {
    pub(crate) fn native_invoke(
        &mut self,
        native: Native,
        args: &[Slot],
    ) -> Result<Outcome, Error> {
        match provider(native) {
            #[cfg(unix)]
            Provider::Unix => return self.native_unix(native, args),
            #[cfg(unix)]
            Provider::Root => return self.native_root(native, args),
            #[cfg(unix)]
            Provider::Process => return self.native_process(native, args),
            Provider::Input => return self.native_input(native, args),
            Provider::Fs => return self.native_fs(native, args),
            Provider::Environment => return self.native_environment(native, args),
            Provider::Multicast => return self.native_multicast(native, args),
            Provider::Udp => return self.native_udp(native, args),
            Provider::Network => return self.native_network(native, args),
            Provider::Dns => return self.native_dns(native, args),
            Provider::Sqlite => return self.native_sqlite(native, args),
            Provider::Db => return self.native_db(native, args),
            Provider::Json => return self.native_json(native, args),
            Provider::Regex => return self.native_regex(native, args),
            Provider::Idna => return self.native_idna(native, args),
            Provider::Net => return self.native_net(native, args),
            Provider::Binary => return self.native_binary(native, args),
            Provider::Secret => return self.native_secret(native, args),
            Provider::Encoding => return self.native_encoding(native, args),
            Provider::Numbers => return self.native_numbers(native, args),
            Provider::Buffer => return self.native_buffer(native, args),
            Provider::Crypto => return self.native_crypto(native, args),
            Provider::Text => return self.native_text(native, args),
            Provider::Core => {}
        }
        match native {
            Native::TestingTestStateOpen
            | Native::TestingTestStateNext
            | Native::TestingTestStateSize
            | Native::TestingTestStateClose => self.handle_test(native, args),
            Native::TestingTestContextActive => {
                Ok(Outcome::Value(Slot::boolean(self.test_failures.is_some())))
            }
            Native::TimeWallClock => {
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                self.allocate(num_bigint::BigInt::from(nanos))
                    .map(Outcome::Value)
            }
            Native::TimeMonotonicClock => {
                static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
                let nanos = ORIGIN
                    .get_or_init(std::time::Instant::now)
                    .elapsed()
                    .as_nanos();
                self.allocate(num_bigint::BigInt::from(nanos))
                    .map(Outcome::Value)
            }
            Native::TimeClockDomain => {
                if args[0].bits == 0 {
                    self.allocate_text(format!("system:{}:1", std::process::id()))
                        .map(Outcome::Value)
                } else if self.test_failures.is_some() {
                    static SERIAL: std::sync::atomic::AtomicU64 =
                        std::sync::atomic::AtomicU64::new(0);
                    let serial = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    self.allocate_text(format!("manual:{}:{serial}", std::process::id()))
                        .map(Outcome::Value)
                } else {
                    let payload = self.native_failure_new(NativeFailure {
                        domain: "core.time".into(),
                        code: "permission_denied".into(),
                        operation: native.symbol().into(),
                        message: "Manual clock domains require a running test case".into(),
                    })?;
                    self.failure_new(crate::failure::FailureKind::Native(payload))
                        .map(Outcome::Failure)
                }
            }
            Native::SystemSystemPlatform => self
                .allocate_text(
                    match std::env::consts::OS {
                        "macos" | "ios" => "macos",
                        "linux" | "android" => "linux",
                        "windows" => "windows",
                        "freebsd" => "freebsd",
                        "openbsd" => "openbsd",
                        "netbsd" => "netbsd",
                        "dragonfly" => "dragonfly",
                        "solaris" | "illumos" => "solaris",
                        "aix" => "aix",
                        _ => "unknown",
                    }
                    .into(),
                )
                .map(Outcome::Value),
            Native::SystemSystemArchitecture => self
                .allocate_text(
                    match std::env::consts::ARCH {
                        "x86_64" => "x86_64",
                        "x86" => "x86",
                        "aarch64" => "arm64",
                        "arm" => "arm",
                        "powerpc64" => "ppc64",
                        "powerpc" => "ppc",
                        "riscv64" => "riscv64",
                        "s390x" => "s390x",
                        _ => "unknown",
                    }
                    .into(),
                )
                .map(Outcome::Value),
            Native::SystemSystemByteOrder => self
                .allocate_text(
                    if cfg!(target_endian = "little") {
                        "little"
                    } else {
                        "big"
                    }
                    .into(),
                )
                .map(Outcome::Value),
            Native::TimeSleepNanoseconds => self.native_call(native, args).map(Outcome::Value),
            Native::Utf8Encode => self
                .array_op(
                    crate::array::ArrayOp::Utf8,
                    [args[0], Slot::default(), Slot::default()],
                )
                .map(Outcome::Value),
            Native::StdoutWrite => {
                self.native_write(native, args[0], &mut std::io::stdout().lock())
            }
            Native::StderrWrite => {
                self.native_write(native, args[0], &mut std::io::stderr().lock())
            }
            _ => Err(Error("unsupported native invocation".into())),
        }
    }
    fn native_write(
        &mut self,
        native: Native,
        value: Slot,
        stream: &mut dyn std::io::Write,
    ) -> Result<Outcome, Error> {
        let bytes: Vec<u8> = self
            .elements(value)?
            .iter()
            .map(|v| {
                if v.tag == BYTE {
                    Ok(v.bits as u8)
                } else {
                    Err(Error("expected byte element".into()))
                }
            })
            .collect::<Result<_, _>>()?;
        if bytes.is_empty() {
            return Ok(Outcome::Value(Slot::small(0)));
        }
        let result = retry(|| stream.write(&bytes)).and_then(|written| {
            if written == 0 {
                return Err(std::io::Error::from(std::io::ErrorKind::WriteZero));
            }
            retry(|| stream.flush())?;
            Ok(written)
        });
        match result {
            Ok(written) => Ok(Outcome::Value(Slot::small(written as i64))),
            Err(error) => {
                let payload = self.native_failure_new(NativeFailure {
                    domain: "core.stdio".into(),
                    code: io_code(error.kind()).into(),
                    operation: native.symbol().into(),
                    message: if native == Native::StdoutWrite {
                        "Cannot write standard output"
                    } else {
                        "Cannot write standard error output"
                    }
                    .into(),
                })?;
                Ok(Outcome::Failure(self.failure_new(
                    crate::failure::FailureKind::Native(payload),
                )?))
            }
        }
    }
    pub fn native_failure_new(&mut self, failure: NativeFailure) -> Result<Slot, Error> {
        let fields = [
            self.allocate_text(failure.domain)?,
            self.allocate_text(failure.code)?,
            self.allocate_text(failure.operation)?,
            self.allocate_text(failure.message)?,
        ];
        Ok(Slot {
            bits: self.native_failures.allocate(fields)?,
            tag: NATIVE_FAILURE,
        })
    }
    pub fn native_failure_fields(&self, value: Slot) -> Result<&[Slot; 4], Error> {
        if value.tag != NATIVE_FAILURE {
            return Err(Error("expected NativeFailure handle".into()));
        }
        self.native_failures.get(value.bits)
    }
    pub fn native_failure(&self, value: Slot) -> Result<NativeFailure, Error> {
        let fields = self.native_failure_fields(value)?;
        Ok(NativeFailure {
            domain: self.text(fields[0])?.into(),
            code: self.text(fields[1])?.into(),
            operation: self.text(fields[2])?.into(),
            message: self.text(fields[3])?.into(),
        })
    }
    pub fn native_call(&mut self, native: Native, args: &[Slot]) -> Result<Slot, Error> {
        match native {
            Native::TimeSleepNanoseconds => {
                let source = self.text(args[0])?;
                let canonical = source == "0"
                    || (!source.is_empty()
                        && !source.starts_with('0')
                        && source.bytes().all(|b| b.is_ascii_digit()));
                let nanos = source.parse::<i64>().ok();
                let invalid = if !canonical {
                    Some("Wait duration must be a canonical non-negative integer")
                } else if nanos.is_none() {
                    Some("Wait duration exceeds the runtime timer range")
                } else {
                    None
                };
                let state = if let Some(message) = invalid {
                    let payload = self.native_failure_new(NativeFailure {
                        domain: "core.time".into(),
                        code: "invalid_input".into(),
                        operation: native.symbol().into(),
                        message: message.into(),
                    })?;
                    let failure = self.failure_new(crate::failure::FailureKind::Native(payload))?;
                    crate::future::State::Ready(Err(failure))
                } else if nanos == Some(0) {
                    crate::future::State::Ready(Ok(Slot::boolean(true)))
                } else {
                    let deadline = std::time::Instant::now()
                        .checked_add(std::time::Duration::from_nanos(
                            nanos.expect("validated duration") as u64,
                        ))
                        .ok_or_else(|| Error("timer deadline exceeds host range".into()))?;
                    crate::future::State::Timer(deadline)
                };
                self.future_with_state(state)
            }
            _ => Err(Error("unsupported native request".into())),
        }
    }
}

pub(crate) fn validate_builtins(program: &Program) -> Result<(), Error> {
    for object in &program.objects {
        if object.is_native_failure()
            && (object.destroy.is_some()
                || object.fields.len() != 4
                || object
                    .fields
                    .iter()
                    .zip(["domain", "code", "operation", "message"])
                    .any(|((name, ty), expected)| {
                        name != expected || crate::kind(program, *ty) != Some(Kind::Str)
                    }))
        {
            return Err(Error("invalid NativeFailure builtin layout".into()));
        }
    }
    Ok(())
}
pub(crate) fn is_error_role(code: &Code, role: u32) -> bool {
    code.error_role == Some(role)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_supported_abi_signature_loads_with_local_and_imported_types() {
        use okkama_runtime_abi::Type as Abi;
        use okkama_sparrow::{Function, Register, TypeRef};
        fn ty(p: &mut Program, t: &Abi, module: &str, local: bool) -> Option<TypeRef> {
            let t = match t {
                Abi::Void => return None,
                Abi::Int => Type::Int,
                Abi::Float => Type::Float,
                Abi::Rat => Type::Rat,
                Abi::Bool => Type::Bool,
                Abi::Byte => Type::Byte,
                Abi::Char => Type::Char,
                Abi::Str => Type::Str,
                Abi::Any => Type::Any,
                Abi::Array(item) => Type::Array(ty(p, item, module, local).unwrap()),
                Abi::Map(key, value) => Type::Map(
                    ty(p, key, module, local).unwrap(),
                    ty(p, value, module, local).unwrap(),
                ),
                Abi::Handle(name) => {
                    let owner = handle_module(name).unwrap_or(module);
                    let name = if local && owner == module {
                        name.to_string()
                    } else {
                        format!("{}::{name}", owner.replace('/', "."))
                    };
                    let index = p
                        .handles
                        .iter()
                        .position(|n| n == &name)
                        .unwrap_or_else(|| {
                            p.handles.push(name);
                            p.handles.len() - 1
                        });
                    Type::Handle(index as u32)
                }
            };
            let index = p.types.iter().position(|old| old == &t).unwrap_or_else(|| {
                p.types.push(t);
                p.types.len() - 1
            });
            Some(TypeRef(index as u32))
        }
        for &native in Native::ALL {
            if !supported(native) {
                continue;
            }
            let module = native.symbol().split_once('@').unwrap().0;
            for local in [false, true] {
                let signature = native.signature();
                let mut program = Program {
                    module: if local { module.into() } else { "probe".into() },
                    version: "v1.0.0".into(),
                    ..Program::default()
                };
                let registers = signature
                    .params
                    .iter()
                    .map(|t| ty(&mut program, t, module, local).unwrap())
                    .collect::<Vec<_>>();
                let params = (0..registers.len()).map(|r| Register(r as u32)).collect();
                let result = ty(&mut program, &signature.result, module, local);
                program.functions.push(Function {
                    symbol: "native".into(),
                    is_async: signature.is_async,
                    native: Some(native.symbol().into()),
                    intrinsic: None,
                    registers,
                    params,
                    result,
                    code: vec![],
                    handlers: vec![],
                });
                assert!(
                    lower(&program, 0).is_ok(),
                    "{} local={local}",
                    native.symbol()
                );
                program.functions[0].is_async = !signature.is_async;
                assert!(
                    lower(&program, 0).is_err(),
                    "async mismatch: {}",
                    native.symbol()
                );
                program.functions[0].is_async = signature.is_async;
                let wrong = TypeRef(program.types.len() as u32);
                program.types.push(Type::Never);
                program.functions[0].result = Some(wrong);
                assert!(
                    lower(&program, 0).is_err(),
                    "result mismatch: {}",
                    native.symbol()
                );
            }
        }
    }
    #[test]
    fn native_handle_names_are_local_only_in_the_owning_module() {
        assert!(nominal_name_matches(
            "runtime/json",
            "JsonNodeState",
            "runtime/json",
            "JsonNodeState"
        ));
        assert!(nominal_name_matches(
            "app",
            "runtime.json::JsonNodeState",
            "runtime/json",
            "JsonNodeState"
        ));
        assert!(!nominal_name_matches(
            "app",
            "JsonNodeState",
            "runtime/json",
            "JsonNodeState"
        ));
        assert!(!nominal_name_matches(
            "runtime/regex",
            "JsonNodeState",
            "runtime/json",
            "JsonNodeState"
        ));
        assert!(!nominal_name_matches(
            "runtime/json",
            "foreign::JsonNodeState",
            "runtime/json",
            "JsonNodeState"
        ));
    }
    #[test]
    fn bigint_clock_bridge_preserves_decimal_spelling_rules_and_exact_order() {
        let mut heap = Heap::new(&[], &[]);
        for (source, canonical) in [
            ("-000", Some("0")),
            ("00012", Some("12")),
            ("-00012", Some("-12")),
            ("+1", None),
            ("1_0", None),
            (" 1", None),
            ("-", None),
            ("", None),
            ("١", None),
        ] {
            let input = heap.allocate_text(source.into()).unwrap();
            let Outcome::Value(valid) = heap.native_invoke(Native::BigintValid, &[input]).unwrap()
            else {
                panic!("validation");
            };
            assert_eq!(valid.bits != 0, canonical.is_some());
            let result = heap.native_invoke(Native::BigintCanonical, &[input]);
            if let Some(expected) = canonical {
                let Outcome::Value(value) = result.unwrap() else {
                    panic!("canonical");
                };
                assert_eq!(heap.text(value).unwrap(), expected);
            } else {
                assert!(result.is_err());
            }
        }
        let left = heap
            .allocate_text("922337203685477580800000000001".into())
            .unwrap();
        let right = heap
            .allocate_text("922337203685477580800000000000".into())
            .unwrap();
        let Outcome::Value(order) = heap
            .native_invoke(Native::BigintOrder, &[left, right])
            .unwrap()
        else {
            panic!("order");
        };
        assert_eq!(order.bits, 1);
    }

    #[test]
    fn invalid_timer_inputs_produce_typed_cached_failures_and_zero_is_ready() {
        let mut heap = Heap::new(&[], &[]);
        for source in ["", "01", "-1", "+1", "1.0", " 1", "9223372036854775808"] {
            let value = heap.allocate_text(source.into()).unwrap();
            let future = heap
                .native_call(Native::TimeSleepNanoseconds, &[value])
                .unwrap();
            let crate::future::State::Ready(Err(failure)) = heap.future(future).unwrap().state
            else {
                panic!("invalid duration did not fail")
            };
            let crate::failure::FailureKind::Native(payload) = heap.failure(failure).unwrap().kind
            else {
                panic!("wrong failure category")
            };
            heap.collect([failure].into_iter()).unwrap();
            let record = heap.native_failure(payload).unwrap();
            assert_eq!(record.domain, "core.time");
            assert_eq!(record.code, "invalid_input");
            assert_eq!(
                record.message,
                if source == "9223372036854775808" {
                    "Wait duration exceeds the runtime timer range"
                } else {
                    "Wait duration must be a canonical non-negative integer"
                }
            );
            let identical = heap.native_failure_new(record).unwrap();
            assert!(heap.equal(payload, identical).unwrap());
        }
        let zero = heap.allocate_text("0".into()).unwrap();
        let future = heap
            .native_call(Native::TimeSleepNanoseconds, &[zero])
            .unwrap();
        assert!(matches!(
            heap.future(future).unwrap().state,
            crate::future::State::Ready(Ok(Slot { tag: BOOL, bits: 1 }))
        ));
    }
}

fn retry<T>(mut operation: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    loop {
        match operation() {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            result => return result,
        }
    }
}
pub(crate) fn io_code(kind: std::io::ErrorKind) -> &'static str {
    use std::io::ErrorKind::*;
    match kind {
        NotFound => "not_found",
        PermissionDenied => "permission_denied",
        ConnectionRefused => "connection_refused",
        ConnectionReset | ConnectionAborted => "connection_reset",
        TimedOut => "timed_out",
        AddrInUse => "address_in_use",
        AddrNotAvailable => "address_not_available",
        NotConnected => "not_connected",
        BrokenPipe => "broken_pipe",
        WouldBlock => "would_block",
        Interrupted => "interrupted",
        InvalidInput => "invalid_input",
        Unsupported => "unsupported",
        OutOfMemory => "resource_exhausted",
        AlreadyExists => "already_exists",
        _ => "unexpected",
    }
}

#[cfg(test)]
mod stdio_tests {
    use super::*;
    #[derive(Default)]
    struct Output {
        data: Vec<u8>,
        interrupted: bool,
        broken: bool,
        flushes: usize,
    }
    impl std::io::Write for Output {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if std::mem::take(&mut self.interrupted) {
                return Err(std::io::ErrorKind::Interrupted.into());
            }
            if self.broken {
                return Err(std::io::ErrorKind::BrokenPipe.into());
            }
            let size = bytes.len().min(2);
            self.data.extend_from_slice(&bytes[..size]);
            Ok(size)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.flushes += 1;
            Ok(())
        }
    }
    #[test]
    fn utf8_and_stdio_preserve_binary_prefixes_retry_interrupted_and_report_typed_failure() {
        let mut heap = Heap::new(&[], &[]);
        let text = heap.allocate_text("A\0😀".into()).unwrap();
        let Outcome::Value(bytes) = heap.native_invoke(Native::Utf8Encode, &[text]).unwrap() else {
            panic!("encoding failed");
        };
        assert_eq!(heap.elements(bytes).unwrap().len(), 6);
        let mut output = Output {
            interrupted: true,
            ..Output::default()
        };
        let Outcome::Value(count) = heap
            .native_write(Native::StdoutWrite, bytes, &mut output)
            .unwrap()
        else {
            panic!("write failed");
        };
        assert_eq!(heap.integer(count).unwrap(), 2);
        assert_eq!(output.data, b"A\0");
        assert_eq!(output.flushes, 1);
        output.broken = true;
        let Outcome::Failure(failure) = heap
            .native_write(Native::StderrWrite, bytes, &mut output)
            .unwrap()
        else {
            panic!("missing typed failure");
        };
        let crate::failure::FailureKind::Native(payload) = heap.failure(failure).unwrap().kind
        else {
            panic!("wrong failure kind");
        };
        let report = heap.native_failure(payload).unwrap();
        assert_eq!(report.domain, "core.stdio");
        assert_eq!(report.code, "broken_pipe");
        assert_eq!(report.operation, Native::StderrWrite.symbol());
        let empty = heap.allocate_array(vec![]).unwrap();
        assert!(matches!(
            heap.native_write(Native::StdoutWrite, empty, &mut output)
                .unwrap(),
            Outcome::Value(Slot { bits: 0, .. })
        ));
        assert_eq!(output.flushes, 1);
    }
}
