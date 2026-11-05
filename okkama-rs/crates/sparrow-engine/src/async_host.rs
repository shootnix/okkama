//! Tokio owns the executor, reactor and timers. This bridge does not implement
//! language Future cells, cancellation or native-resource shutdown by itself.
#[cfg(test)]
use crate::Outcome;
use crate::execution::TaskOutcome;
use crate::{Error, Executable, Integer, Session};
use std::future::Future;

pub(crate) type StartupWait =
    tokio::sync::oneshot::Receiver<Option<tokio::sync::oneshot::Sender<()>>>;
type Startup = tokio::sync::oneshot::Sender<Option<tokio::sync::oneshot::Sender<()>>>;

/// Single-thread host for the VM's non-Send heap values. No coroutine stacks.
#[derive(Debug)]
pub struct AsyncHost {
    local: Option<tokio::task::LocalSet>,
    runtime: Option<tokio::runtime::Runtime>,
}
impl AsyncHost {
    /// Attach to `tokio::runtime::Builder::on_thread_park` when embedding
    /// `run_async` in a caller-owned current-thread runtime. Tokio's idle
    /// notification diagnoses VM waits without runnable or external work.
    /// `AsyncHost::new` installs this hook automatically.
    pub fn check_stalled_tasks() {
        crate::stall::on_idle();
    }

    pub fn new() -> Result<Self, Error> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .on_thread_park(Self::check_stalled_tasks)
            .max_blocking_threads(32)
            .build()
            .map_err(|e| Error(format!("cannot create Tokio runtime: {e}")))?;
        Ok(Self {
            local: Some(tokio::task::LocalSet::new()),
            runtime: Some(runtime),
        })
    }
    /// Borrowed !Send futures run on this thread. Re-entering a Tokio runtime is
    /// rejected rather than relying on Tokio's nested block_on panic.
    pub fn block_on<F: Future>(&self, future: F) -> Result<F::Output, Error> {
        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(Error("cannot block_on from an active Tokio runtime".into()));
        }
        Ok(self
            .local
            .as_ref()
            .expect("live host")
            .block_on(self.runtime.as_ref().expect("live host"), future))
    }
    pub fn run(&self, code: &Executable) -> Result<Integer, Error> {
        self.block_on(code.run_async())?
    }
}
impl Drop for AsyncHost {
    fn drop(&mut self) {
        // Rust-host shutdown only. Language cancellation must finish VM cleanup
        // before this point; dropping a Tokio task is not Okkama cancellation.
        drop(self.local.take());
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}
/// Longest uninterrupted run of instruction slices before a Tokio turn.
const YIELD_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1);

impl Session {
    /// Cooperatively drive a session using Tokio's ordinary Future/Waker path.
    /// The finite budget also bounds uninterrupted native/JIT instruction loops.
    pub async fn complete_async(&mut self) -> Result<Integer, Error> {
        let value = self.complete_slot().await?;
        self.outcome_integer(value)
    }
    pub(crate) async fn complete_slot(&mut self) -> Result<crate::value::Slot, Error> {
        // A private LocalSet also supports callers embedded in an ordinary Tokio
        // current-thread runtime. All spawned VM tasks are joined semantically
        // (including cleanup) before this scope is released.
        self.local_set()
            .run_until(async {
                let root = self.drive_task(None).await;
                let shutdown = self.shutdown_tasks().await;
                let outcome = root?;
                if let Some(failure) = shutdown? {
                    return self.describe_terminal_failure(failure).await;
                }
                match outcome {
                    TaskOutcome::Returned(value) => Ok(value),
                    TaskOutcome::Failed(failure) => self.describe_terminal_failure(failure).await,
                    _ => Err(Error("incomplete root task".into())),
                }
            })
            .await
    }

