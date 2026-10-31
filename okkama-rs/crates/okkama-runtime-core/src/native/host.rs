//! Natives часов, ожидания, случайности и сведений хоста — перенос
//! `HostNatives` JVM: `runtime/time` (часы, сон), `runtime/system`
//! (secure_random, платформа, хост, хранилище) и выборки `runtime/random`.
use super::support::{byte_array, bytes, failure, int, int_value, string, string_map, text};
use super::Error;
use crate::bignum::Big;
use crate::sched;
use crate::{IntValue, Value, abi::Native};
use std::sync::OnceLock;

fn operation(native: Native) -> &'static str {
    native.name()
}

fn origin() -> std::time::Instant {
    static ORIGIN: OnceLock<std::time::Instant> = OnceLock::new();
    *ORIGIN.get_or_init(std::time::Instant::now)
}

thread_local! {
    static DOMAIN_SERIAL: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
}

fn unsigned(sample: &[u8]) -> Big {
    let base = Big::from_u64(256);
    sample
        .iter()
        .fold(Big::zero(), |number, octet| number.multiply(&base).add(&Big::from_u64(u64::from(*octet))))
}

fn platform() -> &'static str {
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
}

fn architecture() -> &'static str {
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
}

fn host_name(native: Native) -> Result<String, Error> {
    let from_command = std::process::Command::new("hostname")
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|name| !name.is_empty());
    from_command.ok_or_else(|| failure("core.system", "unexpected", native, "Host name is unavailable"))
}

fn storage_usage(native: Native, path: &str) -> Result<Value, Error> {
    if path.contains('\0') {
        return Err(failure(
            "core.system",
            "invalid_input",
            native,
            "Storage path contains a zero byte",
        ));
    }
    let absolute = std::fs::canonicalize(crate::host::path(path)).map_err(|error| {
        failure(
            "core.system",
            crate::host::code(&error),
            native,
            "Cannot resolve storage path",
        )
    })?;
    let df = ["/bin/df", "/usr/bin/df"]
        .into_iter()
        .find(|candidate| std::path::Path::new(candidate).is_file())
        .ok_or_else(|| failure("core.system", "unsupported", native, "Storage usage is unavailable"))?;
    let unexpected = || failure("core.system", "unexpected", native, "Cannot inspect storage usage");
    let output = std::process::Command::new(df)
        .arg("-Pk")
        .arg(&absolute)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .map_err(|_| unexpected())?;
    if !output.status.success() {
        return Err(unexpected());
    }
    let listing = String::from_utf8_lossy(&output.stdout);
    let last = listing.lines().last().unwrap_or("");
    // ^\S+\s+([0-9]+)\s+[0-9]+\s+([0-9]+)\s+
    let fields: Vec<&str> = last.split_whitespace().collect();
    let invalid = || failure("core.system", "invalid_data", native, "Invalid storage usage result");
    let number = |text: &str| -> Option<IntValue> {
        (!text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()))
            .then(|| IntValue::from_decimal_digits(text).expect("digits"))
    };
    if fields.len() < 5 {
        return Err(invalid());
    }
    let (Some(capacity), Some(_), Some(available)) = (number(fields[1]), number(fields[2]), number(fields[3])) else {
        return Err(invalid());
    };
    let kilo = IntValue::from_i64(1024);
    let capacity = crate::integer::multiply(&capacity, &kilo);
    let available = crate::integer::multiply(&available, &kilo);
    if crate::integer::compare(&available, &capacity).is_gt() {
        return Err(failure(
            "core.system",
            "invalid_data",
            native,
            "Inconsistent storage usage result",
        ));
    }
    Ok(string_map([
        ("capacity".to_string(), capacity.to_string()),
        ("used".to_string(), crate::integer::subtract(&capacity, &available).to_string()),
        ("available".to_string(), available.to_string()),
    ]))
}

