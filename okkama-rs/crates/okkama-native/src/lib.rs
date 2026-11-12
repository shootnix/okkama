//! Хостовая криптография Okkama: проверенные реализации RustCrypto и aws-lc
//! (RSA) за узким C ABI (`ffi`), которым пользуются рантаймы (JVM через FFM). Алгоритмы не
//! реализуются здесь заново: модуль только выбирает их, проверяет аргументы и
//! переводит отказы в переносимые коды.

pub mod ffi;
pub mod ops;
pub mod rsa_public;
pub mod secure;
pub mod x509;

pub use ops::{Error, Hasher, call, interruptible};
