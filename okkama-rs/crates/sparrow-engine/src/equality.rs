//! Iterative structural comparison with reusable traversal storage.
use crate::{Error, value::*};
impl Heap {
    pub fn any_box(&mut self, value: Slot) -> Result<Slot, Error> {
        Ok(Slot {
            bits: self.boxes.allocate(value)?,
            tag: ANY,
        })
    }
    pub fn equal(&mut self, left: Slot, right: Slot) -> Result<bool, Error> {
        let mut pending = std::mem::take(&mut self.equality_work);
        let mut seen = std::mem::take(&mut self.equality_seen);
        pending.clear();
        seen.clear();
        pending.push((left, right));
        let result = (|| {
            while let Some((a, b)) = pending.pop() {
                if a.tag == ANY || b.tag == ANY {
                    if a.tag == ANY && b.tag == ANY && a.bits == b.bits {
                        self.boxes.get(a.bits)?;
                        continue;
                    }
                    return Err(Error("Cannot compare any without narrowing".into()));
                }
                if matches!(a.tag, SMALL | BIG) && matches!(b.tag, SMALL | BIG)
                    || matches!(a.tag, RAT | SMALL_RAT) && matches!(b.tag, RAT | SMALL_RAT)
                {
                    if !self.compare(a, b)?.is_eq() {
                        return Ok(false);
                    }
                    continue;
                }
                if matches!(a.tag, UNBOUND | BUILDER | MAP_BUILDER | FAILURE | REGION)
                    || matches!(b.tag, UNBOUND | BUILDER | MAP_BUILDER | FAILURE | REGION)
                {
                    return Err(Error("cannot compare internal or undefined values".into()));
                }
                if matches!(
                    (a.tag, b.tag),
                    (NATIVE_FAILURE, OBJECT)
                        | (OBJECT, NATIVE_FAILURE)
                        | (CANCELLED, OBJECT)
                        | (OBJECT, CANCELLED)
                        | (CANCELLED, NATIVE_FAILURE)
                        | (NATIVE_FAILURE, CANCELLED)
                ) {
                    return Ok(false);
                }
                if a.tag != b.tag {
                    return Err(Error("Cannot compare values of different types".into()));
                }
                let equal = match a.tag {
                    CANCELLED => true,
                    FLOAT | BOOL | BYTE | CHAR => a.bits == b.bits,
                    STR => self.text(a)? == self.text(b)?,
                    ENUM => {
                        if a.bits >> 32 != b.bits >> 32 {
                            return Err(Error("Cannot compare values of different types".into()));
                        }
                        a.bits == b.bits
                    }
                    NATIVE_FAILURE => {
                        pending.extend(
                            self.native_failure_fields(a)?
                                .iter()
                                .copied()
                                .zip(self.native_failure_fields(b)?.iter().copied()),
                        );
                        true
                    }
                    HANDLE => {
                        self.handles.get(a.bits)?;
                        self.handles.get(b.bits)?;
                        a.bits == b.bits
                    }
                    FUTURE => {
                        self.future(a)?;
                        self.future(b)?;
                        a.bits == b.bits
                    }
                    CLOSURE => {
                        self.closure(a)?;
                        self.closure(b)?;
                        a.bits == b.bits
                    }
                    RANGE => {
                        let a = self.ranges.get(a.bits)?;
                        let b = self.ranges.get(b.bits)?;
                        a[2].bits == b[2].bits
                            && self.compare(a[0], b[0])?.is_eq()
                            && self.compare(a[1], b[1])?.is_eq()
                    }
                    OBJECT => {
                        let x = self.objects.get(a.bits)?;
                        let y = self.objects.get(b.bits)?;
                        if x[0].bits != y[0].bits || x.len() != y.len() {
                            return Ok(false);
                        }
                        if a.bits != b.bits && seen.insert((OBJECT, a.bits, b.bits)) {
                            for (x, y) in x[1..].iter().zip(&y[1..]).rev() {
                                if x.tag == UNBOUND || y.tag == UNBOUND {
                                    return Err(Error(
                                        "cannot compare unbound object fields".into(),
                                    ));
                                }
                                pending.push((*x, *y));
                            }
                        }
                        true
                    }
                    ARRAY => {
                        let x = self.elements(a)?;
                        let y = self.elements(b)?;
                        if x.len() != y.len() {
                            return Ok(false);
                        }
                        if a.bits != b.bits && seen.insert((ARRAY, a.bits, b.bits)) {
                            pending.extend(x.iter().copied().zip(y.iter().copied()).rev());
                        }
                        true
                    }
                    MAP => {
                        let x = self.maps.get(a.bits)?;
                        let y = self.maps.get(b.bits)?;
                        if x.len() != y.len() {
                            return Ok(false);
                        }
                        if a.bits != b.bits && seen.insert((MAP, a.bits, b.bits)) {
                            for ((k, (_, v)), (k2, (_, v2))) in x.iter().zip(y) {
                                if k != k2 {
                                    return Ok(false);
                                }
                                pending.push((*v, *v2));
                            }
                        }
                        true
                    }
                    _ => return Err(Error("unsupported equality value".into())),
                };
                if !equal {
                    return Ok(false);
                }
            }
            Ok(true)
        })();
        pending.clear();
        seen.clear();
        self.equality_work = pending;
        self.equality_seen = seen;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equality_is_iterative_for_deep_and_cyclic_objects() {
        let mut heap = Heap::new(&[], &[]);
        let mut left = Slot::small(42);
        let mut right = left;
        for _ in 0..100_000 {
            left = heap.object_new(0, [left].into_iter()).unwrap();
            right = heap.object_new(0, [right].into_iter()).unwrap();
        }
        assert!(heap.equal(left, right).unwrap());
        let absent = Slot {
            bits: 0,
            tag: UNBOUND,
        };
        let a = heap.object_new(0, [absent].into_iter()).unwrap();
        let b = heap.object_new(0, [absent].into_iter()).unwrap();
        heap.object_init(a, 0, a).unwrap();
        heap.object_init(b, 0, b).unwrap();
        assert!(heap.equal(a, b).unwrap());
        let other = heap.object_new(1, [absent].into_iter()).unwrap();
        assert!(!heap.equal(a, other).unwrap());
        let unbound = heap.object_new(0, [absent].into_iter()).unwrap();
        assert!(heap.equal(a, unbound).unwrap_err().0.contains("unbound"));
        assert!(heap.equal(unbound, unbound).unwrap());
    }
    #[test]
    fn structural_float_and_range_equality_preserve_representation() {
        let mut heap = Heap::new(&[], &[]);
        let a = Slot {
            bits: (-0.0f64).to_bits(),
            tag: FLOAT,
        };
        let b = Slot {
            bits: 0.0f64.to_bits(),
            tag: FLOAT,
        };
        assert!(heap.compare(a, b).unwrap().is_eq());
        assert!(!heap.equal(a, b).unwrap());
        let x = heap
            .range_new(Slot::small(0), Slot::small(1), true)
            .unwrap();
        let y = heap
            .range_new(Slot::small(0), Slot::small(2), false)
            .unwrap();
        assert!(!heap.equal(x, y).unwrap());
        assert_eq!(
            heap.range_part(x, true).unwrap().bits,
            heap.range_part(y, true).unwrap().bits
        );
    }
    #[test]
    fn any_box_identity_and_roots_are_distinct_from_payload_equality() {
        let mut heap = Heap::new(&[], &[]);
        let text = heap.allocate_text("value".into()).unwrap();
        let a = heap.any_box(text).unwrap();
        let b = heap.any_box(text).unwrap();
        assert!(heap.equal(a, a).unwrap());
        assert_eq!(
            heap.equal(a, b).unwrap_err().0,
            "Cannot compare any without narrowing"
        );
        heap.collect([a].into_iter()).unwrap();
        assert_eq!(heap.text(text).unwrap(), "value");
        heap.collect(std::iter::empty()).unwrap();
        assert!(heap.text(text).is_err());
    }
}
