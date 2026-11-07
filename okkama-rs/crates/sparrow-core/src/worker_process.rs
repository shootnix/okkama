//! Process ownership and pipe I/O are delegated to Tokio. Cancellation kills and
//! reaps the child before the request becomes ready; dropping a VM waiter does
//! not abandon a running OS process.
use std::{
    cell::{Cell, RefCell},
    ffi::OsString,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

const DEFAULT_OUTPUT_LIMIT: usize = 64 * 1024 * 1024;

fn worker_command(mut command: Vec<OsString>) -> Vec<OsString> {
    if !command.is_empty() {
        // CLI switches must precede FILE.spb; everything after it belongs to
        // the language program. Appending this flag would run main recursively.
        command.insert(1, "--worker".into());
    }
    command
}

// Workers own their ordinary descendants. Drop also covers an abandoned host
// future; no detached pipe-reading tasks outlive this guard.
struct ProcessGroup {
    #[cfg(unix)]
    pid: Option<rustix::process::Pid>,
}
impl ProcessGroup {
    fn new(child: &tokio::process::Child) -> Self {
        Self {
            #[cfg(unix)]
            pid: child
                .id()
                .and_then(|id| rustix::process::Pid::from_raw(id as i32)),
        }
    }
    fn terminate(&mut self) {
        #[cfg(unix)]
        if let Some(pid) = self.pid.take() {
            let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
        }
    }
}
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        self.terminate();
    }
}

async fn read_limited(
    mut stream: impl AsyncRead + Unpin,
    remaining: &Cell<usize>,
) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        let count = stream.read(&mut buffer).await?;
        if count == 0 {
            return Ok(bytes);
        }
        let Some(left) = remaining.get().checked_sub(count) else {
            return Err(std::io::Error::other("worker output exceeds byte limit"));
        };
        remaining.set(left);
        bytes.extend_from_slice(&buffer[..count]);
    }
}

#[derive(Debug)]
pub(crate) struct Output {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub success: bool,
    pub code: Option<i32>,
}
#[derive(Debug)]
pub(crate) enum Failure {
    Cancelled,
    Process(String),
}
type Outcome = Result<Rc<Output>, Rc<Failure>>;
type Running = Pin<Box<dyn Future<Output = Outcome>>>;
pub(crate) struct Request {
    body: RefCell<Option<Running>>,
    result: RefCell<Option<Outcome>>,
    cancelled: tokio::sync::watch::Sender<bool>,
}
impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerRequest")
            .field("ready", &self.result.borrow().is_some())
            .finish()
    }
}
impl Request {
    pub fn new(
        command: Vec<OsString>,
        input: Vec<u8>,
        environment: Option<std::collections::BTreeMap<String, String>>,
    ) -> Rc<Self> {
        Self::from_command(worker_command(command), input, environment)
    }
    fn from_command(
        command: Vec<OsString>,
        input: Vec<u8>,
        environment: Option<std::collections::BTreeMap<String, String>>,
    ) -> Rc<Self> {
        let (cancelled, wait) = tokio::sync::watch::channel(false);
        Rc::new(Self {
            body: RefCell::new(Some(Box::pin(run(command, input, environment, wait)))),
            result: RefCell::new(None),
            cancelled,
        })
    }
    pub fn cancel(&self) {
        self.cancelled.send_replace(true);
    }
    pub fn poll(&self, cx: &mut Context<'_>) -> Poll<Outcome> {
        if let Some(result) = self.result.borrow().as_ref() {
            return Poll::Ready(result.clone());
        }
        let mut body = self.body.borrow_mut();
        let result = body.as_mut().expect("running worker").as_mut().poll(cx);
        if let Poll::Ready(value) = &result {
            *self.result.borrow_mut() = Some(value.clone());
            body.take();
        }
        result
    }
}
impl Drop for Request {
    fn drop(&mut self) {
        self.cancel();
    }
}
fn failed(message: impl ToString) -> Rc<Failure> {
    Rc::new(Failure::Process(message.to_string()))
}
async fn run(
    command: Vec<OsString>,
    input: Vec<u8>,
    environment: Option<std::collections::BTreeMap<String, String>>,
    cancelled: tokio::sync::watch::Receiver<bool>,
) -> Outcome {
    let output_limit = match std::env::var("SPARROW_WORKER_OUTPUT_LIMIT_BYTES") {
        Ok(value) => value
            .parse::<usize>()
            .ok()
            .filter(|&limit| limit > 0)
            .ok_or_else(|| {
                failed("SPARROW_WORKER_OUTPUT_LIMIT_BYTES must be a positive integer")
            })?,
        Err(std::env::VarError::NotPresent) => DEFAULT_OUTPUT_LIMIT,
        Err(error) => return Err(failed(error)),
    };
    run_with_limit(command, input, environment, cancelled, output_limit).await
}

