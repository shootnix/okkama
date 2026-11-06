//! External waits: native I/O requests (Rust futures of the providers) and
//! timers. A driver polls its request with a waker that names its frame; the
//! waker may fire on any thread (SQLite workers reply through channels). When
//! no frame is ready the scheduler blocks in Tokio until a waker fires or the
//! earliest timer is due, then wakes those frames.
use crate::runtime::Runtime;
use crate::word::Word;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Wake, Waker};

#[derive(Debug, Default)]
struct Shared {
    woken: Mutex<Vec<Word>>,
    pending: AtomicBool,
    notify: tokio::sync::Notify,
}

#[derive(Debug)]
struct FrameWaker {
    frame: Word,
    shared: Arc<Shared>,
}
impl Wake for FrameWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.shared.woken.lock().expect("woken frames").push(self.frame);
        self.shared.pending.store(true, Ordering::Release);
        self.shared.notify.notify_one();
    }
}

#[derive(Debug, Default)]
pub struct Io {
    tokio: Option<tokio::runtime::Runtime>,
    shared: Arc<Shared>,
    /// Native request of each waiting driver frame (frames are roots).
    pub requests: HashMap<Word, u64>,
}

impl Io {
    pub fn tokio(&mut self) -> &tokio::runtime::Runtime {
        self.tokio.get_or_insert_with(|| {
            tokio::runtime::Builder::new_current_thread().enable_all().build().expect("Tokio runtime")
        })
    }

    pub fn waker(&self, frame: Word) -> Waker {
        Waker::from(Arc::new(FrameWaker { frame, shared: self.shared.clone() }))
    }

    fn take_woken(&self) -> Vec<Word> {
        if !self.shared.pending.swap(false, Ordering::Acquire) {
            return Vec::new();
        }
        std::mem::take(&mut *self.shared.woken.lock().expect("woken frames"))
    }
}

impl Runtime {
    /// Wake frames whose wakers fired, without blocking.
    pub fn poll_io(&mut self) {
        for frame in self.io.take_woken() {
            self.wake(frame);
        }
    }

    /// Block until a native request or a timer can make progress; false when
    /// nothing external is pending.
    pub fn wait_external(&mut self) -> bool {
        let earliest = self.timers.iter().map(|&(deadline, _)| deadline).min();
        if self.io.requests.is_empty() && earliest.is_none() {
            return false;
        }
        if !self.io.shared.pending.load(Ordering::Acquire) {
            let shared = self.io.shared.clone();
            self.io.tokio().block_on(async move {
                match earliest {
                    Some(deadline) => {
                        tokio::select! {
                            _ = shared.notify.notified() => {}
                            _ = tokio::time::sleep_until(deadline.into()) => {}
                        }
                    }
                    None => shared.notify.notified().await,
                }
            });
        }
        self.poll_io();
        let now = std::time::Instant::now();
        let mut due = Vec::new();
        self.timers.retain(|&(deadline, frame)| {
            let fired = deadline <= now;
            if fired {
                due.push((deadline, frame));
            }
            !fired
        });
        due.sort_by_key(|&(deadline, _)| deadline);
        for (_, frame) in due {
            self.wake(frame);
        }
        true
    }
}
