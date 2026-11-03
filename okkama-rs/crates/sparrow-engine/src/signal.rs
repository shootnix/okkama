//! Native one-shot operations share cancellation, tracing and Tokio watch wakes.
//! The enum adds no allocation around each provider's own retained request.
use crate::{
    Error,
    value::{Heap, Slot},
};
use std::rc::{Rc, Weak};
#[derive(Debug, Clone)]
pub(crate) enum Failure {
    Descriptor(u32),
    Existing(Slot),
}
#[derive(Debug, Clone)]
pub(crate) enum Request {
    Stream(Rc<crate::stream::Request>),
    Channel(Rc<crate::channel::Request>),
    Barrier(Rc<crate::barrier::Request>),
}
#[derive(Debug, Clone)]
pub(crate) enum WeakRequest {
    Stream(Weak<crate::stream::Request>),
    Channel(Weak<crate::channel::Request>),
    Barrier(Weak<crate::barrier::Request>),
}
impl Request {
    pub fn cancel(&self) {
        match self {
            Self::Stream(r) => r.cancel(),
            Self::Channel(r) => r.cancel(),
            Self::Barrier(r) => r.cancel(),
        }
    }
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<bool> {
        match self {
            Self::Stream(r) => r.subscribe(),
            Self::Channel(r) => r.subscribe(),
            Self::Barrier(r) => r.subscribe(),
        }
    }
    pub fn roots(&self, roots: &mut Vec<Slot>) {
        match self {
            Self::Stream(r) => r.roots(roots),
            Self::Channel(r) => r.roots(roots),
            Self::Barrier(r) => r.roots(roots),
        }
    }
    pub fn result(&self) -> Option<Result<Slot, Failure>> {
        match self {
            Self::Stream(r) => r.result().map(|r| r.map_err(Failure::Existing)),
            Self::Channel(r) => r.result().map(|r| r.map_err(Failure::Descriptor)),
            Self::Barrier(r) => r.result().map(|r| r.map_err(Failure::Descriptor)),
        }
    }
    pub fn downgrade(&self) -> WeakRequest {
        match self {
            Self::Stream(r) => WeakRequest::Stream(Rc::downgrade(r)),
            Self::Channel(r) => WeakRequest::Channel(Rc::downgrade(r)),
            Self::Barrier(r) => WeakRequest::Barrier(Rc::downgrade(r)),
        }
    }
}
impl WeakRequest {
    pub fn cancel(&self) {
        match self {
            Self::Stream(r) => {
                if let Some(r) = r.upgrade() {
                    r.cancel();
                }
            }
            Self::Barrier(r) => {
                if let Some(r) = r.upgrade() {
                    r.cancel();
                }
            }
            Self::Channel(r) => {
                if let Some(r) = r.upgrade() {
                    r.cancel();
                }
            }
        }
    }
}

impl Heap {
    pub(crate) fn settle_signal(&mut self, future: Slot) -> Result<(), Error> {
        let request = self.future(future)?.request.as_ref().cloned();
        if let Some(result) = request.and_then(|r| r.result()) {
            let result = match result {
                Ok(value) => Ok(value),
                Err(Failure::Existing(failure)) => Err(failure),
                Err(Failure::Descriptor(owner)) => {
                    let error = self.object_new(owner, std::iter::empty())?;
                    Err(self.failure_new(crate::failure::FailureKind::Thrown(error))?)
                }
            };
            self.future_state(future, crate::future::State::Ready(result))?;
        }
        Ok(())
    }
}
