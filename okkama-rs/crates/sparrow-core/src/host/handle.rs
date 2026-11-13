//! Typed native state in the engine's generational arena. No legacy Value or
//! process-global integer handle registry crosses this boundary.
use crate::host::*;
use okkama_runtime_abi::{Native, NativeFailure};
#[derive(Debug)]
pub(crate) enum Handle {
    Input(crate::host::native_input::Input),
    #[cfg(unix)]
    Process(crate::host::native_process::ProcessHandle),
    #[cfg(unix)]
    Rooted(crate::host::native_root::Rooted),
    #[cfg(unix)]
    Watch(std::rc::Rc<crate::host::native_watch::Watch>),
    TemporaryDirectory(crate::host::native_temporary::TemporaryDirectory),
    FileState(std::rc::Rc<std::cell::RefCell<crate::host::native_file_state::FileState>>),
    FileLock(std::rc::Rc<crate::host::native_file_state::FileLock>),
    Directory(crate::host::native_directory::Directory),
    File(crate::host::native_file::RawFile),
    Network(crate::host::native_network::NetworkHandle),
    Db(crate::host::native_db::DbHandle),
    Sqlite(std::rc::Rc<std::cell::RefCell<crate::host::native_sqlite::Connection>>),
    SqliteReply(std::rc::Rc<crate::host::native_sqlite::Reply>),
    Json(crate::host::native_json::JsonHandle),
    Regex(crate::host::native_regex::RegexHandle),
    Secret(crate::host::native_secret::Secret),
    Buffer(crate::host::native_buffer::Buffer),
    TextBuffer(String),
    Digest(Box<okkama_native::Hasher>),
    TestState {
        limit: i64,
        size: i64,
        closed: bool,
    },
    /// A `native object` of a bridged origin provider (`native_origin`).
    Origin(okkama_runtime_core::Value),
    PgPage(Box<crate::host::native_postgres::PgPage>),
    Tls(std::rc::Rc<crate::host::native_tls::Tls>),
}
impl Handle {
    /// Exhaustive on purpose: adding a provider requires an explicit decision
    /// about VM edges. Rust-owned strings, byte buffers and OS handles are leaves.
    pub(crate) fn roots(&self, roots: &mut Vec<Slot>) {
        match self {
            Self::Json(json) => json.roots(roots),
            Self::Input(_)
            | Self::TemporaryDirectory(_)
            | Self::FileState(_)
            | Self::FileLock(_)
            | Self::Directory(_)
            | Self::File(_)
            | Self::Network(_)
            | Self::Db(_)
            | Self::Sqlite(_)
            | Self::SqliteReply(_)
            | Self::Regex(_)
            | Self::Secret(_)
            | Self::Buffer(_)
            | Self::TextBuffer(_)
            | Self::Digest(_)
            | Self::TestState { .. }
            | Self::Origin(_)
            | Self::PgPage(_)
            | Self::Tls(_) => {}
            #[cfg(unix)]
            Self::Process(_) | Self::Rooted(_) | Self::Watch(_) => {}
        }
    }
}
impl Heap {
    pub(crate) fn handle_test(&mut self, native: Native, args: &[Slot]) -> Result<Outcome, Error> {
        let failure = |heap: &mut Heap, code: &str, message: &str| {
            let payload = heap.native_failure_new(NativeFailure {
                domain: "core.testing".into(),
                code: code.into(),
                operation: native.symbol().into(),
                message: message.into(),
            })?;
            heap.failure_new(crate::host::FailureKind::Native(payload))
                .map(Outcome::Failure)
        };
        if self.test_failures.is_none() {
            return failure(
                self,
                "permission_denied",
                "Test state requires a running case",
            );
        }
        if native == Native::TestingTestStateOpen {
            let Some(limit) = self
                .int64(args[0])?
                .filter(|n| (0..=65536).contains(n))
            else {
                return failure(
                    self,
                    "invalid_input",
                    "Test state capacity must be 0..65536",
                );
            };
            return Ok(Outcome::Value(Slot {
                tag: HANDLE,
                bits: self.handles.allocate(Handle::TestState {
                    limit,
                    size: 0,
                    closed: false,
                })?,
            }));
        }
        if args[0].kind() != HANDLE {
            return Err(Error("expected native TestState handle".into()));
        }
        let Handle::TestState {
            limit,
            size,
            closed,
        } = self.handles.get_mut(args[0].bits)?
        else {
            return Err(Error("expected native TestState handle".into()));
        };
        if native == Native::TestingTestStateClose {
            *closed = true;
            return Ok(Outcome::Value(Slot::boolean(true)));
        }
        if *closed {
            return failure(self, "already_closed", "Native handle is already closed");
        }
        match native {
            Native::TestingTestStateSize => Ok(Outcome::Value(Slot::small(*size))),
            Native::TestingTestStateNext => {
                if *size >= *limit {
                    return failure(self, "resource_exhausted", "Test state capacity exhausted");
                }
                let result = *size;
                *size += 1;
                Ok(Outcome::Value(Slot::small(result)))
            }
            _ => Err(Error("invalid TestState operation".into())),
        }
    }
}

