//! Language Future identity/cache. Scheduling and external wakes belong to Tokio.
use crate::{
    Error,
    value::{FUTURE, Heap, Slot},
};
#[derive(Debug, Clone, Copy)]
pub(crate) enum State {
    Cold,
    Worker,
    Io,
    Running,
    Timer(std::time::Instant),
    Event(Slot),
    Signal(Slot),
    Collect(Slot),
    Stream(Slot),
    Permit(Slot, u32),
    Ready(Result<Slot, Slot>),
}
#[derive(Debug)]
pub(crate) struct FutureCell {
    pub function: usize,
    pub io: Option<std::rc::Rc<crate::native_io::Request>>,
    pub worker: Option<std::rc::Rc<crate::worker_process::Request>>,
    pub ready_order: u64,
    pub spawned: bool,
    pub observed: bool,
    pub request: Option<crate::signal::Request>,
    pub cancel_request: Option<crate::signal::WeakRequest>,
    pub permit: Option<std::rc::Rc<crate::intrinsic::PermitRequest>>,
    pub captures: Box<[Slot]>,
    pub state: State,
    pub cancel_requested: bool,
    pub cancel_delivered: bool,
    changed: Option<tokio::sync::watch::Sender<()>>,
    pub cancel_permit: Option<std::rc::Weak<crate::intrinsic::PermitRequest>>,
    pub cancel_wake: Option<tokio::sync::watch::Sender<()>>,
}
impl Drop for FutureCell {
    fn drop(&mut self) {
        if let Some(request) = &self.request {
            request.cancel();
        }
    }
}
impl Heap {
    pub fn future_control(
        &mut self,
        op: okkama_sparrow::FutOp,
        value: Slot,
    ) -> Result<Slot, Error> {
        let future = self.future(value)?;
        match op {
            okkama_sparrow::FutOp::Done => {
                Ok(Slot::boolean(matches!(future.state, State::Ready(_))))
            }
            okkama_sparrow::FutOp::Cancelled => Ok(Slot::boolean(
                if let State::Ready(Err(failure)) = future.state {
                    matches!(
                        self.failure(failure)?.kind,
                        crate::failure::FailureKind::Cancelled
                    )
                } else {
                    false
                },
            )),
            okkama_sparrow::FutOp::Cancel => {
                match future.state {
                    State::Worker => {
                        if let Some(request) = &future.worker {
                            request.cancel();
                        }
                    }
                    State::Io => {
                        let draining = future
                            .io
                            .as_ref()
                            .is_some_and(|request| request.request_cancel());
                        if !draining {
                            let failure =
                                self.failure_new(crate::failure::FailureKind::Cancelled)?;
                            self.future_state(value, State::Ready(Err(failure)))?;
                        }
                    }
                    State::Cold
                    | State::Timer(_)
                    | State::Event(_)
                    | State::Stream(_)
                    | State::Collect(_)
                    | State::Signal(_)
                    | State::Permit(_, _) => {
                        let failure = self.failure_new(crate::failure::FailureKind::Cancelled)?;
                        self.future_state(value, State::Ready(Err(failure)))?;
                    }
                    State::Running if !future.cancel_requested => {
                        self.futures.get_mut(value.bits)?.cancel_requested = true;
                        if let Some(request) = self
                            .futures
                            .get_mut(value.bits)?
                            .cancel_permit
                            .as_ref()
                            .and_then(std::rc::Weak::upgrade)
                        {
                            request.cancel();
                        }
                        if let Some(request) =
                            self.futures.get_mut(value.bits)?.cancel_request.as_ref()
                        {
                            request.cancel();
                        }
                        if let Some(changed) = &self.futures.get_mut(value.bits)?.cancel_wake {
                            changed.send_replace(());
                        }
                    }
                    State::Running | State::Ready(_) => {}
                }
                Ok(Slot {
                    bits: 0,
                    tag: crate::value::UNBOUND,
                })
            }
        }
    }

