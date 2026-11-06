//! Explicitly closed native secrets, also wiped when their arena entry is swept.
use crate::host::*;
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
        self.failure_new(crate::host::FailureKind::Native(payload))
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
        if args[0].kind() != HANDLE {
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
