//! UTF-8 storage, Unicode scalar indexing and literal text operations.
use crate::{
    Error,
    value::{Heap, Slot},
};
#[derive(Debug, Clone, Copy)]
pub(crate) enum TextOp {
    Concat,
    Length,
    Slice,
    Replace,
    ReplaceFirst,
    Trim,
    Chomp,
}
impl Heap {
    pub fn text_op(&mut self, op: TextOp, args: [Slot; 3]) -> Result<Slot, Error> {
        let text = self.text(args[0])?;
        if matches!(op, TextOp::Length) {
            return Ok(Slot::small(text.chars().count() as i64));
        }
        let out = match op {
            TextOp::Concat => {
                let right = self.text(args[1])?;
                let mut out = String::with_capacity(text.len() + right.len());
                out.push_str(text);
                out.push_str(right);
                out
            }
            TextOp::Trim => text.trim_matches(char::is_whitespace).to_owned(),
            TextOp::Chomp => text
                .strip_suffix("\r\n")
                .or_else(|| text.strip_suffix('\n'))
                .unwrap_or(text)
                .to_owned(),
            TextOp::Replace => text.replace(self.text(args[1])?, self.text(args[2])?),
            TextOp::ReplaceFirst => text.replacen(self.text(args[1])?, self.text(args[2])?, 1),
            TextOp::Slice => {
                // Check exact integers before narrowing and retain error order.
                if self.compare(args[1], Slot::small(0))?.is_lt() {
                    return Err(Error("Slice start must not be negative".into()));
                }
                if self.compare(args[1], args[2])?.is_gt() {
                    return Err(Error(
                        "Slice start must be less than or equal to slice end".into(),
                    ));
                }
                if self
                    .compare(args[2], Slot::small(text.chars().count() as i64))?
                    .is_gt()
                {
                    return Err(Error("Slice end must not exceed length".into()));
                }
                let start = self.integer(args[1])?.to_i64().expect("validated index") as usize;
                let end = self.integer(args[2])?.to_i64().expect("validated index") as usize;
                let boundary = |n| {
                    text.char_indices()
                        .map(|(i, _)| i)
                        .chain(std::iter::once(text.len()))
                        .nth(n)
                        .expect("validated code point boundary")
                };
                text[boundary(start)..boundary(end)].to_owned()
            }
            TextOp::Length => unreachable!(),
        };
        self.allocate_text(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    #[test]
    fn text_roots_through_objects_survive_repeated_collection() {
        let constants = [Rc::<str>::from("constant")];
        let mut heap = Heap::new(&[], &constants);
        let text = heap.allocate_text("🙂held".into()).unwrap();
        let root = heap.object_new(0, [text].into_iter()).unwrap();
        for _ in 0..1000 {
            heap.allocate_text("temporary".into()).unwrap();
            if heap.needs_collection() {
                heap.collect([root].into_iter()).unwrap();
            }
        }
        assert!(heap.collections >= 7);
        assert_eq!(
            heap.text(heap.object_get(root, 0).unwrap()).unwrap(),
            "🙂held"
        );
        heap.collect(std::iter::empty()).unwrap();
        assert!(heap.text(text).is_err());
        assert_eq!(
            heap.text(Slot {
                bits: 0,
                tag: crate::value::STR
            })
            .unwrap(),
            "constant"
        );
    }
}
