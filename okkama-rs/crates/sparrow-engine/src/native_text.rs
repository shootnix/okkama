//! Pure codecs and exact numeric parsing at the independent native boundary.
use crate::{Error, native::Outcome, value::*};
use num_bigint::BigInt;
use okkama_runtime_abi::{Native, NativeFailure};

pub(crate) fn handles(native: Native) -> bool {
    use Native::*;
    matches!(
        native,
        HexEncode
            | HexDecode
            | HexValidPrefix
            | Utf8Encode
            | Utf8Decode
            | Utf8ValidPrefix
            | Utf16Encode
            | Utf16Decode
            | Utf16ValidPrefix
            | Utf32Encode
            | Utf32Decode
            | Utf32ValidPrefix
            | AsciiRepresentablePrefix
            | AsciiEncode
            | AsciiValidPrefix
            | AsciiDecode
            | Latin1RepresentablePrefix
            | Latin1Encode
            | Latin1Decode
            | NumberTextIntParse
            | NumberTextIntParseValid
    )
}
fn failed(native: Native) -> Error {
    Error(format!("Native invocation \"{}\" failed", native.symbol()))
}
impl Heap {
    pub(crate) fn native_bytes(&self, value: Slot) -> Result<Vec<u8>, Error> {
        self.elements(value)?
            .iter()
            .map(|value| {
                if value.tag == BYTE {
                    Ok(value.bits as u8)
                } else {
                    Err(Error("expected byte element".into()))
                }
            })
            .collect()
    }
    pub(crate) fn native_byte_array(&mut self, bytes: Vec<u8>) -> Result<Slot, Error> {
        self.allocate_array(
            bytes
                .into_iter()
                .map(|byte| Slot {
                    tag: BYTE,
                    bits: byte as u64,
                })
                .collect(),
        )
    }
    pub(crate) fn native_text(&mut self, native: Native, args: &[Slot]) -> Result<Outcome, Error> {
        use Native::*;
        match native {
            HexEncode => {
                const DIGITS: &[u8; 16] = b"0123456789abcdef";
                let bytes = self.native_bytes(args[0])?;
                let mut text = String::with_capacity(bytes.len().saturating_mul(2));
                for byte in bytes {
                    text.push(DIGITS[(byte >> 4) as usize] as char);
                    text.push(DIGITS[(byte & 15) as usize] as char);
                }
                return self.allocate_text(text).map(Outcome::Value);
            }
            HexValidPrefix => {
                return Ok(Outcome::Value(Slot::small(
                    self.text(args[0])?
                        .chars()
                        .take_while(|c| c.is_ascii_hexdigit())
                        .count() as i64,
                )));
            }
            HexDecode => {
                let text = self.text(args[0])?;
                if text.len() & 1 != 0 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(failed(native));
                }
                let bytes = text
                    .as_bytes()
                    .chunks_exact(2)
                    .map(|p| {
                        ((p[0] as char).to_digit(16).unwrap() * 16
                            + (p[1] as char).to_digit(16).unwrap()) as u8
                    })
                    .collect();
                return self.native_byte_array(bytes).map(Outcome::Value);
            }
            NumberTextIntParse | NumberTextIntParseValid => {
                let parsed = self
                    .integer(args[1])?
                    .to_i64()
                    .and_then(|radix| int_parse(self.text(args[0]).ok()?, radix));
                if native == NumberTextIntParseValid {
                    return Ok(Outcome::Value(Slot::boolean(parsed.is_some())));
                }
                return self
                    .allocate(parsed.ok_or_else(|| failed(native))?)
                    .map(Outcome::Value);
            }
            AsciiRepresentablePrefix | Latin1RepresentablePrefix => {
                let max = if native == AsciiRepresentablePrefix {
                    0x7f
                } else {
                    0xff
                };
                return Ok(Outcome::Value(Slot::small(
                    self.text(args[0])?
                        .chars()
                        .take_while(|c| *c as u32 <= max)
                        .count() as i64,
                )));
            }
            AsciiEncode | Latin1Encode => {
                let bytes = self
                    .text(args[0])?
                    .chars()
                    .map(|c| c as u32 as u8)
                    .collect();
                return self.native_byte_array(bytes).map(Outcome::Value);
            }
            AsciiDecode | Latin1Decode => {
                let text = self
                    .native_bytes(args[0])?
                    .into_iter()
                    .map(char::from)
                    .collect();
                return self.allocate_text(text).map(Outcome::Value);
            }
            AsciiValidPrefix => {
                return Ok(Outcome::Value(Slot::small(
                    self.native_bytes(args[0])?
                        .iter()
                        .take_while(|b| **b < 0x80)
                        .count() as i64,
                )));
            }
            _ => {}
        }
        let width = match native {
            Utf8Encode | Utf8Decode | Utf8ValidPrefix => 1,
            Utf16Encode | Utf16Decode | Utf16ValidPrefix => 2,
            Utf32Encode | Utf32Decode | Utf32ValidPrefix => 4,
            _ => return Err(Error("invalid text provider".into())),
        };
        let big = if width == 1 {
            true
        } else {
            match self.text(args[1])? {
                "big" => true,
                "little" => false,
                _ => return Err(failed(native)),
            }
        };
        if matches!(native, Utf8Encode | Utf16Encode | Utf32Encode) {
            let bytes = encode(self.text(args[0])?, width, big);
            return self.native_byte_array(bytes).map(Outcome::Value);
        }
        let bytes = self.native_bytes(args[0])?;
        let (text, prefix) = decode_prefix(&bytes, width, big);
        if matches!(
            native,
            Utf8ValidPrefix | Utf16ValidPrefix | Utf32ValidPrefix
        ) {
            return Ok(Outcome::Value(Slot::small(prefix as i64)));
        }
        if prefix != bytes.len() {
            let encoding = match (width, big) {
                (1, _) => "UTF-8",
                (2, true) => "UTF-16BE",
                (2, false) => "UTF-16LE",
                (4, true) => "UTF-32BE",
                _ => "UTF-32LE",
            };
            let payload = self.native_failure_new(NativeFailure {
                domain: "core.utf".into(),
                code: "invalid_data".into(),
                operation: native.symbol().into(),
                message: format!("Bytes are not valid {encoding}"),
            })?;
            return self
                .failure_new(crate::failure::FailureKind::Native(payload))
                .map(Outcome::Failure);
        }
        self.allocate_text(text).map(Outcome::Value)
    }
}
fn int_parse(text: &str, radix: i64) -> Option<BigInt> {
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
    if digits.is_empty()
        || !digits
            .bytes()
            .all(|b| (b as char).to_digit(36).is_some_and(|d| d < base))
    {
        return None;
    }
    let value = BigInt::parse_bytes(digits.as_bytes(), base)?;
    Some(if negative { -value } else { value })
}

