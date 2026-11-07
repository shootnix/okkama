//! `runtime/bytes`: views of a byte range (`encoding/bytes`), the same
//! operations as the Sparrow core's; out-of-range arguments are refused.
use super::support::{byte_array, bytes, failed, int, text};
use super::Error;
use crate::{IntValue, Value, abi::Native};

fn index(value: &Value) -> Result<Option<usize>, Error> {
    Ok(int(value)?.to_i64().and_then(|n| usize::try_from(n).ok()))
}

fn integer(n: i64) -> Value {
    Value::Int(IntValue::from_i64(n))
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    let source = bytes(&args[0])?;
    let length = source.len();
    match native.name() {
        "bytes_find" => {
            let start = index(&args[2])?.filter(|&s| s <= length).ok_or_else(|| failed(native))?;
            let needle = bytes(&args[1])?;
            if needle.is_empty() {
                return Err(failed(native));
            }
            let found = source[start..].windows(needle.len()).position(|w| w == needle.as_slice());
            return Ok(integer(found.map_or(-1, |at| (start + at) as i64)));
        }
        "bytes_join" => {
            let start = index(&args[1])?.filter(|&s| s <= length).ok_or_else(|| failed(native))?;
            let mut joined = source[start..].to_vec();
            joined.extend(bytes(&args[2])?);
            return Ok(byte_array(joined));
        }
        _ => {}
    }
    let (Some(start), Some(end)) = (index(&args[1])?, index(&args[2])?) else {
        return Err(failed(native));
    };
    if start > end || end > length {
        return Err(failed(native));
    }
    let range = &source[start..end];
    match native.name() {
        "bytes_utf8_invalid" => Ok(integer(std::str::from_utf8(range).err().map_or(-1, |e| e.valid_up_to() as i64))),
        "bytes_text" => std::str::from_utf8(range).map(text).map_err(|_| failed(native)),
        "bytes_decimal_invalid" | "bytes_decimal" => {
            // ASCII decimal, optional leading '-': the offset of the first byte
            // that breaks it, or -1.
            let sign = usize::from(range.first() == Some(&b'-'));
            let invalid = match range[sign..].iter().position(|b| !b.is_ascii_digit()) {
                Some(at) => (sign + at) as i64,
                None if range.len() == sign => sign as i64,
                None => -1,
            };
            if native.name() == "bytes_decimal_invalid" {
                return Ok(integer(invalid));
            }
            if invalid >= 0 {
                return Err(failed(native));
            }
            // Leading zeros and "-0" are accepted, as on the core.
            let n = num_bigint::BigInt::parse_bytes(range, 10).ok_or_else(|| failed(native))?;
            match i64::try_from(&n) {
                Ok(small) => Ok(integer(small)),
                Err(_) => IntValue::parse(&n.to_string()).map(Value::Int).ok_or_else(|| failed(native)),
            }
        }
        "bytes_equal" => Ok(Value::Bool(range == bytes(&args[3])?.as_slice())),
        "bytes_integer_fits" | "bytes_integer" => {
            // Fixed-width integer, two's complement or magnitude, as
            // `encoding/binary` decodes it; it must fit the signed 64-bit int.
            let (signed, big) = (super::support::boolean(&args[3])?, super::support::boolean(&args[4])?);
            let value = (!range.is_empty())
                .then(|| {
                    let n = match (signed, big) {
                        (true, true) => num_bigint::BigInt::from_signed_bytes_be(range),
                        (true, false) => num_bigint::BigInt::from_signed_bytes_le(range),
                        (false, true) => num_bigint::BigInt::from_bytes_be(num_bigint::Sign::Plus, range),
                        (false, false) => num_bigint::BigInt::from_bytes_le(num_bigint::Sign::Plus, range),
                    };
                    i64::try_from(n).ok()
                })
                .flatten();
            if native.name() == "bytes_integer_fits" {
                return Ok(Value::Bool(value.is_some()));
            }
            value.map(integer).ok_or_else(|| failed(native))
        }
        _ => Err(Error::Unsupported(format!("native provider is not implemented: {}", native.symbol()))),
    }
}
