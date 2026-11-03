//! A Tokio FIFO admission gate separates queueing from Okkama's i64 capacity.
//! A binary gate avoids truncating counts to Tokio::Semaphore::MAX_PERMITS.
use crate::{
    Error,
    value::{Heap, Slot},
};
use std::{cell::Cell, rc::Rc, sync::Arc};
#[derive(Debug)]
pub(crate) struct Pool {
    pub gate: Arc<tokio::sync::Semaphore>,
    remaining: Cell<i64>,
    pub pending: Cell<usize>,
}
impl Pool {
    fn new(amount: i64) -> Rc<Self> {
        Rc::new(Self {
            gate: Arc::new(tokio::sync::Semaphore::new(usize::from(amount > 0))),
            remaining: Cell::new(amount),
            pending: Cell::new(0),
        })
    }
    pub fn available(&self) -> i64 {
        self.remaining
            .get()
            .saturating_sub(i64::try_from(self.pending.get()).unwrap_or(i64::MAX))
            .max(0)
    }
    pub fn commit(&self, permit: tokio::sync::OwnedSemaphorePermit) {
        let remaining = self.remaining.get();
        assert!(remaining > 0, "admission requires capacity");
        self.remaining.set(remaining - 1);
        if remaining == 1 {
            permit.forget();
        } // Otherwise drop passes the FIFO baton.
    }
    pub fn release(&self) {
        let before = self.remaining.get();
        self.remaining
            .set(before.checked_add(1).expect("permit released twice"));
        if before == 0 {
            self.gate.add_permits(1);
        }
    }
}
impl Heap {
    pub(crate) fn semaphore_pool(&mut self, receiver: Slot) -> Result<Rc<Pool>, Error> {
        if let Some(pool) = self.semaphores.get(&receiver.bits) {
            return Ok(pool.clone());
        }
        let amount = self
            .integer(self.object_get(receiver, 0)?)?
            .to_i64()
            .ok_or_else(|| Error("Cannot read field amount".into()))?;
        let pool = Pool::new(amount);
        self.semaphores.insert(receiver.bits, pool.clone());
        Ok(pool)
    }
    pub(crate) fn semaphore_grant(
        &mut self,
        owner: u32,
        pool: Rc<Pool>,
        permit: tokio::sync::OwnedSemaphorePermit,
    ) -> Result<Slot, Error> {
        pool.commit(permit);
        let value = self.object_new(owner, std::iter::empty())?;
        self.grants
            .insert(value.bits, crate::intrinsic::Grant::semaphore(pool));
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{future::State, intrinsic::Operation};
    fn acquire(heap: &mut Heap, receiver: Slot) -> Slot {
        heap.intrinsic(Operation::SemaphoreAcquire(1), Some(receiver))
            .unwrap()
    }
    fn ready(heap: &Heap, future: Slot) -> Slot {
        let State::Ready(Ok(value)) = heap.future(future).unwrap().state else {
            panic!("not ready");
        };
        value
    }
    fn close(heap: &mut Heap, guard: Slot) {
        heap.intrinsic(Operation::GrantClose, Some(guard)).unwrap();
    }
    #[test]
    fn queued_capacity_is_reserved_before_host_poll_and_cancel_restores_fifo() {
        let mut heap = Heap::new(&[], &[]);
        let owner = heap.object_new(0, [Slot::small(2)].into_iter()).unwrap();
        let a = acquire(&mut heap, owner);
        let b = acquire(&mut heap, owner);
        let (a, b) = (ready(&heap, a), ready(&heap, b));
        let first = acquire(&mut heap, owner);
        let removed = acquire(&mut heap, owner);
        let last = acquire(&mut heap, owner);
        let pool = heap.semaphore_pool(owner).unwrap();
        assert_eq!(pool.pending.get(), 3);
        heap.future_control(okkama_sparrow::FutOp::Cancel, removed)
            .unwrap();
        assert_eq!(pool.pending.get(), 2);
        close(&mut heap, a);
        close(&mut heap, b);
        assert_eq!(pool.available(), 0);
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        let r1 = heap.future(first).unwrap().permit.clone().unwrap();
        let r2 = heap.future(last).unwrap().permit.clone().unwrap();
        assert!(r2.poll(&mut cx).is_pending());
        let permit = match r1.poll(&mut cx) {
            std::task::Poll::Ready(Ok(v)) => v,
            _ => panic!("first waiter"),
        };
        let g1 = heap.semaphore_grant(1, pool.clone(), permit).unwrap();
        assert_eq!(pool.available(), 0);
        let permit = match r2.poll(&mut cx) {
            std::task::Poll::Ready(Ok(v)) => v,
            _ => panic!("second waiter"),
        };
        let g2 = heap.semaphore_grant(1, pool.clone(), permit).unwrap();
        close(&mut heap, g1);
        close(&mut heap, g2);
        close(&mut heap, g1);
        assert_eq!(pool.available(), 2);
        assert_eq!(pool.pending.get(), 0);
    }
    #[test]
    fn collecting_an_unclosed_permit_does_not_return_capacity() {
        let mut heap = Heap::new(&[], &[]);
        let owner = heap.object_new(0, [Slot::small(2)].into_iter()).unwrap();
        acquire(&mut heap, owner);
        heap.collect([owner].into_iter()).unwrap();
        assert!(heap.grants.is_empty());
        assert_eq!(heap.semaphore_pool(owner).unwrap().available(), 1);
        heap.collect(std::iter::empty()).unwrap();
        assert!(heap.semaphores.is_empty());
    }
}
