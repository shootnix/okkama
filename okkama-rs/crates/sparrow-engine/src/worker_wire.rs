//! OKW1 transfer values. Traversal uses explicit work buffers, never Rust recursion.
use crate::{Error, value::*};

pub(crate) fn text(out: &mut Vec<u8>, value: &str) -> Result<(), Error> {
    count(out, value.len())?;
    out.extend_from_slice(value.as_bytes());
    Ok(())
}
pub(crate) fn count(out: &mut Vec<u8>, value: usize) -> Result<(), Error> {
    out.extend_from_slice(
        &u32::try_from(value)
            .map_err(|_| Error("spawn value exceeds wire length".into()))?
            .to_be_bytes(),
    );
    Ok(())
}
pub(crate) fn encode(heap: &Heap, out: &mut Vec<u8>, value: Slot) -> Result<(), Error> {
    let mut work = vec![value];
    while let Some(value) = work.pop() {
        match value.tag {
            UNBOUND => out.push(0),
            SMALL | BIG => {
                out.push(1);
                text(out, &heap.integer(value)?.to_string())?;
            }
            FLOAT => {
                out.push(2);
                out.extend_from_slice(&value.bits.to_be_bytes());
            }
            STR => {
                out.push(3);
                text(out, heap.text(value)?)?;
            }
            CHAR => {
                out.push(4);
                out.extend_from_slice(&(value.bits as u32).to_be_bytes());
            }
            BYTE => {
                out.push(5);
                out.push(value.bits as u8);
            }
            BOOL => {
                out.push(6);
                out.push(u8::from(value.bits != 0));
            }
            ARRAY => {
                let values = heap.elements(value)?;
                out.push(7);
                count(out, values.len())?;
                work.extend(values.iter().rev().copied());
            }
            MAP => {
                let values = heap.maps.get(value.bits)?;
                out.push(8);
                count(out, values.len())?;
                for &(key, value) in values.values().rev() {
                    work.push(value);
                    work.push(key);
                }
            }
            _ => {
                return Err(Error(
                    "spawn cannot transfer objects, closures, Futures or native handles".into(),
                ));
            }
        }
    }
    Ok(())
}

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    pub fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self
            .at
            .checked_add(count)
            .filter(|&end| end <= self.bytes.len())
            .ok_or_else(|| Error("truncated spawn value".into()))?;
        let value = &self.bytes[self.at..end];
        self.at = end;
        Ok(value)
    }
    pub fn count(&mut self) -> Result<usize, Error> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().expect("four bytes")) as usize)
    }
    pub fn text(&mut self) -> Result<&'a str, Error> {
        let count = self.count()?;
        std::str::from_utf8(self.take(count)?).map_err(|_| Error("invalid spawn text".into()))
    }
    pub fn finish(&self) -> Result<(), Error> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(Error("trailing spawn data".into()))
        }
    }
    pub fn value(&mut self, heap: &mut Heap) -> Result<Slot, Error> {
        struct Frame {
            map: bool,
            length: usize,
            values: Vec<Slot>,
        }
        let mut stack: Vec<Frame> = Vec::new();
        let mut outstanding = 1usize;
        loop {
            let tag = self.take(1)?[0];
            outstanding -= 1;
            let mut value = match tag {
                0 => Slot {
                    bits: 0,
                    tag: UNBOUND,
                },
                1 => {
                    let text = self.text()?;
                    let digits = text.strip_prefix('-').unwrap_or(text);
                    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                        return Err(Error("invalid spawn integer".into()));
                    }
                    heap.allocate(
                        num_bigint::BigInt::parse_bytes(text.as_bytes(), 10)
                            .ok_or_else(|| Error("invalid spawn integer".into()))?,
                    )?
                }
                2 => {
                    let bits = u64::from_be_bytes(self.take(8)?.try_into().expect("eight bytes"));
                    if !f64::from_bits(bits).is_finite() {
                        return Err(Error("invalid spawn float".into()));
                    }
                    Slot { bits, tag: FLOAT }
                }
                3 => heap.allocate_text(self.text()?.into())?,
                4 => {
                    let bits = self.count()? as u32;
                    if char::from_u32(bits).is_none() {
                        return Err(Error("invalid spawn character".into()));
                    }
                    Slot {
                        bits: u64::from(bits),
                        tag: CHAR,
                    }
                }
                5 => Slot {
                    bits: u64::from(self.take(1)?[0]),
                    tag: BYTE,
                },
                6 => {
                    let value = self.take(1)?[0];
                    if value > 1 {
                        return Err(Error("invalid spawn boolean".into()));
                    }
                    Slot::boolean(value != 0)
                }
                7 | 8 => {
                    let map = tag == 8;
                    let count = self.count()?;
                    let length = count
                        .checked_mul(if map { 2 } else { 1 })
                        .filter(|&n| n <= self.bytes.len() - self.at)
                        .ok_or_else(|| Error("truncated spawn container".into()))?;
                    outstanding = outstanding
                        .checked_add(length)
                        .filter(|&n| n <= self.bytes.len() - self.at)
                        .ok_or_else(|| Error("truncated spawn container".into()))?;
                    if length > 0 {
                        stack.push(Frame {
                            map,
                            length,
                            // Grow only for decoded values, not untrusted lengths
                            // repeated at every nesting level.
                            values: Vec::new(),
                        });
                        continue;
                    }
                    if map {
                        heap.map_new(std::iter::empty())?
                    } else {
                        heap.allocate_array(vec![])?
                    }
                }
                _ => return Err(Error(format!("invalid spawn value tag {tag}"))),
            };
            if outstanding > self.bytes.len() - self.at {
                return Err(Error("truncated spawn container".into()));
            }
            loop {
                let Some(frame) = stack.last_mut() else {
                    return Ok(value);
                };
                frame.values.push(value);
                if frame.values.len() < frame.length {
                    break;
                }
                let frame = stack.pop().expect("completed frame");
                value = if frame.map {
                    heap.map_new(frame.values.chunks_exact(2).map(|pair| (pair[0], pair[1])))?
                } else {
                    heap.allocate_array(frame.values)?
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_lengths_account_for_unread_ancestor_siblings() {
        let bytes = [7, 0, 0, 0, 6, 7, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0];
        let mut reader = Reader::new(&bytes);
        let mut heap = Heap::new(&[], &[]);
        assert!(reader.value(&mut heap).is_err());
        // Reject the second header before reserving/decoding its claimed values.
        assert_eq!(reader.at, 10);
        assert_eq!(heap.arrays.allocations, 0);
    }

    #[test]
    fn deep_transfer_uses_flat_buffers_and_rejects_truncated_container_lengths() {
        let mut bytes = Vec::new();
        for _ in 0..20_000 {
            bytes.extend_from_slice(&[7, 0, 0, 0, 1]);
        }
        bytes.push(1);
        text(&mut bytes, "92233720368547758080001").unwrap();
        let mut heap = Heap::new(&[], &[]);
        let mut reader = Reader::new(&bytes);
        let value = reader.value(&mut heap).unwrap();
        reader.finish().unwrap();
        let mut encoded = Vec::new();
        encode(&heap, &mut encoded, value).unwrap();
        assert_eq!(encoded, bytes);
        heap.collect([value].into_iter()).unwrap();
        for invalid in [
            &[7, 255, 255, 255, 255][..],
            &[8, 255, 255, 255, 255],
            &[6, 2],
            &[4, 0, 0, 216, 0],
            &[3, 0, 0, 0, 1, 255],
            &[9],
        ] {
            assert!(Reader::new(invalid).value(&mut heap).is_err());
        }
    }
    #[test]
    fn mixed_maps_round_trip_and_nontransferable_values_are_rejected() {
        let mut heap = Heap::new(&[], &[]);
        let key = heap.allocate_text("key".into()).unwrap();
        let text_value = heap.allocate_text("OKW1 unicode: 🦀".into()).unwrap();
        let array = heap
            .allocate_array(vec![
                Slot::small(-42),
                Slot::boolean(true),
                text_value,
                Slot {
                    tag: FLOAT,
                    bits: (-0.0f64).to_bits(),
                },
                Slot {
                    tag: BYTE,
                    bits: 255,
                },
                Slot {
                    tag: CHAR,
                    bits: 0x1f980,
                },
            ])
            .unwrap();
        let value = heap.map_new([(key, array)].into_iter()).unwrap();
        let mut bytes = Vec::new();
        encode(&heap, &mut bytes, value).unwrap();
        let mut reader = Reader::new(&bytes);
        let decoded = reader.value(&mut heap).unwrap();
        reader.finish().unwrap();
        assert!(heap.equal(value, decoded).unwrap());
        for end in 0..bytes.len() {
            assert!(Reader::new(&bytes[..end]).value(&mut heap).is_err());
        }
        let object = heap.object_new(0, std::iter::empty()).unwrap();
        assert!(encode(&heap, &mut vec![], object).is_err());
    }
}