pub(crate) fn encode(text: &str, width: usize, big: bool) -> Vec<u8> {
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
pub(crate) fn decode_prefix(bytes: &[u8], width: usize, big: bool) -> (String, usize) {
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
    fn number_parsing_preserves_radices_signs_and_arbitrary_precision() {
        for (text, radix, expected) in [
            (
                "-0xFFFFFFFFFFFFFFFFFFFFFFFF",
                0,
                Some("-79228162514264337593543950335"),
            ),
            ("0B101", 0, Some("5")),
            ("0o77", 0, Some("63")),
            ("Z", 36, Some("35")),
            ("-000", 10, Some("0")),
            ("0xff", 16, None),
            ("+1", 10, None),
            ("1_0", 10, None),
            (" 1", 10, None),
            ("١", 10, None),
            ("", 10, None),
            ("-", 10, None),
            ("0x", 0, None),
            ("2", 2, None),
            ("1", 37, None),
        ] {
            assert_eq!(
                int_parse(text, radix).map(|n| n.to_string()).as_deref(),
                expected,
                "{text} radix {radix}"
            );
        }
    }
    #[test]
    fn native_utf_reports_typed_failure_and_single_byte_uses_scalar_offsets() {
        let mut heap = Heap::new(&[], &[]);
        let invalid = heap.native_byte_array(vec![65, 0xed, 0xa0, 0x80]).unwrap();
        let Outcome::Value(prefix) = heap
            .native_invoke(Native::Utf8ValidPrefix, &[invalid])
            .unwrap()
        else {
            panic!("prefix")
        };
        assert_eq!(prefix.bits, 1);
        let Outcome::Failure(failure) = heap.native_invoke(Native::Utf8Decode, &[invalid]).unwrap()
        else {
            panic!("failure")
        };
        let crate::failure::FailureKind::Native(payload) = heap.failure(failure).unwrap().kind
        else {
            panic!("typed failure")
        };
        let record = heap.native_failure(payload).unwrap();
        assert_eq!(record.domain, "core.utf");
        assert_eq!(record.code, "invalid_data");
        assert_eq!(record.message, "Bytes are not valid UTF-8");
        let source = heap.allocate_text("éÿĀ😀".into()).unwrap();
        let Outcome::Value(prefix) = heap
            .native_invoke(Native::Latin1RepresentablePrefix, &[source])
            .unwrap()
        else {
            panic!("prefix")
        };
        assert_eq!(prefix.bits, 2);
        let Outcome::Value(bytes) = heap.native_invoke(Native::Latin1Encode, &[source]).unwrap()
        else {
            panic!("bytes")
        };
        assert_eq!(heap.native_bytes(bytes).unwrap(), [233, 255, 0, 0]);
    }
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
