//! Общие преобразования значений для providers: аргументы, результаты,
//! дескрипторы `native object` и отказы `NativeFailure`.
use super::Error;
use crate::heap::{ArrayHeap, maps};
use crate::{IntValue, Value, abi::Native, abi::NativeFailure};
use std::rc::Rc;

/// Значение `native object` provider.
pub fn handle<T: 'static>(value: &Value) -> Result<&T, Error> {
    match value {
        Value::Handle(handle) => handle
            .downcast_ref::<T>()
            .ok_or_else(|| Error::InvalidState("native object of another provider".into())),
        _ => Err(Error::InvalidState("expected a native object".into())),
    }
}

pub fn new_handle<T: 'static>(state: T) -> Value {
    Value::Handle(Rc::new(state))
}

/// Ожидаемый сбой provider.
pub fn failure(domain: &str, code: &str, native: Native, message: impl Into<String>) -> Error {
    Error::Failure(NativeFailure {
        domain: domain.into(),
        code: code.into(),
        operation: native.symbol().into(),
        message: message.into(),
    })
}

/// Ошибка вызова (`Native invocation "…" failed`), как исключение Java-provider.
pub fn failed(native: Native) -> Error {
    Error::Trap(format!("Native invocation \"{}\" failed", native.symbol()))
}

pub fn string(value: &Value) -> Result<&str, Error> {
    Ok(value.as_string()?)
}

pub fn int(value: &Value) -> Result<&IntValue, Error> {
    Ok(value.as_int()?)
}

pub fn boolean(value: &Value) -> Result<bool, Error> {
    Ok(value.as_bool()?)
}

pub fn float(value: &Value) -> Result<f64, Error> {
    Ok(value.as_float()?)
}

pub fn items(value: &Value) -> Result<&[Value], Error> {
    Ok(ArrayHeap.elements(value)?)
}

pub fn bytes(value: &Value) -> Result<Vec<u8>, Error> {
    Ok(items(value)?
        .iter()
        .map(Value::as_byte)
        .collect::<Result<Vec<_>, _>>()?)
}

pub fn strings(value: &Value) -> Result<Vec<String>, Error> {
    items(value)?
        .iter()
        .map(|item| Ok(item.as_string()?.to_string()))
        .collect()
}

pub fn ints(value: &Value) -> Result<Vec<IntValue>, Error> {
    items(value)?
        .iter()
        .map(|item| Ok(item.as_int()?.clone()))
        .collect()
}

pub fn text(value: impl Into<String>) -> Value {
    Value::Str(Rc::from(value.into()))
}

pub fn int_value(value: i64) -> Value {
    Value::Int(IntValue::from_i64(value))
}

pub fn byte_array(bytes: impl IntoIterator<Item = u8>) -> Value {
    ArrayHeap.allocate(bytes.into_iter().map(Value::Byte).collect())
}

pub fn array(items: Vec<Value>) -> Value {
    ArrayHeap.allocate(items)
}

pub fn string_array<S: Into<String>>(items: impl IntoIterator<Item = S>) -> Value {
    ArrayHeap.allocate(items.into_iter().map(text).collect())
}

/// `%string:V` из пар (ключи упорядочиваются, как у `TreeMap`).
pub fn map(entries: impl IntoIterator<Item = (String, Value)>) -> Value {
    maps::from_entries(entries.into_iter().map(|(key, value)| (text(key), value)))
        .expect("string keys")
}

pub fn int_map<'a>(entries: impl IntoIterator<Item = (&'a str, i64)>) -> Value {
    map(entries
        .into_iter()
        .map(|(key, value)| (key.to_string(), int_value(value))))
}

pub fn string_map(entries: impl IntoIterator<Item = (String, String)>) -> Value {
    map(entries.into_iter().map(|(key, value)| (key, text(value))))
}

/// Пары `%string:V` словаря-аргумента.
pub fn entries(value: &Value) -> Result<Vec<(String, Value)>, Error> {
    maps::entries(value)?
        .iter()
        .map(|(key, value)| match key.to_value() {
            Value::Str(key) => Ok((key.to_string(), value.clone())),
            _ => Err(Error::InvalidState("expected string map keys".into())),
        })
        .collect()
}

/// Индекс `int` в `0..limit` (Java `int`), иначе None.
pub fn small(value: &IntValue, limit: usize) -> Option<usize> {
    value
        .to_i64()
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value < limit)
}

/// `BigInteger.longValue()` Java: младшие 64 бита в дополнительном коде.
pub fn java_long(value: &IntValue) -> i64 {
    if let Some(small) = value.to_i64() {
        return small;
    }
    let modulus = IntValue::from_big(crate::bignum::Big::from_u64(2).power(64));
    let mut rest = crate::integer::divide(value, &modulus).expect("non-zero").1;
    if rest.is_negative() {
        rest = crate::integer::add(&rest, &modulus);
    }
    let unsigned: u128 = rest.to_string().parse().expect("64-bit remainder");
    unsigned as u64 as i64
}

/// `BigInteger.intValue()` Java: младшие 32 бита.
pub fn java_int(value: &IntValue) -> i32 {
    java_long(value) as i32
}
