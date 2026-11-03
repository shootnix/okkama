//! Binary integer/float wire formats. Conversion uses Rust byte primitives and
//! num-bigint directly, retaining the public width and signedness contracts.
use crate::{Error, native::Outcome, value::*};
use num_bigint::{BigInt, Sign};
use num_traits::{Signed, ToPrimitive};
use okkama_runtime_abi::{Native, NativeFailure};
pub(crate) fn handles(native: Native) -> bool {
    use Native::*;
    matches!(
        native,
        BinaryBinaryIntegerFits
            | BinaryBinaryIntegerBytes
            | BinaryBinaryIntegerFromBytesFits
            | BinaryBinaryIntegerFromBytes
            | BinaryBinaryFloatFits
            | BinaryBinaryFloatBytes
            | BinaryBinaryFloatFinite
            | BinaryBinaryFloatFromBytes
            | BinaryBinaryMaskedSlice
    )
}
fn failed(native: Native) -> Error {
    Error(format!("Native invocation \"{}\" failed", native.symbol()))
}
fn big(heap: &Heap, value: Slot) -> Result<BigInt, Error> {
    Ok(match heap.integer(value)? {
        Integer::Small(n) => BigInt::from(n),
        Integer::Big(n) => (*n).clone(),
    })
}
fn fits(native: Native, value: &BigInt, size: &BigInt, signedness: &str) -> Result<bool, Error> {
    if size <= &BigInt::from(0) {
        return Ok(false);
    }
    let size = size
        .to_u64()
        .filter(|n| *n <= 1 << 20)
        .ok_or_else(|| failed(native))?;
    let bits = 8 * size;
    let magnitude = value.magnitude();
    match signedness {
        "unsigned" => Ok(!value.is_negative() && magnitude.bits() <= bits),
        "signed" if !value.is_negative() => Ok(magnitude.bits() < bits),
        "signed" => Ok(magnitude.bits() < bits
            || magnitude.bits() == bits && (magnitude - 1u8).bits() == bits - 1),
        _ => Err(failed(native)),
    }
}
fn order(native: Native, bytes: &mut [u8], order: &str) -> Result<(), Error> {
    match order {
        "big" => {}
        "little" => bytes.reverse(),
        _ => return Err(failed(native)),
    };
    Ok(())
}
fn from_bytes(native: Native, bytes: &[u8], signedness: &str) -> Result<BigInt, Error> {
    if bytes.is_empty() {
        return Err(failed(native));
    }
    match signedness {
        "signed" => Ok(BigInt::from_signed_bytes_be(bytes)),
        "unsigned" => Ok(BigInt::from_bytes_be(Sign::Plus, bytes)),
        _ => Err(failed(native)),
    }
}
fn width(heap: &Heap, native: Native, value: Slot) -> Result<u32, Error> {
    match heap.integer(value)?.to_i64() {
        Some(32) => Ok(32),
        Some(64) => Ok(64),
        _ => Err(failed(native)),
    }
}
fn float_bytes(value: f64, width: u32) -> Vec<u8> {
    if width == 32 {
        (value as f32).to_be_bytes().to_vec()
    } else {
        value.to_be_bytes().to_vec()
    }
}
fn float_value(bytes: &[u8], width: u32) -> Option<f64> {
    let value = if width == 32 {
        f32::from_be_bytes(bytes.try_into().ok()?) as f64
    } else {
        f64::from_be_bytes(bytes.try_into().ok()?)
    };
    value.is_finite().then_some(value)
}
impl Heap {
    pub(crate) fn native_binary(
        &mut self,
        native: Native,
        args: &[Slot],
    ) -> Result<Outcome, Error> {
        use Native::*;
        let value = match native {
            BinaryBinaryIntegerFits | BinaryBinaryIntegerBytes => {
                let number = big(self, args[0])?;
                let size = big(self, args[1])?;
                let fits = fits(native, &number, &size, self.text(args[2])?)?;
                if native == BinaryBinaryIntegerFits {
                    Slot::boolean(fits)
                } else {
                    if !fits {
                        return Err(failed(native));
                    }
                    let width = size.to_usize().expect("validated width");
                    let significant = number.to_signed_bytes_be();
                    let mut bytes = vec![if number.is_negative() { 255 } else { 0 }; width];
                    let count = significant.len().min(width);
                    bytes[width - count..]
                        .copy_from_slice(&significant[significant.len() - count..]);
                    order(native, &mut bytes, self.text(args[3])?)?;
                    self.native_byte_array(bytes)?
                }
            }
            BinaryBinaryIntegerFromBytesFits | BinaryBinaryIntegerFromBytes => {
                let mut bytes = self.native_bytes(args[0])?;
                order(native, &mut bytes, self.text(args[2])?)?;
                if native == BinaryBinaryIntegerFromBytesFits && bytes.is_empty() {
                    Slot::boolean(false)
                } else {
                    let number = from_bytes(native, &bytes, self.text(args[1])?)?;
                    let small = number.to_i64();
                    if native == BinaryBinaryIntegerFromBytesFits {
                        Slot::boolean(small.is_some())
                    } else {
                        Slot::small(small.ok_or_else(|| failed(native))?)
                    }
                }
            }
            BinaryBinaryFloatFits | BinaryBinaryFloatBytes => {
                if args[0].tag != FLOAT {
                    return Err(Error("expected float slot".into()));
                }
                let width = width(self, native, args[1])?;
                let mut bytes = float_bytes(f64::from_bits(args[0].bits), width);
                if native == BinaryBinaryFloatFits {
                    Slot::boolean(float_value(&bytes, width).is_some())
                } else {
                    order(native, &mut bytes, self.text(args[2])?)?;
                    self.native_byte_array(bytes)?
                }
            }
            BinaryBinaryFloatFinite | BinaryBinaryFloatFromBytes => {
                let width = width(self, native, args[1])?;
                let mut bytes = self.native_bytes(args[0])?;
                order(native, &mut bytes, self.text(args[2])?)?;
                let value = float_value(&bytes, width);
                if native == BinaryBinaryFloatFinite {
                    Slot::boolean(value.is_some())
                } else {
                    Slot {
                        tag: FLOAT,
                        bits: value.ok_or_else(|| failed(native))?.to_bits(),
                    }
                }
            }
            BinaryBinaryMaskedSlice => {
                let bytes = self.native_bytes(args[0])?;
                let mask = self.native_bytes(args[1])?;
                let phase = self
                    .integer(args[2])?
                    .to_i64()
                    .filter(|n| (0..=3).contains(n));
                let begin = self
                    .integer(args[3])?
                    .to_i64()
                    .filter(|n| *n >= 0 && *n as usize <= bytes.len());
                let count = self
                    .integer(args[4])?
                    .to_i64()
                    .filter(|n| (0..=65536).contains(n));
                if let (Some(phase), Some(begin), Some(count)) = (phase, begin, count) {
                    if mask.len() == 4 && count as usize <= bytes.len() - begin as usize {
                        let rotation = (phase as usize + begin as usize) % 4;
                        let result = (0..count as usize)
                            .map(|i| bytes[begin as usize + i] ^ mask[(rotation + i) % 4])
                            .collect();
                        return self.native_byte_array(result).map(Outcome::Value);
                    }
                }
                let payload = self.native_failure_new(NativeFailure {
                    domain: "core.binary".into(),
                    code: "invalid_input".into(),
                    operation: native.symbol().into(),
                    message: "invalid bounded byte mask coordinates".into(),
                })?;
                return self
                    .failure_new(crate::failure::FailureKind::Native(payload))
                    .map(Outcome::Failure);
            }
            _ => return Err(Error("invalid binary provider".into())),
        };
        Ok(Outcome::Value(value))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn binary_signed_boundaries_and_finite_float_bits_are_exact() {
        let n = Native::BinaryBinaryIntegerFits;
        for (value, signed, expected) in [
            (-129, "signed", false),
            (-128, "signed", true),
            (127, "signed", true),
            (128, "signed", false),
            (255, "unsigned", true),
            (256, "unsigned", false),
            (-1, "unsigned", false),
        ] {
            assert_eq!(
                fits(n, &BigInt::from(value), &BigInt::from(1), signed).unwrap(),
                expected
            );
        }
        assert_eq!(
            from_bytes(n, &[0x80], "signed").unwrap(),
            BigInt::from(-128)
        );
        assert_eq!(
            from_bytes(n, &[0x80], "unsigned").unwrap(),
            BigInt::from(128)
        );
        for width in [32, 64] {
            assert_eq!(
                float_value(&float_bytes(-0.0, width), width)
                    .unwrap()
                    .to_bits(),
                (-0.0f64).to_bits()
            );
            assert!(float_value(&float_bytes(f64::INFINITY, width), width).is_none());
            assert!(float_value(&[], width).is_none());
        }
        let mut heap = Heap::new(&[], &[]);
        let signed = heap.allocate_text("signed".into()).unwrap();
        let little = heap.allocate_text("little".into()).unwrap();
        let Outcome::Value(bytes) = heap
            .native_binary(
                Native::BinaryBinaryIntegerBytes,
                &[Slot::small(-129), Slot::small(3), signed, little],
            )
            .unwrap()
        else {
            panic!("bytes")
        };
        assert_eq!(heap.native_bytes(bytes).unwrap(), [0x7f, 0xff, 0xff]);
        let Outcome::Value(number) = heap
            .native_binary(
                Native::BinaryBinaryIntegerFromBytes,
                &[bytes, signed, little],
            )
            .unwrap()
        else {
            panic!("integer")
        };
        assert_eq!(heap.integer(number).unwrap(), -129);
    }
}
