//! Rust process ownership: Tokio owns child readiness/reaping and async pipes.
//! Synchronous close waits for termination without reaping outside Tokio.
use rustix::process::{Pid, Signal, WaitId, WaitIdOptions};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs::File,
    future::Future,
    io,
    path::PathBuf,
    process::{ExitStatus, Stdio},
    rc::Rc,
    task::{Context, Poll, Waker},
};
#[derive(Debug, Clone)]
pub(crate) struct Command {
    pub program: String,
    pub arguments: Vec<String>,
    pub directory: PathBuf,
    pub environment: BTreeMap<String, String>,
}
impl Command {
    fn executable(&self) -> PathBuf {
        if self.program.contains('/') {
            return self.directory.join(&self.program);
        }
        for entry in self
            .environment
            .get("PATH")
            .map(String::as_str)
            .unwrap_or("/usr/bin:/bin")
            .split(':')
        {
            let path = self.directory.join(entry).join(&self.program);
            if path.is_file() && nix::unistd::access(&path, nix::unistd::AccessFlags::X_OK).is_ok()
            {
                return path;
            }
        }
        self.directory.join(&self.program)
    }
}
#[derive(Debug)]
pub(crate) struct Child {
    pub pid: u32,
    child: RefCell<tokio::process::Child>,
    result: RefCell<Option<Result<ExitStatus, String>>>,
    changed: tokio::sync::watch::Sender<()>,
    reaper: RefCell<Option<Waker>>,
}
impl Child {
    fn publish(&self, result: io::Result<ExitStatus>) {
        *self.result.borrow_mut() = Some(result.map_err(|e| e.to_string()));
        self.changed.send_replace(());
        if let Some(waker) = self.reaper.borrow_mut().take() {
            waker.wake();
        }
    }
    fn poll(&self, cx: &mut Context<'_>) -> Poll<()> {
        if self.result.borrow().is_some() {
            return Poll::Ready(());
        }
        *self.reaper.borrow_mut() = Some(cx.waker().clone());
        let result = {
            let mut child = self.child.borrow_mut();
            let future = child.wait();
            let mut future = std::pin::pin!(future);
            future.as_mut().poll(cx)
        };
        match result {
            Poll::Ready(result) => {
                self.publish(result);
                Poll::Ready(())
            }
            Poll::Pending => Poll::Pending,
        }
    }
    fn new(child: tokio::process::Child) -> Rc<Self> {
        let (changed, _) = tokio::sync::watch::channel(());
        let state = Rc::new(Self {
            pid: child.id().expect("new child"),
            child: RefCell::new(child),
            result: RefCell::new(None),
            changed,
            reaper: RefCell::new(None),
        });
        let weak = Rc::downgrade(&state);
        tokio::task::spawn_local(std::future::poll_fn(move |cx| {
            weak.upgrade()
                .map_or(Poll::Ready(()), |state| state.poll(cx))
        }));
        state
    }
    pub fn running(&self) -> io::Result<bool> {
        if self.result.borrow().is_some() {
            return Ok(false);
        }
        let status = self.child.borrow_mut().try_wait()?;
        if let Some(status) = status {
            self.publish(Ok(status));
            Ok(false)
        } else {
            Ok(true)
        }
    }
    pub fn signal(&self, signal: Signal) -> io::Result<()> {
        if !self.running()? {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Process has completed",
            ));
        }
        let pid = Pid::from_raw(self.pid as i32).expect("child pid");
        rustix::process::kill_process(pid, signal).map_err(Into::into)
    }
    pub fn start_kill(&self) -> io::Result<()> {
        if self.running()? {
            self.child.borrow_mut().start_kill()?;
        }
        Ok(())
    }
    pub async fn wait(&self) -> io::Result<ExitStatus> {
        let mut changed = self.changed.subscribe();
        loop {
            if let Some(result) = self.result.borrow().as_ref() {
                return result.clone().map_err(io::Error::other);
            }
            changed
                .changed()
                .await
                .map_err(|_| io::Error::other("Child reaper closed"))?;
        }
    }
    pub async fn terminate(&self) -> io::Result<()> {
        self.start_kill()?;
        self.wait().await.map(|_| ())
    }
    pub fn close(&self) -> io::Result<()> {
        if !self.running()? {
            return Ok(());
        }
        let mut child = self.child.borrow_mut();
        child.start_kill()?;
        let pid = Pid::from_raw(self.pid as i32).expect("child pid");
        // NOWAIT observes termination but leaves ownership of waitpid/reaping
        // with Tokio. This is the synchronous Okkama close contract only.
        loop {
            match rustix::process::waitid(
                WaitId::Pid(pid),
                WaitIdOptions::EXITED | WaitIdOptions::NOWAIT,
            ) {
                Ok(_) => break,
                Err(e) if e == rustix::io::Errno::INTR => continue,
                Err(e) => return Err(e.into()),
            }
        }
        let status = child
            .try_wait()?
            .ok_or_else(|| io::Error::other("Child termination was not observable"))?;
        drop(child);
        self.publish(Ok(status));
        Ok(())
    }
}
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.close();
        if let Some(waker) = self.reaper.get_mut().take() {
            waker.wake();
        }
    }
}
#[derive(Debug)]
pub(crate) struct Spawned {
    pub child: Rc<Child>,
    pub input: Option<File>,
    pub output: Option<File>,
    pub errors: Option<File>,
}
fn blocking(fd: std::os::fd::OwnedFd) -> io::Result<File> {
    let flags = rustix::fs::fcntl_getfl(&fd)?;
    rustix::fs::fcntl_setfl(&fd, flags & !rustix::fs::OFlags::NONBLOCK)?;
    Ok(fd.into())
}
pub(crate) fn launch(commands: &[Command]) -> io::Result<Vec<Spawned>> {
    tokio::runtime::Handle::try_current().map_err(|_| {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "Process I/O requires a Tokio runtime",
        )
    })?;
    let mut spawned = vec![];
    let mut previous = None;
    for (i, command) in commands.iter().enumerate() {
        let mut builder = tokio::process::Command::new(command.executable());
        builder
            .args(&command.arguments)
            .current_dir(&command.directory)
            .env_clear()
            .envs(&command.environment)
            .kill_on_drop(true)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        builder.stdin(previous.take().map_or_else(Stdio::piped, Stdio::from));
        let mut child = builder.spawn()?;
        let input = child
            .stdin
            .take()
            .map(|s| s.into_owned_fd().and_then(blocking))
            .transpose()?;
        let output = child
            .stdout
            .take()
            .map(|s| s.into_owned_fd().and_then(blocking))
            .transpose()?;
        let errors = child
            .stderr
            .take()
            .map(|s| s.into_owned_fd().and_then(blocking))
            .transpose()?;
        let owner = Child::new(child);
        let output = if i + 1 < commands.len() {
            previous = output;
            None
        } else {
            output
        };
        spawned.push(Spawned {
            child: owner,
            input,
            output,
            errors,
        });
    }
    Ok(spawned)
}
pub(crate) fn async_input(file: File) -> io::Result<tokio::process::ChildStdin> {
    tokio::process::ChildStdin::from_std(std::process::ChildStdin::from(
        std::os::fd::OwnedFd::from(file),
    ))
}
pub(crate) fn async_output(file: File) -> io::Result<tokio::process::ChildStdout> {
    tokio::process::ChildStdout::from_std(std::process::ChildStdout::from(
        std::os::fd::OwnedFd::from(file),
    ))
}
pub(crate) fn async_errors(file: File) -> io::Result<tokio::process::ChildStderr> {
    tokio::process::ChildStderr::from_std(std::process::ChildStderr::from(
        std::os::fd::OwnedFd::from(file),
    ))
}
#[derive(Debug)]
pub(crate) struct Captured {
    pub statuses: Vec<ExitStatus>,
    pub output: Vec<u8>,
    pub errors: Vec<Vec<u8>>,
    pub overflow: (i64, i64),
}
struct CaptureControl {
    children: Vec<Rc<Child>>,
    overflow: std::cell::Cell<(i64, i64)>,
    stop: tokio::sync::watch::Sender<bool>,
}
impl CaptureControl {
    fn stop(&self) {
        self.stop.send_replace(true);
        for child in &self.children {
            let _ = child.start_kill();
        }
    }
    fn overflow(&self, kind: i64, index: i64) {
        if self.overflow.get().0 == 0 {
            self.overflow.set((kind, index));
        }
        self.stop();
    }
}
async fn drain<R: tokio::io::AsyncRead + Unpin>(
    mut stream: R,
    limit: u64,
    control: Rc<CaptureControl>,
    kind: i64,
    index: i64,
) -> io::Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let mut stop = control.stop.subscribe();
    let mut output = Vec::new();
    let mut buffer = vec![0; 65536];
    loop {
        if *stop.borrow() {
            return Ok(output);
        }
        let count = tokio::select! {biased;_=stop.changed()=>return Ok(output),result=stream.read(&mut buffer)=>match result {Ok(n)=>n,Err(e)=>{control.stop();return Err(e);}}};
        if count == 0 {
            return Ok(output);
        }
        output.extend_from_slice(&buffer[..count]);
        if output.len() as u64 > limit {
            control.overflow(kind, index);
            return Ok(output);
        }
    }
}
pub(crate) async fn capture(
    children: Vec<Rc<Child>>,
    input: Option<File>,
    output: Option<File>,
    errors: Vec<Option<File>>,
    payload: Vec<u8>,
    limit: u64,
) -> io::Result<Captured> {
    use tokio::io::AsyncWriteExt;
    let (stop, _) = tokio::sync::watch::channel(false);
    let control = Rc::new(CaptureControl {
        children,
        overflow: std::cell::Cell::new((0, -1)),
        stop,
    });
    let mut tasks = tokio::task::JoinSet::new();
    if let Some(output) = output {
        let stream = async_output(output)?;
        let shared = control.clone();
        let last = control.children.len() as i64 - 1;
        tasks.spawn_local(async move { (1, last, drain(stream, limit, shared, 1, last).await) });
    }
    let count = errors.len();
    for (i, error) in errors.into_iter().enumerate() {
        if let Some(error) = error {
            let stream = async_errors(error)?;
            let shared = control.clone();
            tasks.spawn_local(async move {
                (2, i as i64, drain(stream, limit, shared, 2, i as i64).await)
            });
        }
    }
    if let Some(input) = input {
        let mut stream = async_input(input)?;
        let shared = control.clone();
        let mut stop = shared.stop.subscribe();
        tasks.spawn_local(async move {
            let result=if *stop.borrow() {Ok(())} else {tokio::select! {biased;_=stop.changed()=>Ok(()),result=stream.write_all(&payload)=>result}};
            let result=match result {Err(e) if e.kind()==io::ErrorKind::BrokenPipe=>Ok(vec![]),Err(e)=>{shared.stop();Err(e)},Ok(())=>Ok(vec![])};
            (0,0,result)
        });
    }
    let mut result = Captured {
        statuses: vec![],
        output: vec![],
        errors: vec![vec![]; count],
        overflow: (0, -1),
    };
    let mut failure = None;
    while let Some(task) = tasks.join_next().await {
        match task {
            Ok((kind, index, Ok(bytes))) => match kind {
                1 => result.output = bytes,
                2 => result.errors[index as usize] = bytes,
                _ => {}
            },
            Ok((_, _, Err(e))) => {
                control.stop();
                if failure.is_none() {
                    failure = Some(e);
                }
            }
            Err(e) => {
                control.stop();
                if failure.is_none() {
                    failure = Some(io::Error::other(e.to_string()));
                }
            }
        }
    }
    for child in &control.children {
        match child.wait().await {
            Ok(status) => result.statuses.push(status),
            Err(e) => {
                if failure.is_none() {
                    failure = Some(e);
                }
            }
        }
    }
    result.overflow = control.overflow.get();
    if let Some(error) = failure {
        Err(error)
    } else {
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn shell(script: &str) -> Command {
        Command {
            program: "/bin/sh".into(),
            arguments: vec!["-c".into(), script.into()],
            directory: std::env::temp_dir(),
            environment: BTreeMap::from([("PATH".into(), "/usr/bin:/bin".into())]),
        }
    }
    #[test]
    fn synchronous_close_reaps_before_return_without_nested_runtime() {
        crate::AsyncHost::new()
            .unwrap()
            .block_on(async {
                let child = launch(&[shell("exec sleep 60")]).unwrap().remove(0).child;
                assert!(child.running().unwrap());
                child.close().unwrap();
                assert!(!child.running().unwrap());
                use std::os::unix::process::ExitStatusExt;
                assert_eq!(child.wait().await.unwrap().signal(), Some(libc::SIGKILL));
                child.close().unwrap();
            })
            .unwrap();
    }
    #[test]
    fn pipelines_use_os_pipes_and_drain_stdout_stderr_and_input_concurrently() {
        crate::AsyncHost::new()
            .unwrap()
            .block_on(async {
                let commands = [
                    shell("printf first >&2; exec cat"),
                    shell("printf second >&2; exec tr a-z A-Z"),
                ];
                let mut spawned = launch(&commands).unwrap();
                let children = spawned.iter().map(|s| s.child.clone()).collect();
                let input = spawned[0].input.take();
                let output = spawned[1].output.take();
                let errors = spawned.iter_mut().map(|s| s.errors.take()).collect();
                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    capture(children, input, output, errors, vec![b'a'; 200000], 300000),
                )
                .await
                .unwrap()
                .unwrap();
                assert_eq!(result.output, vec![b'A'; 200000]);
                assert_eq!(result.errors, [b"first".to_vec(), b"second".to_vec()]);
                assert!(result.statuses.iter().all(ExitStatus::success));
                assert_eq!(result.overflow, (0, -1));
            })
            .unwrap();
    }
    #[test]
    fn output_overflow_kills_and_reaps_without_an_unbounded_buffer() {
        crate::AsyncHost::new()
            .unwrap()
            .block_on(async {
                let mut process = launch(&[shell("exec yes x")]).unwrap().remove(0);
                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    capture(
                        vec![process.child.clone()],
                        process.input.take(),
                        process.output.take(),
                        vec![process.errors.take()],
                        vec![],
                        64,
                    ),
                )
                .await
                .unwrap()
                .unwrap();
                assert_eq!(result.overflow, (1, 0));
                assert!(result.output.len() <= 65536 + 64);
                assert!(!process.child.running().unwrap());
            })
            .unwrap();
    }
}