    pub fn future_with_state(&mut self, state: State) -> Result<Slot, Error> {
        let ready_order = if matches!(state, State::Ready(_)) {
            self.completion_clock += 1;
            self.completion_clock
        } else {
            0
        };
        Ok(Slot {
            bits: self.futures.allocate(FutureCell {
                function: usize::MAX,
                worker: None,
                io: None,
                ready_order,
                spawned: false,
                observed: false,
                request: None,
                cancel_request: None,
                permit: None,
                captures: Box::new([]),
                state,
                cancel_requested: false,
                cancel_delivered: false,
                changed: None,
                cancel_wake: None,
                cancel_permit: None,
            })?,
            tag: FUTURE,
        })
    }

    pub fn future_new(&mut self, function: usize, args: &[Slot]) -> Result<Slot, Error> {
        Ok(Slot {
            bits: self.futures.allocate(FutureCell {
                function,
                worker: None,
                io: None,
                ready_order: 0,
                spawned: false,
                observed: false,
                request: None,
                cancel_request: None,
                permit: None,
                captures: args.into(),
                state: State::Cold,
                cancel_requested: false,
                cancel_delivered: false,
                changed: None,
                cancel_wake: None,
                cancel_permit: None,
            })?,
            tag: FUTURE,
        })
    }
    pub fn future(&self, value: Slot) -> Result<&FutureCell, Error> {
        if value.tag != FUTURE {
            return Err(Error("expected Future handle".into()));
        }
        self.futures.get(value.bits)
    }
    pub(crate) fn future_subscribe(
        &mut self,
        value: Slot,
    ) -> Result<tokio::sync::watch::Receiver<()>, Error> {
        if matches!(
            self.future(value)?.state,
            State::Collect(_) | State::Stream(_)
        ) {
            return Ok(self
                .completion_signal
                .get_or_insert_with(|| tokio::sync::watch::channel(()).0)
                .subscribe());
        }
        let future = self.futures.get_mut(value.bits)?;
        Ok(future
            .changed
            .get_or_insert_with(|| tokio::sync::watch::channel(()).0)
            .subscribe())
    }
    pub fn future_state(&mut self, value: Slot, state: State) -> Result<(), Error> {
        self.future(value)?;
        let future = self.futures.get_mut(value.bits)?;
        if matches!(state, State::Ready(_)) && !matches!(future.state, State::Ready(_)) {
            self.completion_clock += 1;
            future.ready_order = self.completion_clock;
            if let Some(signal) = &self.completion_signal {
                signal.send_replace(());
            }
        }
        future.state = state;
        if matches!(state, State::Ready(_)) {
            future.worker = None;
            if let Some(io) = future.io.take() {
                io.cancel();
            }
            future.cancel_wake = None;
            future.cancel_permit = None;
            future.cancel_request = None;
            if let Some(channel) = future.request.take() {
                channel.cancel();
            }
            if let Some(permit) = future.permit.take() {
                permit.cancel();
            }
        }
        if let Some(changed) = &future.changed {
            changed.send_replace(());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completed_observed_spawned_futures_are_collectible() {
        let mut heap = Heap::new(&[], &[]);
        let mut handles = Vec::new();

        for _ in 0..32 {
            let handle = heap.future_new(0, &[]).unwrap();
            let future = heap.futures.get_mut(handle.bits).unwrap();
            future.spawned = true;
            future.observed = true;
            heap.spawned.push(handle);

            heap.future_state(handle, State::Ready(Ok(Slot::small(42))))
                .unwrap();
            handles.push(handle);
        }

        // No language references remain. These Rust handles are not GC roots.
        heap.collect(std::iter::empty::<Slot>()).unwrap();

        assert!(
            handles.iter().all(|&handle| heap.future(handle).is_err()),
            "completed observed tasks remain rooted by heap.spawned"
        );
    }

    #[test]
    fn spawned_gc_preserves_live_results_pending_tasks_and_unobserved_failures() {
        let mut heap = Heap::new(&[], &[]);
        let live = heap.future_new(0, &[]).unwrap();
        let pending = heap.future_new(0, &[]).unwrap();
        let unobserved = heap.future_new(0, &[]).unwrap();
        let observed = heap.future_new(0, &[]).unwrap();
        let forgotten = heap.future_new(0, &[]).unwrap();
        for handle in [live, pending, unobserved, observed, forgotten] {
            heap.futures.get_mut(handle.bits).unwrap().spawned = true;
            heap.spawned.push(handle);
        }
        let failure = heap
            .failure_new(crate::failure::FailureKind::Program("test failure".into()))
            .unwrap();
        heap.future_state(live, State::Ready(Ok(Slot::small(42))))
            .unwrap();
        heap.future_state(forgotten, State::Ready(Ok(Slot::small(0))))
            .unwrap();
        heap.future_state(unobserved, State::Ready(Err(failure)))
            .unwrap();
        heap.future_state(observed, State::Ready(Err(failure)))
            .unwrap();
        heap.futures.get_mut(observed.bits).unwrap().observed = true;

        heap.collect(std::iter::once(live)).unwrap();

        let State::Ready(Ok(value)) = heap.future(live).unwrap().state else {
            panic!("live Future lost its cached result");
        };
        assert_eq!(heap.integer(value).unwrap(), 42);
        assert!(heap.future(pending).is_ok());
        assert!(heap.future(unobserved).is_ok());
        assert!(heap.failure(failure).is_ok());
        assert!(heap.future(observed).is_err());
        assert!(heap.future(forgotten).is_err());
        assert_eq!(
            heap.spawned
                .iter()
                .map(|handle| handle.bits)
                .collect::<Vec<_>>(),
            vec![pending.bits, unobserved.bits]
        );
    }

    #[test]
    fn completion_notifies_all_waiters_even_before_their_first_poll() {
        let mut heap = Heap::new(&[], &[]);
        let future = heap.future_new(0, &[]).unwrap();
        let mut a = heap.future_subscribe(future).unwrap();
        let mut b = heap.future_subscribe(future).unwrap();
        assert!(!a.has_changed().unwrap());
        heap.future_state(future, State::Ready(Ok(Slot::small(42))))
            .unwrap();
        assert!(a.has_changed().unwrap());
        assert!(b.has_changed().unwrap());
        a.borrow_and_update();
        b.borrow_and_update();
        assert!(!a.has_changed().unwrap());
        assert!(!b.has_changed().unwrap());
    }

    #[test]
    fn future_identity_is_independent_of_cached_result_and_retains_captures() {
        let mut heap = Heap::new(&[], &[]);
        let child = heap.object_new(0, [Slot::small(42)].into_iter()).unwrap();
        let first = heap.future_new(0, &[child]).unwrap();
        let second = heap.future_new(0, &[child]).unwrap();
        assert!(heap.equal(first, first).unwrap());
        assert!(!heap.equal(first, second).unwrap());
        heap.future_state(first, State::Ready(Ok(Slot::small(7))))
            .unwrap();
        heap.future_state(second, State::Ready(Ok(Slot::small(7))))
            .unwrap();
        assert!(!heap.equal(first, second).unwrap());
        heap.collect([first].into_iter()).unwrap();
        assert_eq!(heap.object_get(child, 0).unwrap().bits, 42);
        assert!(heap.future(second).is_err());
    }
}

#[cfg(test)]
mod cached_tests {
    use super::*;
    #[test]
    fn receiver_cache_keeps_the_same_future_alive_but_does_not_leak_cycles() {
        let mut heap = Heap::new(&[], &[]);
        let receiver = heap.object_new(0, std::iter::empty()).unwrap();
        let future = heap.future_new(1, &[receiver]).unwrap();
        heap.cached_futures
            .entry(receiver.bits)
            .or_default()
            .push((1, future));
        heap.collect([receiver].into_iter()).unwrap();
        assert!(heap.future(future).is_ok());
        heap.collect([future].into_iter()).unwrap();
        assert!(heap.object_descriptor(receiver).is_ok());
        heap.collect(std::iter::empty()).unwrap();
        assert!(heap.cached_futures.is_empty());
        assert!(heap.future(future).is_err());
        assert!(heap.object_descriptor(receiver).is_err());
    }
}
