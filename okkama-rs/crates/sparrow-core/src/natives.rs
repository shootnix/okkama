//! Native providers. Arguments and results are words whose representation
//! follows the native's SPB signature. The full provider set moves here in
//! stage 4; stage 2 needs the standard output path.
use crate::runtime::{FailureKind, Runtime};
use crate::word::{Repr, Word};
use okkama_runtime_abi::Native;

#[derive(Debug)]
pub enum Refusal {
    Failure(FailureKind),
    /// An existing failure word (a provider's NativeFailure).
    Raise(Word),
    Fatal(String),
    /// A budget ran out (`budget`).
    Exhausted(crate::budget::Exhausted),
}

/// Natives this core provides.
pub fn supported(native: Native) -> bool {
    matches!(
        native,
        Native::Utf8Encode
            | Native::StdoutWrite
            | Native::StderrWrite
            | Native::NumberTextIntParse
            | Native::NumberTextIntParseValid
            | Native::TimeSleepNanoseconds
            | Native::Utf8ValidPrefix
            | Native::Utf8Decode
            | Native::BytesBytesFind
            | Native::BytesBytesUtf8Invalid
            | Native::BytesBytesText
            | Native::BytesBytesDecimalInvalid
            | Native::BytesBytesDecimal
            | Native::BytesBytesEqual
            | Native::BytesBytesIntegerFits
            | Native::BytesBytesInteger
            | Native::BytesBytesJoin
    ) || crate::host::supported(native)
}