async fn run_with_limit(
    command: Vec<OsString>,
    input: Vec<u8>,
    environment: Option<std::collections::BTreeMap<String, String>>,
    mut cancelled: tokio::sync::watch::Receiver<bool>,
    output_limit: usize,
) -> Outcome {
    if *cancelled.borrow() {
        return Err(Rc::new(Failure::Cancelled));
    }
    let (executable, args) = command
        .split_first()
        .ok_or_else(|| failed("empty worker command"))?;
    let mut command = tokio::process::Command::new(executable);
    if let Some(environment) = environment {
        command.env_clear().envs(environment);
    }
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command
        .args(args)
        // Independent of argument parsing: a marked child may never fall back
        // to main if the worker flag is lost or misplaced in a future change.
        .env("SPARROW_INTERNAL_WORKER", "1")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(failed)?;
    let mut group = ProcessGroup::new(&child);
    let mut stdin = child.stdin.take().expect("piped stdin");
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let remaining = Cell::new(output_limit);
    let result = {
        let operation = async {
            let (_, stdout, stderr, status) = tokio::try_join!(
                async move { stdin.write_all(&input).await },
                read_limited(stdout, &remaining),
                read_limited(stderr, &remaining),
                async {
                    let status = child.wait().await;
                    // A descendant must not keep pipes open after the worker exits.
                    group.terminate();
                    status
                },
            )
            .map_err(failed)?;
            Ok(Rc::new(Output {
                stdout,
                stderr,
                success: status.success(),
                code: status.code(),
            }))
        };
        tokio::select! {
            biased;
            _ = cancelled.changed() => Err(Rc::new(Failure::Cancelled)),
            result = operation => result,
        }
    };
    if result.is_err() {
        group.terminate();
        let _ = child.start_kill();
        child.wait().await.map_err(failed)?;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_flag_precedes_program_path_and_language_arguments() {
        for args in [
            vec!["sparrow-next", "program.spb"],
            vec!["sparrow-next", "--cold", "program.spb", "--test"],
            vec!["sparrow-next", "--test", "program.spb"],
        ] {
            let original: Vec<OsString> = args.into_iter().map(Into::into).collect();
            let command = worker_command(original.clone());
            assert_eq!(command[0], original[0]);
            assert_eq!(command[1], "--worker");
            assert_eq!(&command[2..], &original[1..]);
        }
        assert!(worker_command(vec![]).is_empty());
    }

    #[test]
    fn output_overflow_stops_worker_without_waiting_for_exit() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (_cancel, wait) = tokio::sync::watch::channel(false);
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                run_with_limit(
                    vec![
                        "/bin/sh".into(),
                        "-c".into(),
                        "head -c 65536 /dev/zero; exec sleep 5".into(),
                    ],
                    vec![],
                    None,
                    wait,
                    4096,
                ),
            )
            .await
            .unwrap();
            assert!(matches!(result, Err(error) if matches!(&*error,
                Failure::Process(message) if message.contains("output exceeds byte limit"))));
        });
    }

    #[test]
    fn output_budget_is_shared_by_both_streams() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let remaining = Cell::new(5);
            assert_eq!(read_limited(&b"abc"[..], &remaining).await.unwrap(), b"abc");
            assert!(read_limited(&b"def"[..], &remaining).await.is_err());
            assert_eq!(remaining.get(), 2);
        });
    }

    #[cfg(unix)]
    #[test]
    fn worker_exit_closes_pipes_inherited_by_descendants() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let request = Request::from_command(
                vec![
                    "/bin/sh".into(),
                    "-c".into(),
                    "sleep 5 & printf done".into(),
                ],
                vec![],
                None,
            );
            let output = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                std::future::poll_fn(|cx| request.poll(cx)),
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(output.stdout, b"done");
            assert!(output.success);
        });
    }

    #[test]
    fn cancelling_a_process_reaps_it_and_large_pipe_outputs_do_not_deadlock() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let request = Request::from_command(
                vec![
                    "/bin/sh".into(),
                    "-c".into(),
                    "printf ready; printf error >&2; exec sleep 60".into(),
                ],
                vec![],
                None,
            );
            let wait = std::future::poll_fn(|cx| request.poll(cx));
            tokio::pin!(wait);
            assert!(
                tokio::time::timeout(std::time::Duration::from_millis(20), wait.as_mut())
                    .await
                    .is_err()
            );
            request.cancel();
            let result = tokio::time::timeout(std::time::Duration::from_secs(3), wait)
                .await
                .unwrap();
            assert!(matches!(result,Err(failure) if matches!(&*failure,Failure::Cancelled)));
            let request = Request::from_command(
                vec![
                    "/bin/sh".into(),
                    "-c".into(),
                    "cat; head -c 200000 /dev/zero >&2".into(),
                ],
                vec![b'x'; 200000],
                None,
            );
            let output = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                std::future::poll_fn(|cx| request.poll(cx)),
            )
            .await
            .unwrap()
            .unwrap();
            assert!(output.success);
            assert_eq!(output.stdout.len(), 200000);
            assert_eq!(output.stderr.len(), 200000);
            assert_eq!(output.code, Some(0));
        });
    }
}
