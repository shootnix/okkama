//! Thin RustCrypto adapter; cryptographic state stays in Rust, independently
//! of either interpreter. Digest updates preserve immutable snapshot semantics.
use crate::{Error, handle::Handle, native::Outcome, value::*};
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
        if value.tag != HANDLE {
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
        self.failure_new(crate::failure::FailureKind::Native(payload))
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

#[cfg(test)]
mod tests {
    use super::*;
    fn value(result: Result<Outcome, Error>) -> Slot {
        match result.unwrap() {
            Outcome::Value(slot) => slot,
            _ => panic!("expected value"),
        }
    }
    fn hex(heap: &Heap, bytes: Slot) -> String {
        heap.native_bytes(bytes)
            .unwrap()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
    #[test]
    fn digest_snapshots_preserve_original_state_and_survive_gc() {
        let mut heap = Heap::new(&[], &[]);
        let algorithm = heap.allocate_text("sha256".into()).unwrap();
        let key = heap.native_byte_array(vec![]).unwrap();
        let original =
            value(heap.native_crypto(Native::DigestCreate, &[algorithm, key, Slot::small(32)]));
        let bytes = heap.native_byte_array(b"abc".to_vec()).unwrap();
        let updated = value(heap.native_crypto(Native::DigestUpdate, &[original, bytes]));
        heap.collect([original, updated].into_iter()).unwrap();
        let digest = value(heap.native_crypto(Native::DigestBytes, &[updated, Slot::small(32)]));
        assert_eq!(
            hex(&heap, digest),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let empty = value(heap.native_crypto(Native::DigestBytes, &[original, Slot::small(32)]));
        assert_eq!(
            hex(&heap, empty),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let Outcome::Failure(failure) = heap
            .native_crypto(Native::DigestBytes, &[updated, Slot::small(-1)])
            .unwrap()
        else {
            panic!("bad size")
        };
        let crate::failure::FailureKind::Native(payload) = heap.failure(failure).unwrap().kind
        else {
            panic!("native")
        };
        let failure = heap.native_failure(payload).unwrap();
        assert_eq!(failure.domain, "core.crypto");
        assert_eq!(failure.code, "invalid_input");
        heap.collect([updated].into_iter()).unwrap();
        assert!(heap.digest(original).is_err());
        assert!(heap.digest(updated).is_ok());
    }
}