#[cfg(test)]
thread_local! {
    /// Tests: the next native call on this thread panics (panic isolation).
    pub(crate) static PANIC_NEXT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn call(runtime: &mut Runtime, function: usize, arguments: &[Word]) -> Result<(Word, Repr), Refusal> {
    #[cfg(test)]
    if PANIC_NEXT.with(|next| next.replace(false)) {
        panic!("provider failure");
    }
    // A missing provider is a runtime error of the call, after earlier effects
    // (conformance L06.native.missing-provider).
    let native = runtime.natives[function].ok_or_else(|| {
        let symbol = runtime.loaded.program.functions[function].native.clone().unwrap_or_default();
        Refusal::Failure(FailureKind::Program(missing_native(&symbol)))
    })?;
    runtime.limits.permits(native).map_err(|message| Refusal::Failure(FailureKind::Program(message)))?;
    match native {
        Native::Utf8Encode => {
            let bytes = runtime.heap.text(arguments[0]).as_bytes().to_vec();
            Ok((runtime.heap.allocate_bytes(&bytes), Repr::Ref))
        }
        Native::StdoutWrite | Native::StderrWrite => {
            let bytes = runtime.heap.byte_view(arguments[0]).into_owned();
            if let Some(limit) = runtime.limits.output {
                let written = runtime.budget.written.saturating_add(bytes.len() as u64);
                if written > limit {
                    return Err(Refusal::Exhausted(crate::budget::Exhausted::Output));
                }
                runtime.budget.written = written;
            }
            let written = if native == Native::StdoutWrite {
                std::io::Write::write_all(&mut std::io::stdout().lock(), &bytes)
            } else {
                std::io::Write::write_all(&mut std::io::stderr().lock(), &bytes)
            };
            match written {
                Ok(()) => Ok((runtime.heap.int(bytes.len() as i64), Repr::Int)),
                Err(error) => Err(Refusal::Failure(FailureKind::Program(error.to_string()))),
            }
        }
        Native::NumberTextIntParse | Native::NumberTextIntParseValid => {
            let radix = runtime.small_or_big(arguments[1]);
            let text = runtime.heap.text(arguments[0]);
            // Decimal integers of up to 18 digits need no BigInt.
            if radix == Some(10)
                && let Some(value) = small_decimal(text) {
                    return Ok(if native == Native::NumberTextIntParseValid {
                        (1, Repr::Raw)
                    } else {
                        (runtime.heap.int(value), Repr::Int)
                    });
                }
            let parsed = radix.and_then(|radix| int_parse(text, radix));
            if native == Native::NumberTextIntParseValid {
                return Ok((u64::from(parsed.is_some()), Repr::Raw));
            }
            match parsed {
                Some(value) => Ok((runtime.heap.big(value), Repr::Int)),
                // The typed failure comes from the provider.
                None => host(runtime, native, arguments),
            }
        }
        Native::Utf8ValidPrefix | Native::Utf8Decode => {
            let array = arguments[0];
            if !runtime.heap.is_bytes(array) {
                return host(runtime, native, arguments);
            }
            let bytes = runtime.heap.bytes(array);
            let valid = std::str::from_utf8(bytes).map_or_else(|e| e.valid_up_to(), |_| bytes.len());
            if native == Native::Utf8ValidPrefix {
                return Ok((runtime.heap.int(valid as i64), Repr::Int));
            }
            if valid != bytes.len() {
                return host(runtime, native, arguments);
            }
            match runtime.heap.text_from_bytes(array) {
                Some(text) => Ok((text, Repr::Ref)),
                None => host(runtime, native, arguments),
            }
        }
        Native::BytesBytesFind
        | Native::BytesBytesUtf8Invalid
        | Native::BytesBytesText
        | Native::BytesBytesDecimalInvalid
        | Native::BytesBytesDecimal
        | Native::BytesBytesEqual
        | Native::BytesBytesIntegerFits
        | Native::BytesBytesInteger
        | Native::BytesBytesJoin => bytes(runtime, native, arguments),
        _ if crate::host::supported(native) => host(runtime, native, arguments),
        _ => Err(Refusal::Fatal(format!(
            "native provider {} is not available in sparrow-core yet",
            native.symbol()
        ))),
    }
}

/// `runtime/bytes`: views of a byte range without copying it (`encoding/bytes`
/// checks ranges first; out-of-range arguments are refused here too).
fn bytes(runtime: &mut Runtime, native: Native, arguments: &[Word]) -> Result<(Word, Repr), Refusal> {
    let refuse = || Refusal::Failure(FailureKind::Program(format!("Native invocation \"{}\" failed", native.symbol())));
    let source = arguments[0];
    let length = runtime.heap.byte_view(source).len();
    if native == Native::BytesBytesFind {
        let start = runtime.small_or_big(arguments[2]).and_then(|s| usize::try_from(s).ok()).filter(|&s| s <= length);
        let start = start.ok_or_else(refuse)?;
        let found = {
            let (haystack, needle) = (runtime.heap.byte_view(source), runtime.heap.byte_view(arguments[1]));
            if needle.is_empty() {
                return Err(refuse());
            }
            find(&haystack[start..], &needle).map_or(-1, |at| (start + at) as i64)
        };
        return Ok((runtime.heap.int(found), Repr::Int));
    }
    if native == Native::BytesBytesJoin {
        let start = runtime.small_or_big(arguments[1]).and_then(|s| usize::try_from(s).ok()).filter(|&s| s <= length);
        let start = start.ok_or_else(refuse)?;
        let joined = {
            let (head, tail) = (runtime.heap.byte_view(source), runtime.heap.byte_view(arguments[2]));
            let mut joined = Vec::with_capacity(length - start + tail.len());
            joined.extend_from_slice(&head[start..]);
            joined.extend_from_slice(&tail);
            joined
        };
        return Ok((runtime.heap.allocate_bytes(&joined), Repr::Ref));
    }
    let bound = |word: Word| runtime.small_or_big(word).and_then(|n| usize::try_from(n).ok());
    let (Some(start), Some(end)) = (bound(arguments[1]), bound(arguments[2])) else {
        return Err(refuse());
    };
    if start > end || end > length {
        return Err(refuse());
    }
    let int = |runtime: &mut Runtime, n: i64| Ok((runtime.heap.int(n), Repr::Int));
    match native {
        Native::BytesBytesUtf8Invalid => {
            let invalid = {
                let view = runtime.heap.byte_view(source);
                std::str::from_utf8(&view[start..end]).err().map_or(-1, |e| e.valid_up_to() as i64)
            };
            int(runtime, invalid)
        }
        Native::BytesBytesText => {
            runtime.heap.text_from_byte_range(source, start, end).map(|t| (t, Repr::Ref)).ok_or_else(refuse)
        }
        Native::BytesBytesDecimalInvalid | Native::BytesBytesDecimal => {
            // ASCII decimal, optional leading '-': the offset (within the
            // range) of the first byte that breaks it, or -1.
            let decimal = native == Native::BytesBytesDecimal;
            let (invalid, value) = {
                let view = runtime.heap.byte_view(source);
                let digits = &view[start..end];
                let sign = usize::from(digits.first() == Some(&b'-'));
                let invalid = match digits[sign..].iter().position(|b| !b.is_ascii_digit()) {
                    Some(at) => (sign + at) as i64,
                    None if digits.len() == sign => sign as i64,
                    None => -1,
                };
                // Up to 18 digits fit an i64 without overflow checks; longer
                // values are BigInts (Err).
                let value = (decimal && invalid < 0).then(|| {
                    if digits.len() - sign <= 18 {
                        let n = digits[sign..].iter().fold(0i64, |n, &b| n * 10 + i64::from(b - b'0'));
                        Ok(if sign == 1 { -n } else { n })
                    } else {
                        Err(num_bigint::BigInt::parse_bytes(digits, 10).expect("ASCII decimal"))
                    }
                });
                (invalid, value)
            };
            match value {
                _ if !decimal => int(runtime, invalid),
                Some(Ok(n)) => int(runtime, n),
                Some(Err(n)) => Ok((runtime.heap.big(n), Repr::Int)),
                None => Err(refuse()),
            }
        }
        Native::BytesBytesEqual => {
            let equal = {
                let (view, other) = (runtime.heap.byte_view(source), runtime.heap.byte_view(arguments[3]));
                view[start..end] == *other
            };
            Ok((u64::from(equal), Repr::Raw))
        }
        _ => {
            // Fixed-width integer: two's complement or magnitude, as
            // `encoding/binary` decodes it; it must fit the signed 64-bit int.
            let (signed, big) = (arguments[3] != 0, arguments[4] != 0);
            let value = {
                let view = runtime.heap.byte_view(source);
                let digits = &view[start..end];
                if digits.is_empty() {
                    None
                } else {
                    let n = match (signed, big) {
                        (true, true) => num_bigint::BigInt::from_signed_bytes_be(digits),
                        (true, false) => num_bigint::BigInt::from_signed_bytes_le(digits),
                        (false, true) => num_bigint::BigInt::from_bytes_be(num_bigint::Sign::Plus, digits),
                        (false, false) => num_bigint::BigInt::from_bytes_le(num_bigint::Sign::Plus, digits),
                    };
                    num_traits::ToPrimitive::to_i64(&n)
                }
            };
            if native == Native::BytesBytesIntegerFits {
                return Ok((u64::from(value.is_some()), Repr::Raw));
            }
            int(runtime, value.ok_or_else(refuse)?)
        }
    }
}

/// First occurrence of a non-empty `needle`. Protocol delimiters are a byte
/// or two: scan for the first byte instead of building a `memmem` searcher.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.len() > 8 {
        return memchr::memmem::find(haystack, needle);
    }
    let (first, rest) = (needle[0], &needle[1..]);
    let mut from = 0;
    while let Some(at) = memchr::memchr(first, &haystack[from..]) {
        let at = from + at;
        if haystack[at + 1..].starts_with(rest) {
            return Some(at);
        }
        from = at + 1;
    }
    None
}

