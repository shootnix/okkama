//! Language failures are arena records; payload references participate in GC.
use crate::{
    Error,
    value::{FAILURE, Heap, Slot},
};
#[derive(Debug)]
pub(crate) enum FailureKind {
    Thrown(Slot),
    Native(Slot),
    Cancelled,
    Panic(String),
    Program(String),
}
#[derive(Debug)]
pub(crate) struct Failure {
    pub kind: FailureKind,
    pub cause: Option<Slot>,
    pub secondary: Vec<Slot>,
    pub trace: Vec<(usize, u32)>,
    pub message: Option<String>,
    last_frame: Option<u64>,
}
impl Heap {
    pub fn failure_record(
        &mut self,
        failure: Slot,
        frame: u64,
        function: usize,
        pc: u32,
    ) -> Result<(), Error> {
        self.failure(failure)?;
        let record = self.failures.get_mut(failure.bits)?;
        if record.last_frame != Some(frame) {
            record.trace.push((function, pc));
            record.last_frame = Some(frame);
        }
        Ok(())
    }
    pub fn failure_new(&mut self, kind: FailureKind) -> Result<Slot, Error> {
        Ok(Slot {
            bits: self.failures.allocate(Failure {
                kind,
                cause: None,
                secondary: Vec::new(),
                trace: Vec::new(),
                message: None,
                last_frame: None,
            })?,
            tag: FAILURE,
        })
    }
    pub fn failure_link(
        &mut self,
        fresh: Slot,
        other: Slot,
        secondary: bool,
    ) -> Result<Slot, Error> {
        self.failure(other)?;
        let record = self.failure(fresh)?;
        if secondary {
            self.failures.get_mut(fresh.bits)?.secondary.push(other);
            return Ok(fresh);
        }
        if !matches!(
            record.kind,
            FailureKind::Thrown(_) | FailureKind::Native(_) | FailureKind::Cancelled
        ) || fresh.bits == other.bits
        {
            return Ok(fresh);
        }
        let mut cursor = fresh;
        let mut seen = std::collections::HashSet::new();
        loop {
            if !seen.insert(cursor.bits) {
                return Ok(fresh);
            }
            match self.failure(cursor)?.cause {
                Some(next) if next.bits == other.bits => return Ok(fresh),
                Some(next) => cursor = next,
                None => break,
            }
        }
        self.failures.get_mut(cursor.bits)?.cause = Some(other);
        Ok(fresh)
    }

    pub fn failure(&self, value: Slot) -> Result<&Failure, Error> {
        if value.tag != FAILURE {
            return Err(Error("expected failure handle".into()));
        }
        self.failures.get(value.bits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn caught_failure_keeps_payload_and_descendants_alive() {
        let mut heap = Heap::new(&[], &[]);
        let text = heap.allocate_text("payload".into()).unwrap();
        let object = heap.object_new(0, [text].into_iter()).unwrap();
        let failure = heap.failure_new(FailureKind::Thrown(object)).unwrap();
        for _ in 0..1000 {
            heap.allocate_array(vec![]).unwrap();
            if heap.needs_collection() {
                heap.collect([failure].into_iter()).unwrap();
            }
        }
        assert_eq!(
            heap.text(heap.object_get(object, 0).unwrap()).unwrap(),
            "payload"
        );
        heap.collect(std::iter::empty()).unwrap();
        assert!(heap.text(text).is_err());
    }
}

#[cfg(test)]
mod chain_tests {
    use super::*;
    #[test]
    fn causes_and_secondary_failures_preserve_identity_and_trace_cycles() {
        let mut heap = Heap::new(&[], &[]);
        let object = heap.object_new(0, [].into_iter()).unwrap();
        let first = heap.failure_new(FailureKind::Thrown(object)).unwrap();
        let second = heap.failure_new(FailureKind::Thrown(object)).unwrap();
        let cleanup = heap
            .failure_new(FailureKind::Panic("cleanup".into()))
            .unwrap();
        assert_eq!(
            heap.failure_link(first, second, false).unwrap().bits,
            first.bits
        );
        assert_eq!(
            heap.failure(first).unwrap().cause.unwrap().bits,
            second.bits
        );
        heap.failure_link(first, second, false).unwrap();
        assert!(heap.failure(second).unwrap().cause.is_none());
        heap.failure_link(first, cleanup, true).unwrap();
        heap.failure_link(first, cleanup, true).unwrap();
        assert_eq!(heap.failure(first).unwrap().secondary.len(), 2);
        heap.failure_link(cleanup, first, false).unwrap();
        assert!(heap.failure(cleanup).unwrap().cause.is_none());
        heap.failure_link(second, first, false).unwrap();
        heap.collect([first].into_iter()).unwrap();
        assert!(heap.failure(second).is_ok());
        assert!(heap.failure(cleanup).is_ok());
        heap.collect(std::iter::empty()).unwrap();
        assert!(heap.failure(first).is_err());
        assert!(heap.object_descriptor(object).is_err());
    }
}
