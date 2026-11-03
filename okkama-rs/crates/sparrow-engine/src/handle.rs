//! Typed native state in the engine's generational arena. No legacy Value or
//! process-global integer handle registry crosses this boundary.
use crate::{Error, native::Outcome, value::*};
use okkama_runtime_abi::{Native, NativeFailure};
#[derive(Debug)]
pub(crate) enum Handle {
    Input(crate::native_input::Input),
    #[cfg(unix)]
    Process(crate::native_process::ProcessHandle),
    #[cfg(unix)]
    Rooted(crate::native_root::Rooted),
    #[cfg(unix)]
    Watch(std::rc::Rc<crate::native_watch::Watch>),
    TemporaryDirectory(crate::native_temporary::TemporaryDirectory),
    FileState(std::rc::Rc<std::cell::RefCell<crate::native_file_state::FileState>>),
    FileLock(std::rc::Rc<crate::native_file_state::FileLock>),
    Directory(crate::native_directory::Directory),
    File(crate::native_file::RawFile),
    Network(crate::native_network::NetworkHandle),
    Db(crate::native_db::DbHandle),
    Json(crate::native_json::JsonHandle),
    Regex(crate::native_regex::RegexHandle),
    Secret(crate::native_secret::Secret),
    Buffer(crate::native_buffer::Buffer),
    Digest(Box<okkama_native::Hasher>),
    TestState {
        limit: i64,
        size: i64,
        closed: bool,
    },
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
            | Self::Regex(_)
            | Self::Secret(_)
            | Self::Buffer(_)
            | Self::Digest(_)
            | Self::TestState { .. } => {}
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
            heap.failure_new(crate::failure::FailureKind::Native(payload))
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
                .integer(args[0])?
                .to_i64()
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
        if args[0].tag != HANDLE {
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

#[cfg(test)]
mod tests {
    use super::*;
    fn value(result: Result<Outcome, Error>) -> Slot {
        match result.unwrap() {
            Outcome::Value(value) => value,
            _ => panic!("expected value"),
        }
    }
    fn code(heap: &Heap, outcome: Outcome) -> String {
        let Outcome::Failure(failure) = outcome else {
            panic!("expected failure")
        };
        let crate::failure::FailureKind::Native(payload) = heap.failure(failure).unwrap().kind
        else {
            panic!("native failure")
        };
        heap.native_failure(payload).unwrap().code
    }
    #[test]
    fn test_state_is_scoped_bounded_closed_and_generation_checked() {
        let mut heap = Heap::new(&[], &[]);
        let outcome = heap
            .handle_test(Native::TestingTestStateOpen, &[Slot::small(1)])
            .unwrap();
        assert_eq!(code(&heap, outcome), "permission_denied");
        heap.test_failures = Some(vec![]);
        for limit in [-1, 65537] {
            let outcome = heap
                .handle_test(Native::TestingTestStateOpen, &[Slot::small(limit)])
                .unwrap();
            assert_eq!(code(&heap, outcome), "invalid_input");
        }
        let handle = value(heap.handle_test(Native::TestingTestStateOpen, &[Slot::small(1)]));
        heap.collect([handle].into_iter()).unwrap();
        assert_eq!(
            value(heap.handle_test(Native::TestingTestStateNext, &[handle])).bits,
            0
        );
        assert_eq!(
            value(heap.handle_test(Native::TestingTestStateSize, &[handle])).bits,
            1
        );
        let outcome = heap
            .handle_test(Native::TestingTestStateNext, &[handle])
            .unwrap();
        assert_eq!(code(&heap, outcome), "resource_exhausted");
        for _ in 0..2 {
            assert_eq!(
                value(heap.handle_test(Native::TestingTestStateClose, &[handle])).bits,
                1
            );
        }
        let outcome = heap
            .handle_test(Native::TestingTestStateSize, &[handle])
            .unwrap();
        assert_eq!(code(&heap, outcome), "already_closed");
        heap.collect(std::iter::empty()).unwrap();
        let fresh = value(heap.handle_test(Native::TestingTestStateOpen, &[Slot::small(1)]));
        assert_ne!(handle.bits, fresh.bits);
        assert!(
            heap.handle_test(Native::TestingTestStateSize, &[handle])
                .is_err()
        );
        assert_eq!(
            value(heap.handle_test(Native::TestingTestStateSize, &[fresh])).bits,
            0
        );
    }
}
