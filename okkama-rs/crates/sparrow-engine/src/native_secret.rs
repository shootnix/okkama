//! Explicitly closed native secrets, also wiped when their arena entry is swept.
use crate::{Error, handle::Handle, native::Outcome, value::*};
use okkama_runtime_abi::{Native, NativeFailure};
use zeroize::Zeroizing;
pub(crate) struct Secret(Option<Zeroizing<Vec<u8>>>);
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Secret")
            .field("closed", &self.0.is_none())
            .finish_non_exhaustive()
    }
}
pub(crate) fn handles(native: Native) -> bool {
    matches!(
        native,
        Native::SecretCreate
            | Native::SecretSize
            | Native::SecretExport
            | Native::SecretClose
            | Native::SecretClosed
    )
}
impl Heap {
    fn secret_failure(
        &mut self,
        native: Native,
        code: &str,
        message: &str,
    ) -> Result<Outcome, Error> {
        let payload = self.native_failure_new(NativeFailure {
            domain: "core.secret".into(),
            code: code.into(),
            operation: native.symbol().into(),
            message: message.into(),
        })?;
        self.failure_new(crate::failure::FailureKind::Native(payload))
            .map(Outcome::Failure)
    }
    pub(crate) fn native_secret(
        &mut self,
        native: Native,
        args: &[Slot],
    ) -> Result<Outcome, Error> {
        use Native::*;
        if native == SecretCreate {
            if self.elements(args[0])?.len() > 1_048_576 {
                return self.secret_failure(
                    native,
                    "invalid_input",
                    "secret payload exceeds limit",
                );
            }
            let bytes = Zeroizing::new(self.native_bytes(args[0])?);
            return Ok(Outcome::Value(Slot {
                tag: HANDLE,
                bits: self.handles.allocate(Handle::Secret(Secret(Some(bytes))))?,
            }));
        }
        if args[0].tag != HANDLE {
            return Err(Error("expected native SecretState handle".into()));
        }
        let Handle::Secret(secret) = self.handles.get_mut(args[0].bits)? else {
            return Err(Error("expected native SecretState handle".into()));
        };
        match native {
            SecretClosed => Ok(Outcome::Value(Slot::boolean(secret.0.is_none()))),
            SecretClose => {
                secret.0 = None;
                Ok(Outcome::Value(Slot::boolean(true)))
            }
            SecretSize | SecretExport => {
                let Some(bytes) = &secret.0 else {
                    return self.secret_failure(
                        native,
                        "already_closed",
                        "Native handle is already closed",
                    );
                };
                if native == SecretSize {
                    return Ok(Outcome::Value(Slot::small(bytes.len() as i64)));
                }
                let snapshot = bytes.to_vec();
                self.native_byte_array(snapshot).map(Outcome::Value)
            }
            _ => Err(Error("invalid secret provider".into())),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn value(result: Result<Outcome, Error>) -> Slot {
        match result.unwrap() {
            Outcome::Value(v) => v,
            _ => panic!("value"),
        }
    }
    #[test]
    fn secret_exports_are_snapshots_and_close_is_shared_idempotent_and_checked() {
        let mut heap = Heap::new(&[], &[]);
        let bytes = heap.native_byte_array(vec![0, 42, 255]).unwrap();
        let secret = value(heap.native_secret(Native::SecretCreate, &[bytes]));
        let export = value(heap.native_secret(Native::SecretExport, &[secret]));
        heap.collect([secret, export].into_iter()).unwrap();
        assert_eq!(
            value(heap.native_secret(Native::SecretSize, &[secret])).bits,
            3
        );
        for _ in 0..2 {
            assert_eq!(
                value(heap.native_secret(Native::SecretClose, &[secret])).bits,
                1
            );
        }
        assert_eq!(heap.native_bytes(export).unwrap(), [0, 42, 255]);
        assert_eq!(
            value(heap.native_secret(Native::SecretClosed, &[secret])).bits,
            1
        );
        let Outcome::Failure(failure) =
            heap.native_secret(Native::SecretExport, &[secret]).unwrap()
        else {
            panic!("closed")
        };
        let crate::failure::FailureKind::Native(payload) = heap.failure(failure).unwrap().kind
        else {
            panic!("native")
        };
        assert_eq!(heap.native_failure(payload).unwrap().code, "already_closed");
        heap.collect(std::iter::empty()).unwrap();
        assert!(heap.native_secret(Native::SecretClosed, &[secret]).is_err());
    }
}
