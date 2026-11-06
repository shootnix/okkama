//! Portable file-watch semantics use Tokio timers and its bounded blocking pool.
//! Snapshots crossing the pool boundary contain only Rust paths and metadata.
use crate::host::{
    Error,
    handle::Handle,
    ProviderError,
    native_fs::{fail, host},
    native_io::Output,
    *,
};
use okkama_runtime_abi::{Native, NativeFailure};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashSet, VecDeque},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    rc::Rc,
};
#[derive(Debug, Clone)]
struct Metadata {
    identity: (u64, u64),
    mode: u32,
    size: u64,
    modified: i64,
    changed: i64,
    directory: bool,
}
impl Metadata {
    fn differs(&self, other: &Self) -> bool {
        self.mode != other.mode
            || self.size != other.size
            || self.modified != other.modified
            || self.changed != other.changed
    }
}
#[derive(Debug)]
struct Snapshot {
    metadata: Option<Metadata>,
    children: BTreeMap<String, Metadata>,
}
fn metadata(path: &Path) -> std::io::Result<Option<Metadata>> {
    match std::fs::symlink_metadata(path) {
        Ok(m) => Ok(Some(Metadata {
            identity: (m.dev(), m.ino()),
            mode: m.mode(),
            size: m.size(),
            modified: m.mtime(),
            changed: m.ctime(),
            directory: m.is_dir(),
        })),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
fn snapshot(path: &Path) -> std::io::Result<Snapshot> {
    let metadata = metadata(path)?;
    let mut children = BTreeMap::new();
    if metadata.as_ref().is_some_and(|m| m.directory) {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if let Some(meta) = self::metadata(&entry.path())? {
                children.insert(entry.file_name().to_string_lossy().into_owned(), meta);
            }
        }
    }
    Ok(Snapshot { metadata, children })
}
#[derive(Debug)]
struct Event {
    kind: &'static str,
    path: String,
    previous: String,
}
impl Output for Event {
    fn publish(self: Box<Self>, heap: &mut Heap) -> Result<Slot, Error> {
        let mut entries = vec![];
        for (key, value) in [
            ("kind", self.kind.to_owned()),
            ("path", self.path),
            ("previous", self.previous),
        ] {
            entries.push((heap.allocate_text(key.into())?, heap.allocate_text(value)?));
        }
        heap.map_new(entries.into_iter())
    }
}
#[derive(Debug)]
pub(crate) struct Watch {
    path: PathBuf,
    text: String,
    snapshot: RefCell<Snapshot>,
    events: RefCell<VecDeque<Event>>,
    busy: Cell<bool>,
    closed: Cell<bool>,
    changed: tokio::sync::watch::Sender<()>,
}
struct Waiter(Rc<Watch>);
impl Drop for Waiter {
    fn drop(&mut self) {
        self.0.busy.set(false);
    }
}
impl Watch {
    fn compare(&self, next: Snapshot) {
        let mut events = self.events.borrow_mut();
        let previous = self.snapshot.borrow();
        let mut event = |kind, path, previous| {
            events.push_back(Event {
                kind,
                path,
                previous,
            })
        };
        match (&previous.metadata, &next.metadata) {
            (None, Some(_)) => event("created", self.text.clone(), String::new()),
            (Some(_), None) => event("removed", self.text.clone(), String::new()),
            (Some(a), Some(b)) if a.identity != b.identity => {
                event("removed", self.text.clone(), String::new());
                event("created", self.text.clone(), String::new());
            }
            (Some(a), Some(b)) if !b.directory && a.differs(b) => {
                event("modified", self.text.clone(), String::new())
            }
            _ => {}
        }
        if next.metadata.as_ref().is_some_and(|m| m.directory)
            && previous.metadata.as_ref().is_some_and(|m| m.directory)
        {
            let child = |name: &str| {
                crate::host::path_syntax::joined(
                    crate::host::path_syntax::Spec::Unix,
                    &[self.text.clone(), name.into()],
                )
            };
            let mut removed = vec![];
            let mut created = vec![];
            for (name, a) in &previous.children {
                match next.children.get(name) {
                    Some(b) if a.identity == b.identity => {
                        if a.differs(b) {
                            event("modified", child(name), String::new());
                        }
                    }
                    _ => removed.push(name),
                }
            }
            for (name, b) in &next.children {
                if previous
                    .children
                    .get(name)
                    .is_none_or(|a| a.identity != b.identity)
                {
                    created.push(name);
                }
            }
            let mut renamed = HashSet::new();
            for name in removed {
                if let Some(new) = created.iter().copied().find(|new| {
                    !renamed.contains(*new)
                        && next.children[*new].identity == previous.children[name].identity
                }) {
                    event("renamed", child(new), child(name));
                    renamed.insert(new);
                } else {
                    event("removed", child(name), String::new());
                }
            }
            for name in created {
                if !renamed.contains(name) {
                    event("created", child(name), String::new());
                }
            }
        }
        drop(previous);
        *self.snapshot.borrow_mut() = next;
    }
}
fn failure(n: Native, code: &str, message: &str) -> NativeFailure {
    NativeFailure {
        domain: "core.fs".into(),
        code: code.into(),
        operation: n.symbol().into(),
        message: message.into(),
    }
}
fn io_failure(n: Native, e: std::io::Error) -> NativeFailure {
    match host(n, e) {
        ProviderError::Native(e) => e,
        _ => unreachable!(),
    }
}
pub(crate) fn handles(n: Native) -> bool {
    use Native::*;
    matches!(
        n,
        FsFileWatchCreate | FsFileWatchEvent | FsFileWatchClose | FsFileWatchClosed
    )
}
impl Heap {
    pub(crate) fn watch_execute(&mut self, n: Native, a: &[Slot]) -> Result<Slot, ProviderError> {
        use Native::*;
        if n == FsFileWatchCreate {
            let path = self.fs_path(n, a[0])?;
            let text = String::from_utf8_lossy(&self.native_bytes(a[0])?).into_owned();
            let initial = snapshot(&path).map_err(|e| host(n, e))?;
            if initial.metadata.is_none() {
                return Err(fail(n, "not_found", "Watched path does not exist"));
            }
            let (changed, _) = tokio::sync::watch::channel(());
            return Ok(Slot {
                tag: HANDLE,
                bits: self.handles.allocate(Handle::Watch(Rc::new(Watch {
                    path,
                    text,
                    snapshot: RefCell::new(initial),
                    events: RefCell::new(VecDeque::new()),
                    busy: Cell::new(false),
                    closed: Cell::new(false),
                    changed,
                })))?,
            });
        }
        if a[0].kind() != HANDLE {
            return Err(Error("expected FileWatchState".into()).into());
        }
        let Handle::Watch(watch) = self.handles.get(a[0].bits)? else {
            return Err(Error("expected FileWatchState".into()).into());
        };
        let watch = watch.clone();
        if n == FsFileWatchClosed {
            return Ok(Slot::boolean(watch.closed.get()));
        }
        if n == FsFileWatchClose {
            watch.closed.set(true);
            watch.changed.send_replace(());
            return Ok(Slot::boolean(true));
        }
        let error = if watch.closed.get() {
            Some(failure(n, "already_closed", "File watch is closed"))
        } else if watch.busy.replace(true) {
            Some(failure(
                n,
                "resource_exhausted",
                "File watch already has a waiter",
            ))
        } else {
            None
        };
        // Only the accepted request owns the reservation; a rejected duplicate
        // must not release another request's reservation on drop.
        let guard = error.is_none().then(|| Waiter(watch.clone()));
        let mut closed = watch.changed.subscribe();
        Ok(self.native_io(async move {
            let _guard=guard;if let Some(error)=error {return Err(error);}
            loop {
                if watch.closed.get() {return Err(failure(n,"already_closed","File watch is closed"));}
                if let Some(event)=watch.events.borrow_mut().pop_front() {return Ok(event);}
                let path=watch.path.clone();
                let scan=tokio::task::spawn_blocking(move||snapshot(&path));
                let next=tokio::select! {biased;_=closed.changed()=>return Err(failure(n,"already_closed","File watch is closed")),result=scan=>result.map_err(|_|failure(n,"unexpected","File watch scan failed"))?.map_err(|e|io_failure(n,e))?};
                watch.compare(next);
                if let Some(event)=watch.events.borrow_mut().pop_front(){return Ok(event);}
                tokio::select! {biased;_=closed.changed()=>return Err(failure(n,"already_closed","File watch is closed")),_=tokio::time::sleep(std::time::Duration::from_millis(50))=>{}}
            }
        })?)
    }
}
