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

/// How long the scheduler spins for a pending native reply before sleeping.
const SPIN_MICROS: u64 = 50;
/// How often a waiting scheduler looks at the interrupt flag.
const INTERRUPT_LOOK: std::time::Duration = std::time::Duration::from_millis(20);

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
    /// The reactor and the local tasks of providers (`spawn_local`: process
    /// reapers and capture pipes); both progress while the scheduler waits.
    tokio: Option<(tokio::runtime::Runtime, tokio::task::LocalSet)>,
    shared: Arc<Shared>,
    /// Native request of each waiting driver frame (frames are roots).
    pub requests: HashMap<Word, u64>,
}

impl Io {
    fn reactor(&mut self) -> &(tokio::runtime::Runtime, tokio::task::LocalSet) {
        self.tokio.get_or_insert_with(|| {
            let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("Tokio runtime");
            (runtime, tokio::task::LocalSet::new())
        })
    }

    pub fn tokio(&mut self) -> &tokio::runtime::Runtime {
        &self.reactor().0
    }

    /// The set `spawn_local` of providers puts its tasks into (see `enter`).
    pub fn local(&mut self) -> &tokio::task::LocalSet {
        &self.reactor().1
    }

    /// Turn the reactor and run local tasks until `future` completes.
    pub fn block_on<F: std::future::Future>(&mut self, future: F) -> F::Output {
        let (runtime, local) = self.reactor();
        runtime.block_on(local.run_until(future))
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
        // Native replies (SQLite workers) usually arrive within microseconds:
        // spin briefly before paying for a sleep and a wake-up. Sockets make
        // progress only when this thread turns the reactor, so every 64 turns
        // the spin also polls it without blocking.
        if !self.io.requests.is_empty() {
            let spin = std::time::Instant::now();
            let mut turns = 0u32;
            while !self.io.shared.pending.load(Ordering::Acquire) {
                std::hint::spin_loop();
                turns += 1;
                // Reading the clock costs more than a turn; look every 64.
                if turns.is_multiple_of(64) {
                    self.io.block_on(tokio::task::yield_now());
                    if spin.elapsed() >= std::time::Duration::from_micros(SPIN_MICROS) {
                        break;
                    }
                }
            }
        }
        if !self.io.shared.pending.load(Ordering::Acquire) {
            self.count(|_| "scheduler idle wait".into());
            let started = std::time::Instant::now();
            let shared = self.io.shared.clone();
            // An interruptible run wakes to look at its interrupt flag.
            let earliest = match self.limits.interrupt {
                Some(_) => {
                    let look = std::time::Instant::now() + INTERRUPT_LOOK;
                    Some(earliest.map_or(look, |deadline| deadline.min(look)))
                }
                None => earliest,
            };
            self.io.block_on(async move {
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
            if let Some(stats) = &mut self.stats {
                *stats.entry("scheduler idle wait, microseconds".into()).or_default() += started.elapsed().as_micros() as u64;
            }
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
