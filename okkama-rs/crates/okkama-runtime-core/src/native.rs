//! Shared providers. Native identifiers and signatures live in runtime-abi.
use crate::{
    IntValue, Value, ValueError,
    abi::{Native, NativeFailure},
    heap::ArrayHeap,
};
use std::rc::Rc;
pub mod application;
pub mod buffer;
pub mod collation;
pub mod encoding;
pub mod formats;
pub mod fs;
pub mod host;
pub mod host_crypto;
pub mod idna;
pub mod input;
pub mod jre;
pub mod json;
pub mod markup;
pub mod math;
pub mod misc;
pub mod net;
pub mod network;
pub mod numbers;
pub mod process;
pub mod regex;
pub mod sqlite;
pub mod stdio;
pub mod text;
pub mod time;
pub mod tls;
pub mod unicode;
pub mod value;
pub mod x509;

pub mod inspect;
pub mod support;
pub mod system;

/// Provider state belongs to one execution, not a process-global VM singleton.
#[derive(Debug, Default)]
pub struct Context {
    streams: stdio::State,
    /// Аргументы командной строки программы.
    pub arguments: Vec<String>,
    /// Имена enum программы без модуля и их case (`debug.data_dump`).
    pub enums: Vec<(String, Vec<String>)>,
    /// Имена объектов программы и их полей по порядку (`debug/dumper`).
    pub objects: Vec<(String, Vec<String>)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidState(String),
    /// Provider ещё не перенесён в этот рантайм.
    Unsupported(String),
    Trap(String),
    Failure(NativeFailure),
}
impl From<ValueError> for Error {
    fn from(error: ValueError) -> Self {
        Self::InvalidState(error.message)
    }
}

pub fn execute(
    native: Native,
    args: &[Value],
    arrays: &mut ArrayHeap,
    context: &mut Context,
) -> Result<Value, Error> {
    if args.len() != native.parameters().len() {
        return Err(Error::InvalidState(
            "native argument count does not match ABI".into(),
        ));
    }
    // Providers по модулю символа (`runtime/regex@1::…` → `runtime/regex`).
    let module = native.symbol().split('@').next().unwrap_or("");
    match module {
        "runtime/regex" => return regex::execute(native, args),
        "runtime/crypto" | "runtime/secret" | "runtime/debug" | "runtime/diff"
        | "runtime/math_aggregate" | "runtime/url_text" | "runtime/layout" => {
            return misc::execute(native, args, context);
        }
        "runtime/idna" | "runtime/public_suffix" => return idna::execute(native, args),
        "runtime/json" => return json::execute(native, args),
        "runtime/toml" | "runtime/html" => return markup::execute(native, args),
        "runtime/format" | "runtime/statistics" | "runtime/random_distribution" => {
            return math::execute(native, args, context);
        }
        "runtime/random" => {
            let name = native.symbol().split("::").nth(1).unwrap_or("");
            return if name.starts_with("pcg_") {
                math::execute(native, args, context)
            } else {
                host::execute(native, args)
            };
        }
        "runtime/time" | "runtime/system" => return host::execute(native, args),
        "runtime/uuid" | "runtime/unicode_normalization" => return value::execute(native, args),
        "runtime/unicode_character" => return unicode::execute(native, args),
        "runtime/net" => return net::execute(native, args),
        "runtime/network" => return network::execute(native, args),
        "runtime/x509" => return x509::execute(native, args),
        "runtime/tls_transport" => return tls::execute(native, args),
        "runtime/database" => return sqlite::execute(native, args),
        "runtime/application" | "runtime/account" => return application::execute(native, args),
        "runtime/collation" | "runtime/locale" => return collation::execute(native, args),
        "runtime/fs" => return fs::execute(native, args),
        "runtime/fs_root" | "runtime/snapshot" | "runtime/environment" => {
            return system::execute(native, args);
        }
        "runtime/input_state" => return input::execute(native, args, context),
        "runtime/process" if native.symbol().contains("::process_arguments/") => {
            return input::execute(native, args, context);
        }
        "runtime/process" => return process::execute(native, args),
        "runtime/time_format" | "runtime/time_zone" => return time::execute(native, args),
        "runtime/io_decorators" | "runtime/testing" | "runtime/csv_state" | "runtime/pem"
        | "runtime/template" | "runtime/ini" => return formats::execute(native, args),
        "runtime/binary" | "runtime/hpack" | "runtime/collections" => {
            return text::execute(native, args);
        }
        _ => {}
    }
    if matches!(
        native,
        Native::HostCrypto | Native::DigestCreate | Native::DigestUpdate | Native::DigestBytes
    ) {
        return host_crypto::execute(native, args, arrays);
    }
    if encoding::handles(native) {
        return encoding::execute(native, args, arrays);
    }
    if numbers::handles(native) {
        return numbers::execute(native, args);
    }
    if buffer::handles(native) {
        return buffer::execute(native, args, arrays);
    }
    if matches!(
        native,
        Native::StdoutWrite | Native::StderrWrite | Native::StdinRead | Native::StdinReadLine
    ) {
        return context.streams.execute(
            native,
            args,
            arrays,
            &mut std::io::stdin().lock(),
            &mut std::io::stdout().lock(),
            &mut std::io::stderr().lock(),
        );
    }
    let width = match native {
        Native::Utf8Encode | Native::Utf8Decode | Native::Utf8ValidPrefix => 1,
        Native::Utf16Encode | Native::Utf16Decode | Native::Utf16ValidPrefix => 2,
        Native::Utf32Encode | Native::Utf32Decode | Native::Utf32ValidPrefix => 4,
        _ => {
            return Err(Error::Unsupported(format!(
                "native provider is not implemented: {}",
                native.symbol()
            )));
        }
    };
    let big = if width == 1 {
        true
    } else {
        match args[1].as_string()? {
            "big" => true,
            "little" => false,
            _ => {
                return Err(Error::Trap(format!(
                    "Native invocation \"{}\" failed",
                    native.symbol()
                )));
            }
        }
    };
    if matches!(
        native,
        Native::Utf8Encode | Native::Utf16Encode | Native::Utf32Encode
    ) {
        let text = args[0].as_string()?;
        let bytes = encode(text, width, big);
        return Ok(arrays.allocate(bytes.into_iter().map(Value::Byte).collect()));
    }
    let bytes = arrays
        .elements(&args[0])?
        .iter()
        .map(Value::as_byte)
        .collect::<Result<Vec<_>, _>>()?;
    let (text, prefix) = decode_prefix(&bytes, width, big);
    if matches!(
        native,
        Native::Utf8ValidPrefix | Native::Utf16ValidPrefix | Native::Utf32ValidPrefix
    ) {
        return Ok(Value::Int(IntValue::from_u128(prefix as u128)));
    }
    if prefix != bytes.len() {
        let encoding = match (width, big) {
            (1, _) => "UTF-8",
            (2, true) => "UTF-16BE",
            (2, false) => "UTF-16LE",
            (4, true) => "UTF-32BE",
            _ => "UTF-32LE",
        };
        return Err(Error::Failure(NativeFailure {
            domain: "core.utf".into(),
            code: "invalid_data".into(),
            operation: native.symbol().into(),
            message: format!("Bytes are not valid {encoding}"),
        }));
    }
    Ok(Value::Str(Rc::from(text)))
}

