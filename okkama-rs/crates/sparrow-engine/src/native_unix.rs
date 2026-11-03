//! Unix streams share the TCP readiness/cancellation path. Filesystem listeners
//! remove only their own socket inode on close; replacement paths are retained.
use crate::{
    Error,
    native::{Outcome, ProviderError},
    native_network::{Busy, NetworkHandle, Stream, Transport, failure, host, interruptible},
    value::*,
};
use okkama_runtime_abi::{Native, NativeFailure};
use std::{
    cell::{Cell, RefCell},
    os::unix::fs::{FileTypeExt, MetadataExt},
    path::{Component, Path, PathBuf},
    rc::Rc,
};
use tokio::net::{UnixListener, UnixStream};
#[derive(Debug)]
pub(crate) struct Pair {
    sockets: [Rc<Stream>; 2],
    released: Cell<bool>,
    closed: Cell<bool>,
}
#[derive(Debug)]
pub(crate) struct Listener {
    socket: RefCell<Option<Rc<UnixListener>>>,
    changed: tokio::sync::watch::Sender<()>,
    accepting: Rc<Cell<bool>>,
    path: PathBuf,
    identity: (u64, u64),
}
#[derive(Debug, Clone)]
pub(crate) enum UnixHandle {
    Pair(Rc<Pair>),
    Listener(Rc<Listener>),
}
pub(crate) fn handles(n: Native) -> bool {
    use Native::*;
    matches!(
        n,
        NetworkUnixPathValid
            | NetworkUnixPair
            | NetworkUnixPairSocket
            | NetworkUnixPairRelease
            | NetworkUnixPairClose
            | NetworkUnixEndpoint
            | NetworkUnixConnect
            | NetworkUnixListen
            | NetworkUnixListenerPath
            | NetworkUnixAccept
            | NetworkUnixListenerClose
            | NetworkUnixListenerClosed
    )
}
fn checked_path(n: Native, raw: &[u8]) -> Result<(), NativeFailure> {
    if raw.is_empty() || raw.contains(&0) {
        return Err(failure(
            n,
            "invalid_input",
            "Unix socket path must be nonempty and contain no NUL",
        ));
    }
    let maximum = if cfg!(target_os = "linux") { 107 } else { 103 };
    if raw.len() > maximum {
        return Err(failure(
            n,
            "invalid_input",
            &format!("Unix socket path exceeds {maximum} bytes"),
        ));
    }
    Ok(())
}
fn path(n: Native, raw: &[u8], cwd: &Path) -> Result<PathBuf, NativeFailure> {
    checked_path(n, raw)?;
    let raw = PathBuf::from(String::from_utf8_lossy(raw).as_ref());
    let absolute = if raw.is_absolute() {
        raw
    } else {
        cwd.join(raw)
    };
    let mut result = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::ParentDir => {
                result.pop();
            }
            Component::CurDir => {}
            other => result.push(other),
        }
    }
    checked_path(n, result.to_string_lossy().as_bytes())?;
    Ok(result)
}
fn identity(path: &Path) -> Option<(u64, u64)> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    meta.file_type()
        .is_socket()
        .then(|| (meta.dev(), meta.ino()))
}
impl Listener {
    fn socket(&self, n: Native) -> Result<Rc<UnixListener>, NativeFailure> {
        self.socket
            .borrow()
            .clone()
            .ok_or_else(|| failure(n, "already_closed", "Socket is closed"))
    }
    fn close(&self, n: Native) -> Result<(), NativeFailure> {
        if self.socket.borrow_mut().take().is_none() {
            return Ok(());
        }
        self.changed.send_replace(());
        if identity(&self.path) == Some(self.identity) {
            std::fs::remove_file(&self.path)
                .or_else(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        Ok(())
                    } else {
                        Err(e)
                    }
                })
                .map_err(|e| host(n, "Cannot remove Unix listener entry", e))?;
        }
        Ok(())
    }
}
impl Heap {
    fn unix_handle(&self, value: Slot) -> Result<UnixHandle, Error> {
        match self.network_handle(value)? {
            NetworkHandle::Unix(state) => Ok(state.clone()),
            _ => Err(Error("expected Unix native handle".into())),
        }
    }
    pub(crate) fn native_unix(&mut self, n: Native, a: &[Slot]) -> Result<Outcome, Error> {
        match self.unix_execute(n, a) {
            Ok(v) => Ok(Outcome::Value(v)),
            Err(ProviderError::Engine(e)) => Err(e),
            Err(ProviderError::Native(e)) => {
                let payload = self.native_failure_new(e)?;
                let failure = self.failure_new(crate::failure::FailureKind::Native(payload))?;
                if n.signature().is_async {
                    self.future_with_state(crate::future::State::Ready(Err(failure)))
                        .map(Outcome::Value)
                } else {
                    Ok(Outcome::Failure(failure))
                }
            }
        }
    }
    fn unix_execute(&mut self, n: Native, a: &[Slot]) -> Result<Slot, ProviderError> {
        use Native::*;
        let native = ProviderError::Native;
        let invalid = || {
            native(failure(
                n,
                "already_closed",
                "Native handle is already closed",
            ))
        };
        match n {
            NetworkUnixPathValid => {
                checked_path(n, &self.native_bytes(a[0])?).map_err(native)?;
                Ok(Slot::boolean(true))
            }
            NetworkUnixPair => {
                if tokio::runtime::Handle::try_current().is_err() {
                    return Err(Error("Unix pair requires a Tokio runtime".into()).into());
                }
                let (left, right) = UnixStream::pair()
                    .map_err(|e| native(host(n, "Cannot create Unix socket pair", e)))?;
                Ok(
                    self.network_new(NetworkHandle::Unix(UnixHandle::Pair(Rc::new(Pair {
                        sockets: [Rc::new(Stream::new(left)), Rc::new(Stream::new(right))],
                        released: Cell::new(false),
                        closed: Cell::new(false),
                    }))))?,
                )
            }
            NetworkUnixPairSocket | NetworkUnixPairRelease | NetworkUnixPairClose => {
                let UnixHandle::Pair(pair) = self.unix_handle(a[0])? else {
                    return Err(Error("expected Unix pair".into()).into());
                };
                if n == NetworkUnixPairClose {
                    if !pair.released.get() && !pair.closed.replace(true) {
                        for socket in &pair.sockets {
                            socket.close();
                        }
                    }
                    return Ok(Slot::boolean(true));
                }
                if pair.released.get() || pair.closed.get() {
                    return Err(invalid());
                }
                if n == NetworkUnixPairRelease {
                    pair.released.set(true);
                    return Ok(Slot::boolean(true));
                }
                Ok(self.network_new(NetworkHandle::Stream(
                    pair.sockets[usize::from(a[1].bits != 0)].clone(),
                ))?)
            }
            NetworkUnixEndpoint => {
                let state = self.network_stream(a[0])?;
                let socket = state.socket(n).map_err(native)?;
                let Transport::Unix(socket) = &*socket else {
                    return Err(native(failure(
                        n,
                        "invalid_input",
                        "Operation requires a Unix stream connection",
                    )));
                };
                let address = if a[1].bits != 0 {
                    socket.peer_addr()
                } else {
                    socket.local_addr()
                }
                .map_err(|e| native(host(n, "Cannot inspect Unix endpoint", e)))?;
                let bytes = address
                    .as_pathname()
                    .map(|p| p.to_string_lossy().as_bytes().to_vec())
                    .unwrap_or_default();
                Ok(self.native_byte_array(bytes)?)
            }
            NetworkUnixConnect => {
                let path = path(n, &self.native_bytes(a[0])?, &self.working_directory()?)
                    .map_err(native)?;
                Ok(self.native_io(async move {
                    let stream = UnixStream::connect(path)
                        .await
                        .map_err(|e| host(n, "Cannot connect Unix socket", e))?;
                    Ok(NetworkHandle::Stream(Rc::new(Stream::new(stream))))
                })?)
            }
            NetworkUnixListen => {
                let backlog = self
                    .integer(a[1])?
                    .to_i64()
                    .filter(|v| (0..=i32::MAX as i64).contains(v))
                    .ok_or_else(|| {
                        native(failure(
                            n,
                            "invalid_input",
                            "Unix backlog must be nonnegative and fit a host int",
                        ))
                    })?;
                let path = path(n, &self.native_bytes(a[0])?, &self.working_directory()?)
                    .map_err(native)?;
                if tokio::runtime::Handle::try_current().is_err() {
                    return Err(Error("Unix listener requires a Tokio runtime".into()).into());
                }
                let socket =
                    socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)
                        .map_err(|e| native(host(n, "Cannot create Unix stream socket", e)))?;
                socket
                    .set_nonblocking(true)
                    .and_then(|()| socket2::SockAddr::unix(&path))
                    .and_then(|address| socket.bind(&address))
                    .and_then(|()| socket.listen(backlog as i32))
                    .map_err(|e| native(host(n, "Cannot bind Unix listener", e)))?;
                let identity = identity(&path).ok_or_else(|| {
                    native(failure(
                        n,
                        "unexpected",
                        "Cannot capture Unix listener entry identity",
                    ))
                })?;
                let socket = UnixListener::from_std(socket.into())
                    .map_err(|e| native(host(n, "Cannot register Unix listener", e)))?;
                Ok(
                    self.network_new(NetworkHandle::Unix(UnixHandle::Listener(Rc::new(
                        Listener {
                            socket: RefCell::new(Some(Rc::new(socket))),
                            changed: tokio::sync::watch::channel(()).0,
                            accepting: Rc::new(Cell::new(false)),
                            path,
                            identity,
                        },
                    ))))?,
                )
            }
            _ => {
                let UnixHandle::Listener(state) = self.unix_handle(a[0])? else {
                    return Err(Error("expected Unix listener".into()).into());
                };
                if n == NetworkUnixListenerClose {
                    state.close(n).map_err(native)?;
                    return Ok(Slot::boolean(true));
                }
                if n == NetworkUnixListenerClosed {
                    return Ok(Slot::boolean(state.socket.borrow().is_none()));
                }
                let socket = state.socket(n).map_err(native)?;
                match n {
                    NetworkUnixListenerPath => {
                        Ok(self
                            .native_byte_array(state.path.to_string_lossy().as_bytes().to_vec())?)
                    }
                    NetworkUnixAccept => {
                        let busy = Busy::acquire(
                            &state.accepting,
                            n,
                            "Unix socket already has a pending accepting",
                        )
                        .map_err(native)?;
                        let changed = state.changed.subscribe();
                        Ok(self.native_io(async move {
                            let _state = state;
                            let _busy = busy;
                            interruptible(changed, n, async move {
                                let (socket, _) = socket
                                    .accept()
                                    .await
                                    .map_err(|e| host(n, "Cannot accept Unix connection", e))?;
                                Ok(NetworkHandle::Stream(Rc::new(Stream::new(socket))))
                            })
                            .await
                        })?)
                    }
                    _ => Err(Error("unsupported Unix provider".into()).into()),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::future::State;
    fn call(h: &mut Heap, n: Native, a: &[Slot]) -> Slot {
        let outcome = if handles(n) {
            h.native_unix(n, a)
        } else {
            h.native_network(n, a)
        };
        match outcome.unwrap() {
            Outcome::Value(v) => v,
            Outcome::Failure(_) => panic!("failed {}", n.symbol()),
        }
    }
    async fn settle(h: &mut Heap, f: Slot) -> Result<Slot, Slot> {
        if let Some(request) = h.future(f).unwrap().io.clone() {
            tokio::time::timeout(
                std::time::Duration::from_secs(3),
                std::future::poll_fn(|cx| request.poll(cx)),
            )
            .await
            .expect("Unix I/O did not wake");
            h.native_io_settle(f).unwrap();
        }
        let State::Ready(result) = h.future(f).unwrap().state else {
            panic!("not ready")
        };
        result
    }
    #[test]
    fn pair_release_transfers_ownership_and_keeps_binary_streams_independent() {
        crate::AsyncHost::new()
            .unwrap()
            .block_on(async {
                let mut h = Heap::new(&[], &[]);
                let pair = call(&mut h, Native::NetworkUnixPair, &[]);
                let left = call(
                    &mut h,
                    Native::NetworkUnixPairSocket,
                    &[pair, Slot::boolean(false)],
                );
                let right = call(
                    &mut h,
                    Native::NetworkUnixPairSocket,
                    &[pair, Slot::boolean(true)],
                );
                call(&mut h, Native::NetworkUnixPairRelease, &[pair]);
                call(&mut h, Native::NetworkUnixPairClose, &[pair]);
                h.collect([left, right].into_iter()).unwrap();
                let endpoint = call(
                    &mut h,
                    Native::NetworkUnixEndpoint,
                    &[left, Slot::boolean(false)],
                );
                assert!(h.native_bytes(endpoint).unwrap().is_empty());
                let bytes = h.native_byte_array(vec![0, 255, 42]).unwrap();
                let write = call(&mut h, Native::NetworkStreamWrite, &[left, bytes]);
                settle(&mut h, write).await.unwrap();
                let read = call(&mut h, Native::NetworkStreamRead, &[right, Slot::small(3)]);
                let result = settle(&mut h, read).await.unwrap();
                assert_eq!(h.native_bytes(result).unwrap(), [0, 255, 42]);
                call(&mut h, Native::NetworkClose, &[left]);
                assert!(!h.network_stream(right).unwrap().closed());
            })
            .unwrap();
    }
    #[test]
    fn unreleased_pair_close_closes_both_aliases() {
        crate::AsyncHost::new()
            .unwrap()
            .block_on(async {
                let mut h = Heap::new(&[], &[]);
                let pair = call(&mut h, Native::NetworkUnixPair, &[]);
                let left = call(
                    &mut h,
                    Native::NetworkUnixPairSocket,
                    &[pair, Slot::boolean(false)],
                );
                call(&mut h, Native::NetworkUnixPairClose, &[pair]);
                call(&mut h, Native::NetworkUnixPairClose, &[pair]);
                assert!(h.network_stream(left).unwrap().closed());
                assert!(matches!(
                    h.native_unix(Native::NetworkUnixPairRelease, &[pair])
                        .unwrap(),
                    Outcome::Failure(_)
                ));
            })
            .unwrap();
    }
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    #[test]
    fn listener_accept_cancel_and_close_preserve_a_replacement_file() {
        crate::AsyncHost::new()
            .unwrap()
            .block_on(async {
                let mut h = Heap::new(&[], &[]);
                static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                let path = PathBuf::from(format!(
                    "/tmp/sn-{}-{}.sock",
                    std::process::id(),
                    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                ));
                let _cleanup = Cleanup(path.clone());
                let bytes = h
                    .native_byte_array(path.to_string_lossy().as_bytes().to_vec())
                    .unwrap();
                let listener = call(&mut h, Native::NetworkUnixListen, &[bytes, Slot::small(16)]);
                let cancelled = call(&mut h, Native::NetworkUnixAccept, &[listener]);
                h.future_control(okkama_sparrow::FutOp::Cancel, cancelled)
                    .unwrap();
                let accept = call(&mut h, Native::NetworkUnixAccept, &[listener]);
                let connect = call(&mut h, Native::NetworkUnixConnect, &[bytes]);
                let client = settle(&mut h, connect).await.unwrap();
                let server = settle(&mut h, accept).await.unwrap();
                assert!(!h.network_stream(client).unwrap().closed());
                assert!(!h.network_stream(server).unwrap().closed());
                let pending = call(&mut h, Native::NetworkUnixAccept, &[listener]);
                std::fs::remove_file(&path).unwrap();
                use std::io::Write;
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .unwrap()
                    .write_all(b"replacement")
                    .unwrap();
                call(&mut h, Native::NetworkUnixListenerClose, &[listener]);
                assert_eq!(std::fs::read(&path).unwrap(), b"replacement");
                assert!(settle(&mut h, pending).await.is_err());
                call(&mut h, Native::NetworkUnixListenerClose, &[listener]);
            })
            .unwrap();
    }
}
