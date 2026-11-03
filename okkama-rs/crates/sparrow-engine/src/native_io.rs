//! Heap-free Rust futures polled by Tokio. Publishing a completed result is a
//! separate synchronous step; no VM heap borrow can survive an I/O suspension.
use crate::{
    Error,
    failure::FailureKind,
    future::State,
    value::{Heap, Slot},
};
use okkama_runtime_abi::NativeFailure;
use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

/// Implementations may own Rust data/resources, never untraced VM Slots.
pub(crate) trait Output {
    fn publish(self: Box<Self>, heap: &mut Heap) -> Result<Slot, Error>;
}
type ResultValue = Result<Box<dyn Output>, NativeFailure>;
type Body = Pin<Box<dyn Future<Output = ResultValue>>>;
type Cleanup = Pin<Box<dyn Future<Output = Result<(), NativeFailure>>>>;
type CancelHook = Box<dyn FnOnce() -> Cleanup>;
enum Completion {
    Value(ResultValue),
    Cancelled(Result<(), NativeFailure>),
}

pub(crate) struct Request {
    body: RefCell<Option<Body>>,
    result: RefCell<Option<Completion>>,
    cancel_hook: RefCell<Option<CancelHook>>,
    cleanup: RefCell<Option<Cleanup>>,
    cancelling: Cell<bool>,
    finished: Cell<bool>,
    waiters: RefCell<Vec<Waker>>,
}
impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeIoRequest")
            .field("finished", &self.finished.get())
            .finish()
    }
}
impl Request {
    fn new<F, T>(future: F) -> Rc<Self>
    where
        F: Future<Output = Result<T, NativeFailure>> + 'static,
        T: Output + 'static,
    {
        Rc::new(Self {
            body: RefCell::new(Some(Box::pin(async move {
                future.await.map(|v| Box::new(v) as Box<dyn Output>)
            }))),
            result: RefCell::new(None),
            cancel_hook: RefCell::new(None),
            cleanup: RefCell::new(None),
            cancelling: Cell::new(false),
            finished: Cell::new(false),
            waiters: RefCell::new(Vec::new()),
        })
    }
    pub fn poll(&self, cx: &mut Context<'_>) -> Poll<()> {
        if self.finished.get() {
            return Poll::Ready(());
        }
        let result = if self.cancelling.get() {
            self.cleanup
                .borrow_mut()
                .as_mut()
                .expect("native cancellation cleanup")
                .as_mut()
                .poll(cx)
                .map(Completion::Cancelled)
        } else {
            self.body
                .borrow_mut()
                .as_mut()
                .expect("pending native I/O")
                .as_mut()
                .poll(cx)
                .map(Completion::Value)
        };
        match result {
            Poll::Pending => {
                let mut waiters = self.waiters.borrow_mut();
                if !waiters.iter().any(|w| w.will_wake(cx.waker())) {
                    waiters.push(cx.waker().clone());
                }
                Poll::Pending
            }
            Poll::Ready(value) => {
                self.body.borrow_mut().take();
                self.cleanup.borrow_mut().take();
                *self.result.borrow_mut() = Some(value);
                self.finished.set(true);
                for waker in self.waiters.borrow_mut().drain(..) {
                    waker.wake();
                }
                Poll::Ready(())
            }
        }
    }
    /// Returns true when the language Future must remain pending until cleanup.
    pub fn request_cancel(&self) -> bool {
        if self.cancelling.get() {
            return true;
        }
        let hook = self.cancel_hook.borrow_mut().take();
        if let Some(hook) = hook {
            self.body.borrow_mut().take();
            self.result.borrow_mut().take();
            self.cancelling.set(true);
            self.finished.set(false);
            *self.cleanup.borrow_mut() = Some(hook());
            for waker in self.waiters.borrow_mut().drain(..) {
                waker.wake();
            }
            true
        } else {
            self.cancel();
            false
        }
    }
    pub fn cancel(&self) {
        self.cancel_hook.borrow_mut().take();
        self.cleanup.borrow_mut().take();
        self.body.borrow_mut().take();
        self.result.borrow_mut().take();
        self.finished.set(true);
        for waker in self.waiters.borrow_mut().drain(..) {
            waker.wake();
        }
    }
}
impl Heap {
    pub(crate) fn native_io<F, T>(&mut self, body: F) -> Result<Slot, Error>
    where
        F: Future<Output = Result<T, NativeFailure>> + 'static,
        T: Output + 'static,
    {
        self.native_io_with_cleanup(body, None)
    }
    pub(crate) fn native_io_with_cleanup<F, T>(
        &mut self,
        body: F,
        cleanup: Option<CancelHook>,
    ) -> Result<Slot, Error>
    where
        F: Future<Output = Result<T, NativeFailure>> + 'static,
        T: Output + 'static,
    {
        let slot = self.future_with_state(State::Io)?;
        let request = Request::new(body);
        *request.cancel_hook.borrow_mut() = cleanup;
        self.futures.get_mut(slot.bits)?.io = Some(request);
        Ok(slot)
    }
    pub(crate) fn native_io_settle(&mut self, future: Slot) -> Result<(), Error> {
        if !matches!(self.future(future)?.state, State::Io) {
            return Ok(());
        }
        let request = self
            .future(future)?
            .io
            .as_ref()
            .ok_or_else(|| Error("missing native I/O request".into()))?
            .clone();
        let result = request
            .result
            .borrow_mut()
            .take()
            .ok_or_else(|| Error("native I/O has not completed".into()))?;
        let value = match result {
            Completion::Value(Ok(output)) => match output.publish(self) {
                Ok(value) => Ok(value),
                // Publishing consumes the Rust output. Cache its failure in the
                // language Future too; leaving State::Io would make subsequent
                // waiters try to consume a result that no longer exists.
                Err(error) => Err(self.failure_new(FailureKind::Program(error.0))?),
            },
            Completion::Value(Err(error)) => {
                let payload = self.native_failure_new(error)?;
                Err(self.failure_new(FailureKind::Native(payload))?)
            }
            Completion::Cancelled(cleanup) => {
                let cancelled = self.failure_new(FailureKind::Cancelled)?;
                if let Err(error) = cleanup {
                    let payload = self.native_failure_new(error)?;
                    let secondary = self.failure_new(FailureKind::Native(payload))?;
                    self.failure_link(cancelled, secondary, true)?;
                }
                Err(cancelled)
            }
        };
        self.future_state(future, State::Ready(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Number(i64);
    impl Output for Number {
        fn publish(self: Box<Self>, _: &mut Heap) -> Result<Slot, Error> {
            Ok(Slot::small(self.0))
        }
    }
    struct DropCount(Rc<Cell<usize>>);
    impl Drop for DropCount {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    fn poll_once(request: &Request) -> Poll<()> {
        request.poll(&mut Context::from_waker(Waker::noop()))
    }

    #[test]
    fn publication_error_is_cached_as_a_terminal_language_failure() {
        struct InvalidOutput;
        impl Output for InvalidOutput {
            fn publish(self: Box<Self>, _: &mut Heap) -> Result<Slot, Error> {
                Err(Error("cannot publish native result".into()))
            }
        }
        let mut heap = Heap::new(&[], &[]);
        let future = heap.native_io(async { Ok(InvalidOutput) }).unwrap();
        let request = heap.future(future).unwrap().io.clone().unwrap();
        assert!(poll_once(&request).is_ready());
        heap.native_io_settle(future).unwrap();
        let State::Ready(Err(failure)) = heap.future(future).unwrap().state else {
            panic!("publication failure left the Future pending");
        };
        assert!(matches!(&heap.failure(failure).unwrap().kind,
            FailureKind::Program(message) if message == "cannot publish native result"));
        heap.collect(std::iter::once(future)).unwrap();
        heap.native_io_settle(future).unwrap();
        assert!(matches!(heap.future(future).unwrap().state,
            State::Ready(Err(cached)) if cached.bits == failure.bits));
        assert!(heap.future(future).unwrap().io.is_none());
    }

    #[test]
    fn cancellation_before_publication_discards_output_and_runs_cleanup_once() {
        struct OwnedOutput(DropCount);
        impl Output for OwnedOutput {
            fn publish(self: Box<Self>, _: &mut Heap) -> Result<Slot, Error> {
                let _ = &self.0;
                panic!("cancelled native output must not be published");
            }
        }
        let mut heap = Heap::new(&[], &[]);
        let dropped = Rc::new(Cell::new(0));
        let owned = OwnedOutput(DropCount(dropped.clone()));
        let cleanups = Rc::new(Cell::new(0));
        let calls = cleanups.clone();
        let future = heap
            .native_io_with_cleanup(
                async move { Ok(owned) },
                Some(Box::new(move || {
                    calls.set(calls.get() + 1);
                    Box::pin(async { Ok(()) })
                })),
            )
            .unwrap();
        let request = heap.future(future).unwrap().io.clone().unwrap();
        assert!(poll_once(&request).is_ready());
        assert_eq!(dropped.get(), 0);
        for _ in 0..2 {
            heap.future_control(okkama_sparrow::FutOp::Cancel, future)
                .unwrap();
        }
        assert_eq!(dropped.get(), 1);
        assert_eq!(cleanups.get(), 1);
        assert!(matches!(heap.future(future).unwrap().state, State::Io));
        assert!(poll_once(&request).is_ready());
        heap.native_io_settle(future).unwrap();
        let State::Ready(Err(failure)) = heap.future(future).unwrap().state else {
            panic!("cancellation was not published");
        };
        assert!(matches!(
            heap.failure(failure).unwrap().kind,
            FailureKind::Cancelled
        ));
        heap.native_io_settle(future).unwrap();
        heap.future_control(okkama_sparrow::FutOp::Cancel, future)
            .unwrap();
        assert_eq!(cleanups.get(), 1);
    }

    #[test]
    fn cancellation_after_publication_preserves_value_without_cleanup() {
        let mut heap = Heap::new(&[], &[]);
        let cleanups = Rc::new(Cell::new(0));
        let calls = cleanups.clone();
        let future = heap
            .native_io_with_cleanup(
                async { Ok(Number(42)) },
                Some(Box::new(move || {
                    calls.set(calls.get() + 1);
                    Box::pin(async { Ok(()) })
                })),
            )
            .unwrap();
        let request = heap.future(future).unwrap().io.clone().unwrap();
        assert!(poll_once(&request).is_ready());
        heap.native_io_settle(future).unwrap();
        heap.future_control(okkama_sparrow::FutOp::Cancel, future)
            .unwrap();
        heap.collect(std::iter::once(future)).unwrap();
        assert!(matches!(
            heap.future(future).unwrap().state,
            State::Ready(Ok(Slot { bits: 42, .. }))
        ));
        assert_eq!(cleanups.get(), 0);
    }

    #[test]
    fn completion_wakes_each_registered_waiter_once() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        struct Counter(AtomicUsize);
        impl std::task::Wake for Counter {
            fn wake(self: Arc<Self>) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        let ready = Rc::new(Cell::new(false));
        let state = ready.clone();
        let request = Request::new(std::future::poll_fn(move |_| {
            if state.get() {
                Poll::Ready(Ok(Number(42)))
            } else {
                Poll::Pending
            }
        }));
        let a = Arc::new(Counter(AtomicUsize::new(0)));
        let b = Arc::new(Counter(AtomicUsize::new(0)));
        let aw = Waker::from(a.clone());
        let bw = Waker::from(b.clone());
        assert!(request.poll(&mut Context::from_waker(&aw)).is_pending());
        assert!(request.poll(&mut Context::from_waker(&aw)).is_pending());
        assert!(request.poll(&mut Context::from_waker(&bw)).is_pending());
        ready.set(true);
        assert!(poll_once(&request).is_ready());
        assert!(poll_once(&request).is_ready());
        assert_eq!(a.0.load(Ordering::Relaxed), 1);
        assert_eq!(b.0.load(Ordering::Relaxed), 1);
        assert!(request.waiters.borrow().is_empty());
    }
    #[test]
    fn cancellation_before_poll_drops_owned_resources_without_starting_io() {
        let mut heap = Heap::new(&[], &[]);
        let started = Rc::new(Cell::new(false));
        let dropped = Rc::new(Cell::new(0));
        let owned = DropCount(dropped.clone());
        let did_start = started.clone();
        let future = heap
            .native_io(async move {
                let _owned = owned;
                did_start.set(true);
                std::future::pending::<()>().await;
                Ok(Number(42))
            })
            .unwrap();
        heap.future_control(okkama_sparrow::FutOp::Cancel, future)
            .unwrap();
        assert!(!started.get());
        assert_eq!(dropped.get(), 1);
        assert!(matches!(
            heap.future(future).unwrap().state,
            State::Ready(Err(_))
        ));
    }
    #[test]
    fn heap_can_collect_during_io_and_publishes_completion_only_once() {
        let host = crate::AsyncHost::new().unwrap();
        host.block_on(async {
            let mut heap = Heap::new(&[], &[]);
            let future = heap
                .native_io(async {
                    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                    Ok(Number(42))
                })
                .unwrap();
            let request = heap.future(future).unwrap().io.clone().unwrap();
            heap.collect([future].into_iter()).unwrap();
            std::future::poll_fn(|cx| request.poll(cx)).await;
            heap.collect([future].into_iter()).unwrap();
            heap.native_io_settle(future).unwrap();
            heap.native_io_settle(future).unwrap();
            assert!(matches!(
                heap.future(future).unwrap().state,
                State::Ready(Ok(Slot { bits: 42, .. }))
            ));
            assert!(heap.future(future).unwrap().io.is_none());
        })
        .unwrap();
    }
    #[test]
    fn native_cancel_waits_for_cleanup_once_and_preserves_secondary_failure() {
        crate::AsyncHost::new()
            .unwrap()
            .block_on(async {
                let mut heap = Heap::new(&[], &[]);
                let calls = Rc::new(Cell::new(0));
                let calls_after = calls.clone();
                let (release, wait) = tokio::sync::oneshot::channel();
                let future = heap
                    .native_io_with_cleanup(
                        std::future::pending::<Result<Number, NativeFailure>>(),
                        Some(Box::new(move || {
                            calls_after.set(calls_after.get() + 1);
                            Box::pin(async move {
                                wait.await.unwrap();
                                Err(NativeFailure {
                                    domain: "core.process".into(),
                                    code: "unexpected".into(),
                                    operation: "reap".into(),
                                    message: "test cleanup failure".into(),
                                })
                            })
                        })),
                    )
                    .unwrap();
                for _ in 0..2 {
                    heap.future_control(okkama_sparrow::FutOp::Cancel, future)
                        .unwrap();
                }
                assert_eq!(calls.get(), 1);
                assert!(matches!(heap.future(future).unwrap().state, State::Io));
                let request = heap.future(future).unwrap().io.clone().unwrap();
                assert!(
                    tokio::time::timeout(
                        std::time::Duration::from_millis(1),
                        std::future::poll_fn(|cx| request.poll(cx))
                    )
                    .await
                    .is_err()
                );
                heap.collect([future].into_iter()).unwrap();
                release.send(()).unwrap();
                std::future::poll_fn(|cx| request.poll(cx)).await;
                heap.native_io_settle(future).unwrap();
                let State::Ready(Err(failure)) = heap.future(future).unwrap().state else {
                    panic!("not cancelled")
                };
                let record = heap.failure(failure).unwrap();
                assert!(matches!(record.kind, FailureKind::Cancelled));
                assert_eq!(record.secondary.len(), 1);
                let FailureKind::Native(payload) = heap.failure(record.secondary[0]).unwrap().kind
                else {
                    panic!("cleanup detail missing")
                };
                assert_eq!(heap.native_failure(payload).unwrap().operation, "reap");
                heap.future_control(okkama_sparrow::FutOp::Cancel, future)
                    .unwrap();
                assert_eq!(calls.get(), 1);
            })
            .unwrap();
    }
    #[test]
    fn failure_publication_retains_native_domain_code_and_operation() {
        let host = crate::AsyncHost::new().unwrap();
        host.block_on(async {
            let mut heap = Heap::new(&[], &[]);
            let future = heap
                .native_io(async {
                    Err::<Number, _>(NativeFailure {
                        domain: "core.net".into(),
                        code: "not_found".into(),
                        operation: "lookup".into(),
                        message: "missing".into(),
                    })
                })
                .unwrap();
            let request = heap.future(future).unwrap().io.clone().unwrap();
            std::future::poll_fn(|cx| request.poll(cx)).await;
            heap.native_io_settle(future).unwrap();
            let State::Ready(Err(failure)) = heap.future(future).unwrap().state else {
                panic!("missing failure");
            };
            // Only the cached Future remains a root: its failure and native
            // domain/code/operation strings must survive through that edge.
            heap.collect(std::iter::once(future)).unwrap();
            let FailureKind::Native(payload) = heap.failure(failure).unwrap().kind else {
                panic!("wrong failure");
            };
            let report = heap.native_failure(payload).unwrap();
            assert_eq!(
                (
                    report.domain.as_str(),
                    report.code.as_str(),
                    report.operation.as_str()
                ),
                ("core.net", "not_found", "lookup")
            );
            heap.collect(std::iter::empty()).unwrap();
            assert!(heap.failure(failure).is_err());
            assert!(heap.native_failure(payload).is_err());
        })
        .unwrap();
    }
}