fn encode(text: &str, width: usize, big: bool) -> Vec<u8> {
    if width == 1 {
        return text.as_bytes().to_vec();
    }
    let mut out = Vec::new();
    if width == 2 {
        for unit in text.encode_utf16() {
            out.extend(if big {
                unit.to_be_bytes()
            } else {
                unit.to_le_bytes()
            });
        }
    } else {
        for value in text.chars() {
            let unit = value as u32;
            out.extend(if big {
                unit.to_be_bytes()
            } else {
                unit.to_le_bytes()
            });
        }
    }
    out
}

/// Return the first malformed scalar's starting BYTE offset, never replacement text.
fn decode_prefix(bytes: &[u8], width: usize, big: bool) -> (String, usize) {
    if width == 1 {
        let prefix =
            std::str::from_utf8(bytes).map_or_else(|error| error.valid_up_to(), |_| bytes.len());
        return (
            std::str::from_utf8(&bytes[..prefix])
                .expect("validated UTF-8 prefix")
                .into(),
            prefix,
        );
    }
    let unit = |offset: usize| -> Option<u32> {
        let data = bytes.get(offset..offset.checked_add(width)?)?;
        let mut value = 0u32;
        for index in 0..width {
            value = (value << 8) | u32::from(data[if big { index } else { width - 1 - index }]);
        }
        Some(value)
    };
    let mut text = String::new();
    let mut offset = 0;
    while let Some(mut scalar) = unit(offset) {
        let mut length = width;
        if width == 2 && (0xd800..=0xdbff).contains(&scalar) {
            let Some(low) = unit(offset + 2).filter(|low| (0xdc00..=0xdfff).contains(low)) else {
                break;
            };
            scalar = 0x10000 + ((scalar - 0xd800) << 10) + low - 0xdc00;
            length = 4;
        }
        let Some(value) = char::from_u32(scalar) else {
            break;
        };
        text.push(value);
        offset += length;
    }
    (text, offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_boundaries_and_bom_roundtrip_without_normalization() {
        let text = "\0\u{7f}\u{80}\u{7ff}\u{800}\u{d7ff}\u{e000}\u{feff}\u{ffff}\u{10000}\u{10ffff}e\u{301}";
        for width in [1, 2, 4] {
            for big in [false, true] {
                let bytes = encode(text, width, big);
                assert_eq!(
                    decode_prefix(&bytes, width, big),
                    (text.into(), bytes.len())
                );
            }
        }
        assert_eq!(encode("A🙂", 2, true), [0, 65, 0xd8, 0x3d, 0xde, 0x42]);
        assert_eq!(encode("A🙂", 4, false), [65, 0, 0, 0, 0x42, 0xf6, 1, 0]);
    }

    #[test]
    fn malformed_prefix_starts_at_the_whole_scalar() {
        for bad in [
            &[0xc0, 0x80][..],
            &[0xed, 0xa0, 0x80],
            &[0xf4, 0x90, 0x80, 0x80],
            &[0xf0, 0x90],
            &[0x80],
        ] {
            let mut bytes = vec![65];
            bytes.extend_from_slice(bad);
            assert_eq!(decode_prefix(&bytes, 1, true), ("A".into(), 1));
        }
        for bad in [
            &[0xd8, 0x00][..],
            &[0xdc, 0x00],
            &[0xd8, 0x00, 0, 65],
            &[0xd8, 0x00, 0xdc],
            &[0],
        ] {
            let mut bytes = vec![0, 65];
            bytes.extend_from_slice(bad);
            assert_eq!(decode_prefix(&bytes, 2, true), ("A".into(), 2));
        }
        for bad in [
            &[0, 0x11, 0, 0][..],
            &[0, 0, 0xd8, 0],
            &[0, 0, 0],
            &[0xff, 0xff, 0xff, 0xff],
        ] {
            let mut bytes = vec![0, 0, 0, 65];
            bytes.extend_from_slice(bad);
            assert_eq!(decode_prefix(&bytes, 4, true), ("A".into(), 4));
        }
    }
}
