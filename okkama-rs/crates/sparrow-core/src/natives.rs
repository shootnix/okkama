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
    ) || crate::host::supported(native)
}

pub fn call(runtime: &mut Runtime, function: usize, arguments: &[Word]) -> Result<(Word, Repr), Refusal> {
    // A missing provider is a runtime error of the call, after earlier effects
    // (conformance L06.native.missing-provider).
    let native = runtime.natives[function]
        .ok_or_else(|| Refusal::Failure(FailureKind::Program("unknown native provider symbol".into())))?;
    match native {
        Native::Utf8Encode => {
            let bytes: Vec<Word> = runtime.heap.text(arguments[0]).bytes().map(Word::from).collect();
            Ok((runtime.heap.allocate_array(Repr::Raw, &bytes), Repr::Ref))
        }
        Native::StdoutWrite | Native::StderrWrite => {
            let bytes: Vec<u8> = runtime.heap.elements(arguments[0]).iter().map(|&b| b as u8).collect();
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
            let parsed = radix.and_then(|radix| int_parse(runtime.heap.text(arguments[0]), radix));
            if native == Native::NumberTextIntParseValid {
                return Ok((u64::from(parsed.is_some()), Repr::Raw));
            }
            match parsed {
                Some(value) => Ok((runtime.heap.big(value), Repr::Int)),
                None => Err(Refusal::Fatal(
                    "typed native failures arrive with the native provider port (stage 4)".into(),
                )),
            }
        }
        _ if crate::host::supported(native) => match crate::host::call(runtime, native, arguments) {
            crate::host::Result2::Value(word, repr) => Ok((word, repr)),
            crate::host::Result2::Raise(failure) => Err(Refusal::Raise(failure)),
            crate::host::Result2::Refused(message) => Err(Refusal::Failure(FailureKind::Program(message))),
        },
        _ => Err(Refusal::Fatal(format!(
            "native provider {} is not available in sparrow-core yet",
            native.symbol()
        ))),
    }
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