fn host(runtime: &mut Runtime, native: Native, arguments: &[Word]) -> Result<(Word, Repr), Refusal> {
    match crate::host::call(runtime, native, arguments) {
        crate::host::Result2::Value(word, repr) => Ok((word, repr)),
        crate::host::Result2::Raise(failure) => Err(Refusal::Raise(failure)),
        crate::host::Result2::Refused(message) => Err(Refusal::Failure(FailureKind::Program(message))),
    }
}

/// A decimal integer of at most 18 digits (optional `-`), exactly as
/// `int_parse` with radix 10 reads it.
fn small_decimal(text: &str) -> Option<i64> {
    let (negative, digits) = text.strip_prefix('-').map_or((false, text), |d| (true, d));
    if digits.is_empty() || digits.len() > 18 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let value = digits.bytes().fold(0i64, |n, b| n * 10 + i64::from(b - b'0'));
    Some(if negative { -value } else { value })
}

/// Integer text in `radix` (2..=36), or with a 0b/0o/0x prefix when radix is 0.
fn int_parse(text: &str, radix: i64) -> Option<num_bigint::BigInt> {
    if radix != 0 && !(2..=36).contains(&radix) {
        return None;
    }
    let (negative, unsigned) = text.strip_prefix('-').map_or((false, text), |s| (true, s));
    let (base, digits) = if radix == 0 {
        match unsigned.get(..2) {
            Some("0b" | "0B") => (2, &unsigned[2..]),
            Some("0o" | "0O") => (8, &unsigned[2..]),
            Some("0x" | "0X") => (16, &unsigned[2..]),
            _ => (10, unsigned),
        }
    } else {
        (radix as u32, unsigned)
    };
    if digits.is_empty() || !digits.bytes().all(|b| (b as char).to_digit(36).is_some_and(|d| d < base)) {
        return None;
    }
    let value = num_bigint::BigInt::parse_bytes(digits.as_bytes(), base)?;
    Some(if negative { -value } else { value })
}

/// As the other engines: a runtime symbol without a provider is not
/// implemented; a program's own native symbol is not registered.
pub(crate) fn missing_native(symbol: &str) -> String {
    if symbol.starts_with("runtime/") {
        format!("native provider is not implemented: {symbol}")
    } else {
        format!("Native symbol \"{symbol}\" is not registered")
    }
}
