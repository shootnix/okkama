//! A cyclic barrier's break/reset policy layered over Tokio watch notifications.
//! Tokio Barrier alone cannot express Okkama's cancellation/broken generations.
use crate::{
    Error,
    future::State,
    value::{Heap, Slot},
};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
#[derive(Debug, Clone, Copy)]
pub(crate) enum Operation {
    Wait(u32),
    Snapshot(u32),
    Abort,
    Reset,
    Read(usize),
}
#[derive(Debug)]
pub(crate) struct Barrier {
    parties: i64,
    generation: i64,
    broken: bool,
    waiting: Vec<Rc<Request>>,
}
#[derive(Debug, Clone, Copy)]
pub(crate) struct Snapshot {
    parties: i64,
    waiters: i64,
    generation: i64,
    broken: bool,
}
#[derive(Debug)]
pub(crate) struct Request {
    barrier: Weak<RefCell<Barrier>>,
    receiver: Slot,
    generation: i64,
    index: i64,
    closed: u32,
    result: Cell<Option<Result<Slot, u32>>>,
    cancelled: Cell<bool>,
    changed: tokio::sync::watch::Sender<bool>,
}
impl Request {
    fn complete(&self, result: Result<Slot, u32>) {
        self.result.set(Some(result));
        self.changed.send_replace(true);
    }
    pub fn cancel(&self) {
        if self.result.get().is_some() || self.cancelled.replace(true) {
            return;
        }
        if let Some(barrier) = self.barrier.upgrade() {
            let mut barrier = barrier.borrow_mut();
            if barrier.generation == self.generation {
                barrier.break_generation();
            }
        }
    }
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<bool> {
        self.changed.subscribe()
    }
    pub fn result(&self) -> Option<Result<Slot, u32>> {
        self.result.get()
    }
    pub fn roots(&self, roots: &mut Vec<Slot>) {
        if !self.cancelled.get() {
            roots.push(self.receiver);
        }
    }
}
impl Barrier {
    fn break_generation(&mut self) {
        self.broken = true;
        for request in self.waiting.drain(..) {
            if !request.cancelled.get() {
                request.complete(Err(request.closed));
            }
        }
    }
    fn next_generation(&mut self) -> Result<(), Error> {
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| Error("Barrier generation overflow".into()))?;
        Ok(())
    }
}
impl Heap {
    fn barrier(&mut self, receiver: Slot) -> Result<Rc<RefCell<Barrier>>, Error> {
        if let Some(barrier) = self.barriers.get(&receiver.bits) {
            return Ok(barrier.clone());
        }
        let parties = self
            .integer(self.object_get(receiver, 0)?)?
            .to_i64()
            .ok_or_else(|| Error("Cannot read field amount".into()))?;
        let barrier = Rc::new(RefCell::new(Barrier {
            parties,
            generation: 0,
            broken: false,
            waiting: vec![],
        }));
        self.barriers.insert(receiver.bits, barrier.clone());
        Ok(barrier)
    }
    pub(crate) fn barrier_operation(
        &mut self,
        operation: Operation,
        receiver: Slot,
    ) -> Result<Slot, Error> {
        if let Operation::Read(field) = operation {
            let snapshot = self
                .barrier_snapshots
                .get(&receiver.bits)
                .ok_or_else(|| Error("Invalid barrier state".into()))?;
            return Ok(match field {
                0 => Slot::small(snapshot.parties),
                1 => Slot::small(snapshot.waiters),
                2 => Slot::small(snapshot.generation),
                3 => Slot::boolean(snapshot.broken),
                _ => return Err(Error("Invalid barrier state field".into())),
            });
        }
        let barrier = self.barrier(receiver)?;
        match operation {
            Operation::Wait(closed) => {
                let request = {
                    let mut state = barrier.borrow_mut();
                    let request = Rc::new(Request {
                        barrier: Rc::downgrade(&barrier),
                        receiver,
                        generation: state.generation,
                        index: state.waiting.len() as i64,
                        closed,
                        result: Cell::new(None),
                        cancelled: Cell::new(false),
                        changed: tokio::sync::watch::channel(false).0,
                    });
                    if state.broken {
                        request.complete(Err(closed));
                    } else {
                        state.waiting.push(request.clone());
                        if state.waiting.len() as i64 == state.parties {
                            state.next_generation()?;
                            for request in state.waiting.drain(..) {
                                request.complete(Ok(Slot::small(request.index)));
                            }
                        }
                    }
                    request
                };
                let future = self.future_with_state(State::Signal(receiver))?;
                let cell = self.futures.get_mut(future.bits)?;
                cell.captures = vec![receiver].into_boxed_slice();
                cell.request = Some(crate::signal::Request::Barrier(request));
                self.settle_signal(future)?;
                Ok(future)
            }
            Operation::Snapshot(owner) => {
                let state = barrier.borrow();
                let snapshot = Snapshot {
                    parties: state.parties,
                    waiters: state.waiting.len() as i64,
                    generation: state.generation,
                    broken: state.broken,
                };
                let value = self.object_new(owner, std::iter::empty())?;
                self.barrier_snapshots.insert(value.bits, snapshot);
                Ok(value)
            }
            Operation::Abort => {
                barrier.borrow_mut().break_generation();
                Ok(Slot::default())
            }
            Operation::Reset => {
                let mut state = barrier.borrow_mut();
                state.break_generation();
                state.next_generation()?;
                state.broken = false;
                Ok(Slot::default())
            }
            Operation::Read(_) => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_old_wait_cannot_break_the_reset_generation_and_snapshots_survive_gc() {
        let mut heap = Heap::new(&[], &[]);
        let owner = heap.object_new(0, [Slot::small(2)].into_iter()).unwrap();
        let old = heap.barrier_operation(Operation::Wait(1), owner).unwrap();
        let snapshot = heap
            .barrier_operation(Operation::Snapshot(2), owner)
            .unwrap();
        heap.barrier_operation(Operation::Reset, owner).unwrap();
        let current = heap.barrier_operation(Operation::Wait(1), owner).unwrap();
        heap.future_control(okkama_sparrow::FutOp::Cancel, old)
            .unwrap();
        assert!(!heap.barriers[&owner.bits].borrow().broken);
        assert_eq!(heap.barriers[&owner.bits].borrow().waiting.len(), 1);
        heap.collect([current, snapshot].into_iter()).unwrap();
        assert_eq!(
            heap.barrier_operation(Operation::Read(1), snapshot)
                .unwrap()
                .bits,
            1
        );
        assert_eq!(
            heap.barrier_operation(Operation::Read(2), snapshot)
                .unwrap()
                .bits,
            0
        );
        let last = heap.barrier_operation(Operation::Wait(1), owner).unwrap();
        assert!(matches!(heap.future(last).unwrap().state,State::Ready(Ok(v)) if v.bits==1));
        heap.settle_signal(current).unwrap();
        assert!(matches!(heap.future(current).unwrap().state,State::Ready(Ok(v)) if v.bits==0));
        heap.collect(std::iter::empty()).unwrap();
        assert!(heap.barriers.is_empty());
        assert!(heap.barrier_snapshots.is_empty());
    }
    #[test]
    fn cancelled_native_wait_breaks_peers_but_keeps_its_own_cancelled_failure() {
        let mut heap = Heap::new(&[], &[]);
        let owner = heap.object_new(0, [Slot::small(3)].into_iter()).unwrap();
        let first = heap.barrier_operation(Operation::Wait(1), owner).unwrap();
        let peer = heap.barrier_operation(Operation::Wait(1), owner).unwrap();
        heap.future_control(okkama_sparrow::FutOp::Cancel, first)
            .unwrap();
        assert!(heap.barriers[&owner.bits].borrow().broken);
        heap.settle_signal(peer).unwrap();
        let State::Ready(Err(failure)) = heap.future(first).unwrap().state else {
            panic!("cancelled")
        };
        assert!(matches!(
            heap.failure(failure).unwrap().kind,
            crate::failure::FailureKind::Cancelled
        ));
        let State::Ready(Err(failure)) = heap.future(peer).unwrap().state else {
            panic!("broken")
        };
        assert!(matches!(
            heap.failure(failure).unwrap().kind,
            crate::failure::FailureKind::Thrown(_)
        ));
    }
}
