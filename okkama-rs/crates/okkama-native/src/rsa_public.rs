//! Открытые операции RSA через низкоуровневый API aws-lc: проверка подписей
//! PKCS#1 v1.5 и PSS над готовым хэшем и размер ключа. Высокоуровневый
//! aws-lc-rs знает только PSS с солью длины хэша и не знает SHA-224, а X.509
//! повторяет JDK: любой хэш из списка, любая соль, модули до 16384 бит.
//! Закрытых операций здесь нет (они в `ops.rs` через aws-lc-rs).
#![allow(unsafe_code)]

use std::fmt;
use std::os::raw::c_int;

use aws_lc_sys as lc;

/// Хэш подписи: NID для PKCS#1 v1.5 и EVP_MD для PSS/MGF1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hash {
    Sha1,
    Sha224,
    Sha256,
    Sha384,
    Sha512,
}

impl Hash {
    fn nid(self) -> c_int {
        match self {
            Hash::Sha1 => lc::NID_sha1,
            Hash::Sha224 => lc::NID_sha224,
            Hash::Sha256 => lc::NID_sha256,
            Hash::Sha384 => lc::NID_sha384,
            Hash::Sha512 => lc::NID_sha512,
        }
    }

    fn md(self) -> *const lc::EVP_MD {
        // SAFETY: функции возвращают указатели на статические описания.
        unsafe {
            match self {
                Hash::Sha1 => lc::EVP_sha1(),
                Hash::Sha224 => lc::EVP_sha224(),
                Hash::Sha256 => lc::EVP_sha256(),
                Hash::Sha384 => lc::EVP_sha384(),
                Hash::Sha512 => lc::EVP_sha512(),
            }
        }
    }
}

/// Открытый ключ RSA, разобранный из RSAPublicKey (PKCS#1) DER.
pub struct PublicKey(*mut lc::RSA);

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicKey({} bits)", self.bits())
    }
}

impl PublicKey {
    /// `None`, если DER не ключ RSA или модуль больше 16384 бит.
    pub fn parse(der: &[u8]) -> Option<Self> {
        // SAFETY: указатель и длина берутся из среза; результат — NULL или
        // ключ, которым дальше владеет `PublicKey`.
        let rsa = unsafe { lc::RSA_public_key_from_bytes(der.as_ptr(), der.len()) };
        clear_errors();
        (!rsa.is_null()).then_some(Self(rsa))
    }

    pub fn bits(&self) -> u32 {
        // SAFETY: `self.0` — живой ключ.
        unsafe { lc::RSA_bits(self.0) }
    }

    /// RSASSA-PKCS1-v1_5 над хэшем `digest`.
    pub fn pkcs1(&self, hash: Hash, digest: &[u8], signature: &[u8]) -> bool {
        // SAFETY: срезы живут на время вызова; ключ живой.
        let ok = unsafe {
            lc::RSA_verify(hash.nid(), digest.as_ptr(), digest.len(), signature.as_ptr(), signature.len(), self.0)
        };
        clear_errors();
        ok == 1
    }

    /// RSASSA-PSS над хэшем `digest`, MGF1 с хэшем `mask`, соль `salt` байт.
    pub fn pss(&self, hash: Hash, mask: Hash, salt: usize, digest: &[u8], signature: &[u8]) -> bool {
        let Ok(salt) = c_int::try_from(salt) else {
            return false;
        };
        // SAFETY: срезы живут на время вызова; ключ живой; EVP_MD статические.
        let ok = unsafe {
            lc::RSA_verify_pss_mgf1(
                self.0,
                digest.as_ptr(),
                digest.len(),
                hash.md(),
                mask.md(),
                salt,
                signature.as_ptr(),
                signature.len(),
            )
        };
        clear_errors();
        ok == 1
    }
}

impl Drop for PublicKey {
    fn drop(&mut self) {
        // SAFETY: ключ получен от `RSA_public_key_from_bytes` и освобождается один раз.
        unsafe { lc::RSA_free(self.0) }
    }
}

/// Отказ aws-lc оставляет запись в очереди ошибок потока; она не нужна.
fn clear_errors() {
    // SAFETY: без аргументов, очищает очередь текущего потока.
    unsafe { lc::ERR_clear_error() }
}
