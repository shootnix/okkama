//! Thin RustCrypto adapter; cryptographic state stays in Rust, independently
//! of either interpreter. Digest updates preserve immutable snapshot semantics.
use crate::host::*;
use okkama_runtime_abi::{Native, NativeFailure};
pub(crate) fn handles(native: Native) -> bool {
    matches!(
        native,
        Native::HostCrypto | Native::DigestCreate | Native::DigestUpdate | Native::DigestBytes
    )
}
fn length(heap: &Heap, value: Slot) -> Result<usize, okkama_native::Error> {
    heap.integer(value)
        .ok()
        .and_then(|n| n.to_i64())
        .filter(|n| (0..=i32::MAX as i64).contains(n))
        .map(|n| n as usize)
        .ok_or(okkama_native::Error::Invalid(
            "Digest length is out of range".into(),
        ))
}
impl Heap {
    fn digest(&self, value: Slot) -> Result<&okkama_native::Hasher, Error> {
        if value.kind() != HANDLE {
            return Err(Error("expected native DigestState handle".into()));
        }
        match self.handles.get(value.bits)? {
            Handle::Digest(state) => Ok(state),
            _ => Err(Error("expected native DigestState handle".into())),
        }
    }
    fn digest_new(&mut self, state: okkama_native::Hasher) -> Result<Outcome, Error> {
        Ok(Outcome::Value(Slot {
            tag: HANDLE,
            bits: self.handles.allocate(Handle::Digest(Box::new(state)))?,
        }))
    }
    fn crypto_failure(
        &mut self,
        native: Native,
        error: okkama_native::Error,
    ) -> Result<Outcome, Error> {
        let code = match error {
            okkama_native::Error::Invalid(_) => "invalid_input",
            okkama_native::Error::Authentication(_) => "permission_denied",
            okkama_native::Error::Unsupported(_) => "unsupported",
            okkama_native::Error::Failed(_) => "unexpected",
        };
        let payload = self.native_failure_new(NativeFailure {
            domain: "core.crypto".into(),
            code: code.into(),
            operation: native.symbol().into(),
            message: error.message().into(),
        })?;
        self.failure_new(crate::host::FailureKind::Native(payload))
            .map(Outcome::Failure)
    }
    pub(crate) fn native_crypto(
        &mut self,
        native: Native,
        args: &[Slot],
    ) -> Result<Outcome, Error> {
        match native {
            Native::HostCrypto => {
                let parts = self
                    .elements(args[1])?
                    .iter()
                    .map(|part| self.native_bytes(*part))
                    .collect::<Result<Vec<_>, _>>()?;
                let slices = parts.iter().map(Vec::as_slice).collect::<Vec<_>>();
                match okkama_native::call(self.text(args[0])?, &slices) {
                    Ok(bytes) => self.native_byte_array(bytes).map(Outcome::Value),
                    Err(error) => self.crypto_failure(native, error),
                }
            }
            Native::DigestCreate => {
                let size = match length(self, args[2]) {
                    Ok(size) => size,
                    Err(error) => return self.crypto_failure(native, error),
                };
                match okkama_native::Hasher::new(
                    self.text(args[0])?,
                    &self.native_bytes(args[1])?,
                    size,
                ) {
                    Ok(state) => self.digest_new(state),
                    Err(error) => self.crypto_failure(native, error),
                }
            }
            Native::DigestUpdate => {
                let mut next = self.digest(args[0])?.clone();
                next.update(&self.native_bytes(args[1])?);
                self.digest_new(next)
            }
            Native::DigestBytes => {
                let size = match length(self, args[1]) {
                    Ok(size) => size,
                    Err(error) => return self.crypto_failure(native, error),
                };
                match self.digest(args[0])?.finish(size) {
                    Ok(bytes) => self.native_byte_array(bytes).map(Outcome::Value),
                    Err(error) => self.crypto_failure(native, error),
                }
            }
            _ => Err(Error("invalid crypto provider".into())),
        }
    }
}

