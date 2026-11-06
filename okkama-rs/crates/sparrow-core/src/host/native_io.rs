//! Heap-free Rust futures polled by Tokio. Publishing a completed result is a
//! separate synchronous step; no VM heap borrow can survive an I/O suspension.
use crate::host::*;
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
pub(crate) type ResultValue = Result<Box<dyn Output>, NativeFailure>;
pub(crate) type Body = Pin<Box<dyn Future<Output = ResultValue>>>;
pub(crate) type Cleanup = Pin<Box<dyn Future<Output = Result<(), NativeFailure>>>>;
pub(crate) type CancelHook = Box<dyn FnOnce() -> Cleanup>;
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
        let request = Request::new(body);
        *request.cancel_hook.borrow_mut() = cleanup;
        let slot = self.future_with_state(State::Io)?;
        self.futures.get_mut(slot.bits)?.io = Some(request);
        Ok(slot)
    }
    /// Publish a completed request into its Future's state.
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
                    self.failure_link(cancelled, secondary);
                }
                Err(cancelled)
            }
        };
        let cell = self.futures.get_mut(future.bits)?;
        cell.state = State::Ready(value);
        cell.io = None;
        Ok(())
    }
}
