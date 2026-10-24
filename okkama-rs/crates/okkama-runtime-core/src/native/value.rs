//! `runtime/uuid` и `runtime/unicode_normalization` — перенос части
//! `ValueNatives` JVM (`runtime/decimal` — в `numbers`).
use super::Error;
use super::support::{byte_array, bytes, failed, failure, int, int_value, string, text};
use crate::{Value, abi::Native};
use unicode_normalization::UnicodeNormalization;

fn invalid(native: Native, message: &str) -> Error {
    failure("core.uuid", "invalid_input", native, message)
}

fn canonical(origin: &str) -> bool {
    let bytes = origin.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, b)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                *b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}

fn sixteen(native: Native, value: &Value) -> Result<Vec<u8>, Error> {
    let octets = bytes(value)?;
    if octets.len() != 16 {
        return Err(invalid(native, "UUID binary value must contain sixteen bytes"));
    }
    Ok(octets)
}

fn versioned(mut octets: Vec<u8>, version: u8) -> Value {
    octets[6] = (octets[6] & 0x0f) | (version << 4);
    octets[8] = (octets[8] & 0x3f) | 0x80;
    byte_array(octets)
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    let name = native
        .symbol()
        .split("::")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("");
    match name {
        "normalized_text" => {
            let origin = string(&args[0])?;
            Ok(text(match string(&args[1])? {
                "NFC" => origin.nfc().collect::<String>(),
                "NFD" => origin.nfd().collect(),
                "NFKC" => origin.nfkc().collect(),
                "NFKD" => origin.nfkd().collect(),
                _ => return Err(failed(native)),
            }))
        }
        "uuid_valid" => Ok(Value::Bool(canonical(string(&args[0])?))),
        "uuid_bytes" => {
            let origin = string(&args[0])?;
            if !canonical(origin) {
                return Err(invalid(native, "UUID text is not canonical"));
            }
            let hex: String = origin.chars().filter(|c| *c != '-').collect();
            Ok(byte_array((0..16).map(|index| {
                u8::from_str_radix(&hex[2 * index..2 * index + 2], 16).expect("hex digits")
            })))
        }
        "uuid_string" => {
            let octets = sixteen(native, &args[0])?;
            let hex: String = octets.iter().map(|b| format!("{b:02x}")).collect();
            Ok(text(format!(
                "{}-{}-{}-{}-{}",
                &hex[..8],
                &hex[8..12],
                &hex[12..16],
                &hex[16..20],
                &hex[20..32]
            )))
        }
        "uuid_version" => Ok(int_value(i64::from(sixteen(native, &args[0])?[6] >> 4))),
        "uuid_variant" => {
            let value = sixteen(native, &args[0])?[8];
            Ok(text(if value & 0x80 == 0 {
                "reserved"
            } else if value & 0xc0 == 0x80 {
                "rfc"
            } else if value & 0xe0 == 0xc0 {
                "microsoft"
            } else {
                "future"
            }))
        }
        "uuid_versioned" => {
            let octets = sixteen(native, &args[0])?;
            let version = int(&args[1])?
                .to_i64()
                .filter(|version| (0..=15).contains(version))
                .ok_or_else(|| invalid(native, "UUID version must be between zero and fifteen"))?;
            Ok(versioned(octets, version as u8))
        }
        "uuid_named" => {
            let namespace = bytes(&args[0])?;
            if namespace.len() != 16 {
                return Err(invalid(native, "UUID namespace must contain sixteen bytes"));
            }
            let data = bytes(&args[1])?;
            let version = int(&args[2])?.to_i64();
            let hash: Vec<u8> = match version {
                Some(3) => {
                    use md5::Digest;
                    let mut digest = md5::Md5::new();
                    digest.update(&namespace);
                    digest.update(&data);
                    digest.finalize().to_vec()
                }
                Some(5) => {
                    use sha1::Digest;
                    let mut digest = sha1::Sha1::new();
                    digest.update(&namespace);
                    digest.update(&data);
                    digest.finalize().to_vec()
                }
                _ => return Err(invalid(native, "Named UUID version must be three or five")),
            };
            Ok(versioned(hash[..16].to_vec(), version.expect("checked") as u8))
        }
        "uuid_custom" => {
            let (high, middle, low) = (int(&args[0])?, int(&args[1])?, int(&args[2])?);
            let high = high
                .to_i64()
                .filter(|value| (0..=281_474_976_710_655).contains(value))
                .ok_or_else(|| invalid(native, "UUID 8 high field is outside 48 bits"))?;
            let middle = middle
                .to_i64()
                .filter(|value| (0..=4095).contains(value))
                .ok_or_else(|| invalid(native, "UUID 8 middle field is outside 12 bits"))?;
            let low = low
                .to_i64()
                .filter(|value| (0..=4_611_686_018_427_387_903).contains(value))
                .ok_or_else(|| invalid(native, "UUID 8 low field is outside 62 bits"))?;
            let mut out = [0u8; 16];
            for (index, octet) in out.iter_mut().take(6).enumerate() {
                *octet = (high >> (8 * (5 - index))) as u8;
            }
            out[6] = 0x80 | ((middle >> 8) & 0x0f) as u8;
            out[7] = middle as u8;
            for index in 0..8 {
                out[8 + index] = (low >> (8 * (7 - index))) as u8;
            }
            out[8] = (out[8] & 0x3f) | 0x80;
            Ok(byte_array(out))
        }
        _ => Err(Error::Unsupported(format!(
            "native provider is not implemented: {}",
            native.symbol()
        ))),
    }
}
