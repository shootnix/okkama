//! Arrays and builders own contiguous slots. Finish transfers the backing Vec;
//! the original builder stays live and empty, matching Okkama's alias semantics.
use crate::{
    Error,
    value::{ARRAY, BUILDER, BYTE, Heap, STR, Slot},
};
#[derive(Debug, Clone, Copy)]
pub(crate) enum ArrayOp {
    Length,
    Get,
    Slice,
    Join,
    BuilderNew,
    Push,
    Append,
    Finish,
    Split,
    Utf8,
}
impl Heap {
    pub fn allocate_array(&mut self, elements: Vec<Slot>) -> Result<Slot, Error> {
        Ok(Slot {
            bits: self.arrays.allocate(elements)?,
            tag: ARRAY,
        })
    }
    pub fn elements(&self, value: Slot) -> Result<&[Slot], Error> {
        if value.tag != ARRAY {
            return Err(Error("expected array handle".into()));
        }
        Ok(self.arrays.get(value.bits)?)
    }
    pub fn array_lookup(&self, array: Slot, index: Slot) -> Result<Option<Slot>, Error> {
        let elements = self.elements(array)?;
        let index = self
            .integer(index)?
            .to_i64()
            .and_then(|i| usize::try_from(i).ok());
        Ok(index.and_then(|i| elements.get(i).copied()))
    }
    pub fn slice_bounds(
        &self,
        length: usize,
        start: Slot,
        end: Slot,
    ) -> Result<std::ops::Range<usize>, Error> {
        if self.compare(start, Slot::small(0))?.is_lt() {
            return Err(Error("Slice start must not be negative".into()));
        }
        if self.compare(start, end)?.is_gt() {
            return Err(Error(
                "Slice start must be less than or equal to slice end".into(),
            ));
        }
        if self.compare(end, Slot::small(length as i64))?.is_gt() {
            return Err(Error("Slice end must not exceed length".into()));
        }
        Ok(
            self.integer(start)?.to_i64().expect("validated index") as usize
                ..self.integer(end)?.to_i64().expect("validated index") as usize,
        )
    }
    pub fn array_op(&mut self, op: ArrayOp, args: [Slot; 3]) -> Result<Slot, Error> {
        use ArrayOp::*;
        if matches!(op, Push | Append | Finish) && args[0].tag != BUILDER {
            return Err(Error("expected builder handle".into()));
        }
        match op {
            Length => Ok(Slot::small(self.elements(args[0])?.len() as i64)),
            Get => {
                let index = self
                    .integer(args[1])?
                    .to_i64()
                    .and_then(|i| usize::try_from(i).ok());
                let elements = self.elements(args[0])?;
                index
                    .and_then(|i| elements.get(i).copied())
                    .ok_or_else(|| Error("Array index out of bounds".into()))
            }
            Slice => {
                let elements = self.elements(args[0])?;
                let range = self.slice_bounds(elements.len(), args[1], args[2])?;
                self.allocate_array(elements[range].to_vec())
            }
            Join => {
                let separator = self.text(args[1])?;
                let mut text = String::new();
                for (i, element) in self.elements(args[0])?.iter().enumerate() {
                    if i != 0 {
                        text.push_str(separator);
                    }
                    text.push_str(self.text(*element)?);
                }
                self.allocate_text(text)
            }
            BuilderNew => Ok(Slot {
                bits: self.builders.allocate(Vec::new())?,
                tag: BUILDER,
            }),
            Push => {
                self.builders.get_mut(args[0].bits)?.push(args[1]);
                Ok(args[0])
            }
            Append => {
                if args[1].tag != ARRAY {
                    return Err(Error("expected array handle".into()));
                }
                // Separate typed arenas allow a direct borrowed bulk copy.
                self.builders
                    .get_mut(args[0].bits)?
                    .extend_from_slice(self.arrays.get(args[1].bits)?);
                Ok(args[0])
            }
            Finish => {
                let elements = std::mem::take(self.builders.get_mut(args[0].bits)?);
                self.allocate_array(elements)
            }
            Utf8 => {
                let bytes = self
                    .text(args[0])?
                    .bytes()
                    .map(|b| Slot {
                        bits: u64::from(b),
                        tag: BYTE,
                    })
                    .collect();
                self.allocate_array(bytes)
            }
            Split => {
                if args[0].tag != STR || args[1].tag != STR {
                    return Err(Error("expected text operands".into()));
                }
                let text = self.strings.get(args[0].bits)?.clone();
                let separator = self.strings.get(args[1].bits)?.clone();
                let mut elements = Vec::new();
                if separator.is_empty() {
                    for (i, ch) in text.char_indices() {
                        elements.push(self.allocate_text(text[i..i + ch.len_utf8()].to_owned())?);
                    }
                } else {
                    for part in text.split(separator.as_ref()) {
                        elements.push(self.allocate_text(part.to_owned())?);
                    }
                }
                self.allocate_array(elements)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builders_and_arrays_trace_objects_and_text_without_ownership_walks() {
        let mut heap = Heap::new(&[], &[]);
        let text = heap.allocate_text("held".into()).unwrap();
        let object = heap.object_new(0, [text].into_iter()).unwrap();
        let builder = heap
            .array_op(ArrayOp::BuilderNew, [Slot::default(); 3])
            .unwrap();
        heap.array_op(ArrayOp::Push, [builder, object, Slot::default()])
            .unwrap();
        for _ in 0..1000 {
            heap.allocate_array(vec![Slot::small(1)]).unwrap();
            if heap.needs_collection() {
                heap.collect([builder].into_iter()).unwrap();
            }
        }
        assert!(heap.collections >= 7);
        let array = heap
            .array_op(ArrayOp::Finish, [builder, Slot::default(), Slot::default()])
            .unwrap();
        heap.collect([array].into_iter()).unwrap();
        let field = heap
            .object_get(heap.elements(array).unwrap()[0], 0)
            .unwrap();
        assert_eq!(heap.text(field).unwrap(), "held");
        heap.collect(std::iter::empty()).unwrap();
        assert!(heap.text(text).is_err());
    }
}
