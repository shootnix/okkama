//! C ABI. Буферы результата выделяет библиотека; вызывающий освобождает их
//! `okkama_free`. Код возврата: 0 — успех (в `out` результат), иначе код
//! `ops::Error::code` и текст отказа в `out` (UTF-8).
#![allow(unsafe_code)]

use std::slice;

use crate::ops::{self, Error, Hasher};

fn give(bytes: Vec<u8>, out: *mut *mut u8, out_len: *mut usize) {
    let boxed = bytes.into_boxed_slice();
    let len = boxed.len();
    let ptr = Box::into_raw(boxed).cast::<u8>();
    // SAFETY: вызывающий передаёт действительные указатели на слоты результата.
    unsafe {
        *out = ptr;
        *out_len = len;
    }
}

fn answer(result: Result<Vec<u8>, Error>, out: *mut *mut u8, out_len: *mut usize) -> i32 {
    match result {
        Ok(bytes) => {
            give(bytes, out, out_len);
            0
        }
        Err(error) => {
            let code = error.code();
            give(error.message().as_bytes().to_vec(), out, out_len);
            code
        }
    }
}

/// SAFETY: `ptr`/`len` описывают читаемую память или `len == 0`.
unsafe fn bytes<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if len == 0 || ptr.is_null() {
        &[]
    } else {
        // SAFETY: см. контракт функции.
        unsafe { slice::from_raw_parts(ptr, len) }
    }
}

/// Освободить буфер результата.
///
/// # Safety
/// `ptr`/`len` — буфер, выданный этой библиотекой, освобождается один раз.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn okkama_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        // SAFETY: буфер создан `give` из `Box<[u8]>` той же длины.
        drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)) });
    }
}

/// Операция `op` над `count` аргументами (`args[i]`, длины `lens[i]`).
///
/// # Safety
/// Все указатели действительны для указанных длин; `out`/`out_len` — слоты.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn okkama_crypto(
    op: *const u8,
    op_len: usize,
    args: *const *const u8,
    lens: *const usize,
    count: usize,
    out: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    let result = std::panic::catch_unwind(|| {
        // SAFETY: контракт функции.
        let name = unsafe { bytes(op, op_len) };
        let name =
            std::str::from_utf8(name).map_err(|_| Error::Invalid("operation name".into()))?;
        let mut values = Vec::with_capacity(count);
        for index in 0..count {
            // SAFETY: контракт функции: `count` пар указатель/длина.
            unsafe { values.push(bytes(*args.add(index), *lens.add(index))) };
        }
        ops::call(name, &values)
    })
    .unwrap_or_else(|_| Err(Error::Failed("crypto provider panicked".into())));
    answer(result, out, out_len)
}

/// Новое состояние дайджеста/MAC; при отказе — null, код в `code` и текст в `out`.
///
/// # Safety
/// Указатели действительны для указанных длин; `out`/`out_len` — слоты.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn okkama_digest_new(
    algorithm: *const u8,
    algorithm_len: usize,
    key: *const u8,
    key_len: usize,
    length: usize,
    code: *mut i32,
    out: *mut *mut u8,
    out_len: *mut usize,
) -> *mut Hasher {
    // SAFETY: контракт функции.
    let (name, key) = unsafe { (bytes(algorithm, algorithm_len), bytes(key, key_len)) };
    let made = std::str::from_utf8(name)
        .map_err(|_| Error::Invalid("algorithm name".into()))
        .and_then(|name| Hasher::new(name, key, length));
    match made {
        Ok(hasher) => Box::into_raw(Box::new(hasher)),
        Err(error) => {
            let value = answer(Err(error), out, out_len);
            // SAFETY: `code` — слот вызывающего.
            unsafe { *code = value };
            std::ptr::null_mut()
        }
    }
}

/// Новое состояние: копия `state`, дополненная данными (`state` не меняется).
///
/// # Safety
/// `state` — живое состояние этой библиотеки; данные читаемы.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn okkama_digest_update(
    state: *const Hasher,
    data: *const u8,
    len: usize,
) -> *mut Hasher {
    // SAFETY: контракт функции.
    let mut next = unsafe { (*state).clone() };
    // SAFETY: контракт функции.
    next.update(unsafe { bytes(data, len) });
    Box::into_raw(Box::new(next))
}

/// Результат состояния (`length` — длина XOF); состояние не меняется.
///
/// # Safety
/// `state` — живое состояние; `out`/`out_len` — слоты.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn okkama_digest_finish(
    state: *const Hasher,
    length: usize,
    out: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    // SAFETY: контракт функции.
    answer(unsafe { (*state).finish(length) }, out, out_len)
}

/// Освободить состояние.
///
/// # Safety
/// `state` — состояние этой библиотеки, освобождается один раз.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn okkama_digest_free(state: *mut Hasher) {
    if !state.is_null() {
        // SAFETY: создано `Box::into_raw` в этом модуле.
        drop(unsafe { Box::from_raw(state) });
    }
}
