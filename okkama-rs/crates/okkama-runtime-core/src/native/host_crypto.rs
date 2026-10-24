//! `runtime/host_crypto`: криптография хоста для IR-интерпретатора и Sparrow —
//! тот же крейт `okkama-native` (RustCrypto), что JVM-рантайм вызывает через FFM.
//! Коды отказов совпадают с JVM (`HostCryptoNatives`): `invalid_input`,
//! `permission_denied` (проверка подлинности), `unsupported`, `unexpected`;
//! домен `core.crypto`.

use std::rc::Rc;

use super::Error;
use crate::{IntValue, Value, abi::Native, abi::NativeFailure, heap::ArrayHeap};

fn failure(native: Native, error: &okkama_native::Error) -> Error {
    let code = match error {
        okkama_native::Error::Invalid(_) => "invalid_input",
        okkama_native::Error::Authentication(_) => "permission_denied",
        okkama_native::Error::Unsupported(_) => "unsupported",
        okkama_native::Error::Failed(_) => "unexpected",
    };
    Error::Failure(NativeFailure {
        domain: "core.crypto".into(),
        code: code.into(),
        operation: native.symbol().into(),
        message: error.message().to_string(),
    })
}

fn bytes(value: &Value, arrays: &ArrayHeap) -> Result<Vec<u8>, Error> {
    Ok(arrays
        .elements(value)?
        .iter()
        .map(Value::as_byte)
        .collect::<Result<Vec<_>, _>>()?)
}

fn byte_array(bytes: Vec<u8>, arrays: &mut ArrayHeap) -> Value {
    arrays.allocate(bytes.into_iter().map(Value::Byte).collect())
}

/// Длина результата дайджеста: 0..2^31, как `HostCryptoNatives.length`.
fn length(native: Native, value: &IntValue) -> Result<usize, Error> {
    value
        .to_i64()
        .filter(|length| (0..=i64::from(i32::MAX)).contains(length))
        .map(|length| length as usize)
        .ok_or_else(|| {
            Error::Failure(NativeFailure {
                domain: "core.crypto".into(),
                code: "invalid_input".into(),
                operation: native.symbol().into(),
                message: "Digest length is out of range".into(),
            })
        })
}

fn digest(value: &Value) -> Result<&okkama_native::Hasher, Error> {
    super::support::handle::<okkama_native::Hasher>(value)
}

pub fn execute(native: Native, args: &[Value], arrays: &mut ArrayHeap) -> Result<Value, Error> {
    match native {
        Native::HostCrypto => {
            let operation = args[0].as_string()?.to_string();
            let parts: Vec<Vec<u8>> = arrays
                .elements(&args[1])?
                .to_vec()
                .iter()
                .map(|part| bytes(part, arrays))
                .collect::<Result<_, _>>()?;
            let slices: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();
            let result = okkama_native::call(&operation, &slices)
                .map_err(|error| failure(native, &error))?;
            Ok(byte_array(result, arrays))
        }
        Native::DigestCreate => {
            let algorithm = args[0].as_string()?.to_string();
            let key = bytes(&args[1], arrays)?;
            let length = length(native, args[2].as_int()?)?;
            let hasher = okkama_native::Hasher::new(&algorithm, &key, length)
                .map_err(|error| failure(native, &error))?;
            Ok(Value::Handle(Rc::new(hasher)))
        }
        Native::DigestUpdate => {
            let mut next = digest(&args[0])?.clone();
            next.update(&bytes(&args[1], arrays)?);
            Ok(Value::Handle(Rc::new(next)))
        }
        Native::DigestBytes => {
            let length = length(native, args[1].as_int()?)?;
            let result = digest(&args[0])?
                .finish(length)
                .map_err(|error| failure(native, &error))?;
            Ok(byte_array(result, arrays))
        }
        _ => Err(Error::InvalidState("not a host_crypto native".into())),
    }
}
