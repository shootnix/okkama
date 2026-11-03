//! Exact ascending ranges. Bounds are traced slots, not recursively owned data.
use crate::{
    Error,
    value::{Arithmetic, BUILDER, Heap, RANGE, Slot},
};
impl Heap {
    pub fn range_new(&mut self, start: Slot, end: Slot, inclusive: bool) -> Result<Slot, Error> {
        if self.compare(start, end)?.is_gt() {
            return Err(Error(
                "Range start must be less than or equal to range end".into(),
            ));
        }
        Ok(Slot {
            bits: self
                .ranges
                .allocate([start, end, Slot::boolean(inclusive)])?,
            tag: RANGE,
        })
    }
    pub fn range_part(&mut self, range: Slot, end: bool) -> Result<Slot, Error> {
        if range.tag != RANGE {
            return Err(Error("expected range handle".into()));
        }
        let [start, bound, inclusive] = *self.ranges.get(range.bits)?;
        if !end {
            Ok(start)
        } else if inclusive.bits == 0 {
            Ok(bound)
        } else {
            self.arithmetic(Arithmetic::Add, bound, Slot::small(1))
        }
    }
    pub fn builder_range(
        &mut self,
        builder: Slot,
        start: Slot,
        end: Slot,
        inclusive: bool,
    ) -> Result<Slot, Error> {
        if builder.tag != BUILDER {
            return Err(Error("expected builder handle".into()));
        }
        if self.compare(start, end)?.is_gt() {
            return Err(Error(
                "Range start must be less than or equal to range end".into(),
            ));
        }
        let distance = self.arithmetic(Arithmetic::Subtract, end, start)?;
        let count = self
            .integer(distance)?
            .to_i64()
            .and_then(|n| usize::try_from(n).ok())
            .and_then(|n| n.checked_add(usize::from(inclusive)))
            .ok_or_else(|| Error("Range is too large to materialize".into()))?;
        self.builders
            .get_mut(builder.bits)?
            .try_reserve(count)
            .map_err(|_| Error("Cannot allocate range elements".into()))?;
        let mut next = start;
        for i in 0..count {
            self.builders.get_mut(builder.bits)?.push(next);
            if i + 1 < count {
                next = self.arithmetic(Arithmetic::Add, next, Slot::small(1))?;
            }
        }
        Ok(builder)
    }
}