fn sleep(native: Native, source: &str) -> Value {
    let invalid = |message: &str| {
        let Error::Failure(failure) = failure("core.time", "invalid_input", native, message) else {
            unreachable!("failure builds NativeFailure")
        };
        crate::failure::Failure::new(crate::failure::Kind::Native(std::rc::Rc::new(failure)))
    };
    let canonical = source == "0"
        || (!source.is_empty() && !source.starts_with('0') && source.bytes().all(|b| b.is_ascii_digit()));
    if !canonical {
        return Value::Future(sched::resolved(Err(invalid(
            "Wait duration must be a canonical non-negative integer",
        ))));
    }
    let Some(nanoseconds) = source.parse::<i64>().ok() else {
        return Value::Future(sched::resolved(Err(invalid(
            "Wait duration exceeds the runtime timer range",
        ))));
    };
    if nanoseconds == 0 {
        return Value::Future(sched::resolved(Ok(Value::Bool(true))));
    }
    let promise = sched::promise();
    let target = promise.clone();
    let op = sched::offload(
        move || std::thread::sleep(std::time::Duration::from_nanos(nanoseconds as u64)),
        move |()| sched::settle(&target, Ok(Value::Bool(true))),
    );
    // Отменённое ожидание не держит программу.
    promise.listen(move || op.detach());
    Value::Future(promise)
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    match operation(native) {
        "wall_clock" => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            Ok(Value::Int(IntValue::from_u128(now.as_nanos())))
        }
        "monotonic_clock" => Ok(Value::Int(IntValue::from_u128(origin().elapsed().as_nanos()))),
        "clock_domain" => {
            let pid = std::process::id();
            if !args[0].as_bool()? {
                return Ok(text(format!("system:{pid}:1")));
            }
            if !crate::testing::active() {
                return Err(failure(
                    "core.time",
                    "permission_denied",
                    native,
                    "Manual clock domains require a running test case",
                ));
            }
            let serial = DOMAIN_SERIAL.with(|serial| {
                serial.set(serial.get() + 1);
                serial.get()
            });
            Ok(text(format!("manual:{pid}:{serial}")))
        }
        "sleep_nanoseconds" => Ok(sleep(native, string(&args[0])?)),
        "secure_random" => {
            let size = int(&args[0])?;
            if size.is_negative() {
                return Err(failure(
                    "core.system",
                    "invalid_input",
                    native,
                    "Secure random size must be non-negative",
                ));
            }
            let size = size
                .to_i64()
                .filter(|size| *size <= 16 * 1024 * 1024)
                .ok_or_else(|| {
                    failure(
                        "core.system",
                        "resource_exhausted",
                        native,
                        "Secure random request exceeds 16 MiB",
                    )
                })?;
            let random = okkama_native::call("random", &[size.to_string().as_bytes()])
                .map_err(|error| failure("core.system", "unexpected", native, error.message()))?;
            Ok(byte_array(random))
        }
        "random_sample_accepted" | "random_integer_from_sample" => {
            let sample = bytes(&args[0])?;
            if sample.len() != 8 {
                return Err(failure(
                    "core.random",
                    "invalid_input",
                    native,
                    "Random integer sample must have eight bytes",
                ));
            }
            let value = IntValue::from_big(unsigned(&sample));
            let (start, stop) = (int(&args[1])?, int(&args[2])?);
            let span = crate::integer::subtract(stop, start);
            if operation(native) == "random_sample_accepted" {
                if span.signum() <= 0 {
                    return Ok(Value::Bool(false));
                }
                let modulus = IntValue::from_big(Big::from_u64(2).power(64));
                let threshold = crate::integer::divide(&modulus, &span).expect("positive").1;
                return Ok(Value::Bool(crate::integer::compare(&value, &threshold).is_ge()));
            }
            if span.is_zero() {
                // Math::BigInt: x mod 0 = x.
                return Ok(Value::Int(crate::integer::add(start, &value)));
            }
            let magnitude = if span.is_negative() { span.negated() } else { span.clone() };
            let mut rest = crate::integer::divide(&value, &magnitude).expect("non-zero").1;
            // Остаток со знаком делителя, как у Math::BigInt.
            if span.is_negative() && !rest.is_zero() {
                rest = crate::integer::add(&rest, &span);
            }
            Ok(Value::Int(crate::integer::add(start, &rest)))
        }
        "random_float_from_sample" => {
            let sample = bytes(&args[0])?;
            if sample.len() != 7 {
                return Err(failure(
                    "core.random",
                    "invalid_input",
                    native,
                    "Random float sample must have seven bytes",
                ));
            }
            let value = sample.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b)) >> 3;
            Ok(Value::Float(value as f64 / 9_007_199_254_740_992.0))
        }
        "system_platform" => Ok(text(platform())),
        "system_architecture" => Ok(text(architecture())),
        "system_byte_order" => Ok(text(if cfg!(target_endian = "little") { "little" } else { "big" })),
        "system_host_name" => Ok(text(host_name(native)?)),
        "system_cpu_count" => {
            if let Ok(environment) = std::env::var("NUMBER_OF_PROCESSORS") {
                let trimmed = environment.trim();
                if !trimmed.is_empty() && !trimmed.starts_with('0') && trimmed.bytes().all(|b| b.is_ascii_digit()) {
                    return Ok(Value::Int(IntValue::from_decimal_digits(trimmed).expect("digits")));
                }
            }
            let count = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
            Ok(int_value(count.max(1) as i64))
        }
        "storage_usage" => storage_usage(native, string(&args[0])?),
        _ => Err(Error::Unsupported(format!(
            "native provider is not implemented: {}",
            native.symbol()
        ))),
    }
}