    async fn describe_terminal_failure(
        &mut self,
        failure: crate::value::Slot,
    ) -> Result<crate::value::Slot, Error> {
        self.describe_failure(failure)?;
        let description = self.drive_task(None).await;
        // Error.text is ordinary language code and may spawn tasks. Run their
        // cleanup too, retaining the primary diagnostic if either stage fails.
        let shutdown = self.shutdown_tasks().await;
        description?;
        shutdown?;
        Err(Error("failure description unexpectedly returned".into()))
    }

    async fn shutdown_tasks(&mut self) -> Result<Option<crate::value::Slot>, Error> {
        loop {
            if let Some(handle) = self.cancel_spawned()? {
                // The waiter roots this handle. Do not retain a list snapshot or
                // index across await: GC may prune completed registrations, and
                // cleanup may spawn more tasks.
                let mut waiter = self.future_session(handle, true)?;
                waiter.drive_task(None).await?;
            } else {
                let failure = self.unobserved_spawned_failure()?;
                // No GC safepoint occurs before complete_slot describes it.
                self.clear_spawned();
                return Ok(failure);
            }
        }
    }

    fn drive_task(
        &mut self,
        mut started: Option<Startup>,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<TaskOutcome, Error>> + '_>> {
        Box::pin(async move {
            let activity = crate::stall::Driver::new(self.driver_group());
            let mut slice = std::time::Instant::now();
            loop {
                activity.running();
                // Ready spawn/notification transitions can return before the
                // VM instruction budget expires. Charge those turns to Tokio's
                // cooperative budget too, without signalling language suspension
                // or releasing an eager-start handshake.
                tokio::task::coop::consume_budget().await;
                if let Some(wait) = self.startup_wait() {
                    let release = wait
                        .await
                        .map_err(|_| Error("child task stopped during startup".into()))?;
                    self.clear_startup();
                    if let Some(release) = release {
                        let _ = release.send(());
                    }
                }
                let outcome = match self.resume_task(4096) {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        if let Some(started) = started.take() {
                            let _ = started.send(None);
                        }
                        return Err(error);
                    }
                };
                match outcome {
                    TaskOutcome::Returned(_) | TaskOutcome::Failed(_) => {
                        if let Some(started) = started.take() {
                            let _ = started.send(None);
                        }
                        return Ok(outcome);
                    }
                    TaskOutcome::Yielded => {
                        // Each Tokio turn of a current-thread runtime polls the
                        // I/O driver (a system call); yielding after every
                        // instruction slice spent half of the time there. Run
                        // slices for up to YIELD_INTERVAL; the cooperative budget
                        // above still forces a turn after 128 slices.
                        if slice.elapsed() >= YIELD_INTERVAL {
                            tokio::task::yield_now().await;
                            slice = std::time::Instant::now();
                        }
                    }
                    TaskOutcome::Waiting => {
                        if let Some(child) = self.take_spawn()? {
                            if let Some(mut child) = child {
                                let (ready, wait) = tokio::sync::oneshot::channel();
                                tokio::task::spawn_local(async move {
                                    if let Err(error) = child.drive_task(Some(ready)).await {
                                        let _ = child.fail_child(error);
                                    }
                                });
                                self.set_startup(wait);
                            }
                            continue;
                        }
                        let crate::execution::PendingWait {
                            future,
                            worker,
                            io,
                            deadline,
                            mut event,
                            permit,
                            mut changed,
                            mut cancelled,
                        } = self.pending_wait()?;
                        if tokio::runtime::Handle::try_current().is_err() {
                            return Err(Error(
                                "pending native I/O requires a Tokio runtime".into(),
                            ));
                        }
                        if let Some(started) = started.take() {
                            let (release, resume) = tokio::sync::oneshot::channel();
                            let _ = started.send(Some(release));
                            // The parent resumes before this child can deliver an
                            // already-expired timer. Subscription above also keeps
                            // cancellation between startup and release observable.
                            resume
                                .await
                                .map_err(|_| Error("spawn parent stopped during startup".into()))?;
                        }
                        // Subscribe before yielding, so cancellation/completion cannot
                        // race registration. Tokio watch wakes every awaiting task.
                        let external = deadline.is_some() || worker.is_some() || io.is_some();
                        let mut timer = std::pin::pin!(async {
                            if let Some(io) = io {
                                std::future::poll_fn(|cx| io.poll(cx)).await;
                            } else if let Some(worker) = worker {
                                let _ = std::future::poll_fn(|cx| worker.poll(cx)).await;
                            } else if let Some(permit) = permit {
                                return std::future::poll_fn(|cx| permit.poll(cx)).await.map(Some);
                            } else if let Some(event) = &mut event {
                                loop {
                                    if *event.borrow_and_update() {
                                        break;
                                    }
                                    if event.changed().await.is_err() {
                                        break;
                                    }
                                }
                            } else if let Some(deadline) = deadline {
                                tokio::time::sleep_until(tokio::time::Instant::from_std(deadline))
                                    .await;
                            } else {
                                std::future::pending::<()>().await;
                            }
                            Ok(None)
                        });
                        let mut notification = std::pin::pin!(changed.changed());
                        let mut cancellation = std::pin::pin!(cancelled.changed());
                        let expired = std::future::poll_fn(|cx| {
                            activity.running();
                            if activity.take_stalled() {
                                return std::task::Poll::Ready(Ok((3, None)));
                            }
                            if cancellation.as_mut().poll(cx).is_ready() {
                                return std::task::Poll::Ready(Ok((2, None)));
                            }
                            if notification.as_mut().poll(cx).is_ready() {
                                return std::task::Poll::Ready(Ok((0, None)));
                            }
                            let result = timer
                                .as_mut()
                                .poll(cx)
                                .map(|result| result.map(|permit| (1, permit)));
                            if result.is_pending() {
                                activity.pending(external, cx.waker());
                            }
                            result
                        })
                        .await?;
                        if expired.0 == 3 {
                            self.fail_stalled_wait()?;
                        } else if expired.0 == 1 {
                            self.settle_pending(future, expired.1)?;
                        } else if expired.0 == 2 {
                            self.wake_cancellation();
                        }
                    }
                }
            }
        })
    }
}
impl Executable {
    pub async fn run_async(&self) -> Result<Integer, Error> {
        self.start().complete_async().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_tokio_can_install_the_same_idle_diagnostic_hook() {
        let program =
            okkama_sparrow::assemble(include_str!("../tests/fixtures/stalled-event.asm")).unwrap();
        let code = Executable::new(&program, crate::Mode::Cold).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .on_thread_park(AsyncHost::check_stalled_tasks)
            .build()
            .unwrap();
        let error = runtime.block_on(code.run_async()).unwrap_err();
        assert!(error.0.contains("no task can make progress"));
    }

    #[test]
    fn abandoning_a_host_wait_preserves_the_vm_deadline_and_long_timers_do_not_panic() {
        let mut p =
            okkama_sparrow::assemble(include_str!("../tests/fixtures/async-timer.asm")).unwrap();
        p.functions[0].code[0] = okkama_sparrow::Instruction::Const {
            dst: okkama_sparrow::Dst(okkama_sparrow::Register(0)),
            value: okkama_sparrow::Constant::Str(i64::MAX.to_string()),
        };
        for mode in [crate::Mode::Cold, crate::Mode::Jit] {
            let code = Executable::new(&p, mode).unwrap();
            let mut session = code.start();
            assert_eq!(session.resume(4096).unwrap(), Outcome::Waiting);
            let (_, before) = session.pending_timer().unwrap();
            let host = AsyncHost::new().unwrap();
            host.block_on(async {
                assert!(
                    tokio::time::timeout(
                        std::time::Duration::from_millis(1),
                        session.complete_async()
                    )
                    .await
                    .is_err()
                );
            })
            .unwrap();
            assert_eq!(session.resume(1).unwrap(), Outcome::Waiting);
            let (_, after) = session.pending_timer().unwrap();
            assert_eq!(before, after);
        }
    }
}
