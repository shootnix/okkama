//! One machine word per value. The representation is chosen by the static SPB
//! type, so registers, fields and array elements carry no runtime tag:
//!
//! - `int`: `(n << 1) | 1` for 63-bit integers, otherwise a pointer to a BigInt
//!   object (pointers are 8-aligned, so the low bit tells them apart);
//! - `float`: raw IEEE bits (language floats are always finite);
//! - `bool`, `byte`, `char`: the raw value;
//! - every other type: a pointer to a heap object with a header.
//!
//! Zero is "unbound storage" (a constructor field not assigned yet): no tagged
//! integer and no pointer is zero. Raw scalars use [`UNBOUND_RAW`], a NaN bit
//! pattern for floats and outside every bool/byte/char range.

/// A value word; its meaning comes from a [`Repr`].
pub type Word = u64;

/// Unbound storage of references and integers.
pub const UNBOUND: Word = 0;
/// Unbound storage of raw scalars (a quiet NaN with a payload no finite float
/// or scalar value uses).
pub const UNBOUND_RAW: Word = 0x7ff8_dead_beef_0001;

/// How a word of a given static type is traced and converted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Repr {
    /// Tagged small integer or BigInt pointer.
    Int = 0,
    /// Raw bits: float, bool, byte, char.
    Raw = 1,
    /// Pointer to a heap object (or [`UNBOUND`]).
    Ref = 2,
}

impl Repr {
    pub(crate) fn from_bits(bits: u64) -> Self {
        match bits {
            0 => Self::Int,
            1 => Self::Raw,
            _ => Self::Ref,
        }
    }
    /// The heap object a word of this representation points to, if any.
    #[inline]
    pub fn pointer(self, word: Word) -> Option<Word> {
        match self {
            Self::Int if word & 1 == 0 && word != UNBOUND => Some(word),
            Self::Ref if word != UNBOUND => Some(word),
            _ => None,
        }
    }
}

/// Largest and smallest integers stored inline.
pub const SMALL_MAX: i64 = (1 << 62) - 1;
pub const SMALL_MIN: i64 = -(1 << 62);

/// Inline integer word, if `n` fits in 63 bits.
#[inline]
pub fn small(n: i64) -> Option<Word> {
    (SMALL_MIN..=SMALL_MAX)
        .contains(&n)
        .then_some(((n << 1) | 1) as Word)
}

/// The inline integer of a word, if it is one.
#[inline]
pub fn small_value(word: Word) -> Option<i64> {
    (word & 1 == 1).then_some((word as i64) >> 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_integers_round_trip_and_never_look_like_pointers_or_unbound() {
        for n in [
            0,
            1,
            -1,
            42,
            -42,
            SMALL_MAX,
            SMALL_MIN,
            i32::MAX as i64,
            i32::MIN as i64,
        ] {
            let word = small(n).unwrap();
            assert_eq!(small_value(word), Some(n));
            assert_ne!(word, UNBOUND);
            assert_eq!(Repr::Int.pointer(word), None);
        }
        for n in [SMALL_MAX + 1, SMALL_MIN - 1, i64::MAX, i64::MIN] {
            assert_eq!(small(n), None);
        }
        assert_eq!(Repr::Int.pointer(0x1000), Some(0x1000));
        assert_eq!(Repr::Int.pointer(UNBOUND), None);
        assert_eq!(Repr::Raw.pointer(0x1000), None);
    }

    #[test]
    fn unbound_raw_is_not_a_finite_float_or_scalar() {
        let f = f64::from_bits(UNBOUND_RAW);
        assert!(f.is_nan());
        assert!(UNBOUND_RAW > u64::from(u32::MAX));
    }
}

/// Multiplicative hashing for engine-assigned keys (heap addresses, handle ids)
/// and short strings: SipHash's flooding resistance buys nothing there and it
/// showed up in native-call profiles.
#[derive(Debug, Default, Clone, Copy)]
pub struct FastHasher(u64);
impl std::hash::Hasher for FastHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.write_u64(u64::from_le_bytes(word));
        }
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}
pub type FastBuild = std::hash::BuildHasherDefault<FastHasher>;
pub type FastMap<K, V> = std::collections::HashMap<K, V, FastBuild>;
pub type FastSet<K> = std::collections::HashSet<K, FastBuild>;
