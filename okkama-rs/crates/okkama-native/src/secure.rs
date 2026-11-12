//! Криптостойкие случайные байты от генератора платформы (#195).
//!
//! - На системах Apple — `arc4random_buf`: генератор ChaCha20 в пространстве
//!   пользователя, который засевает и пересевает ядро. Запрос не делает
//!   системного вызова (`getentropy` делает: около микросекунды на macOS).
//! - Остальные системы — `getrandom`: glibc 2.41 на Linux 6.11 и новее
//!   отвечает из vDSO генератором того же устройства, старые системы делают
//!   один системный вызов.
//!
//! Своего генератора здесь нет: состояние держат ядро и libc, они же знают
//! о fork и о клонировании виртуальной машины.
#![allow(unsafe_code)]

use crate::Error;

#[cfg(target_vendor = "apple")]
unsafe extern "C" {
    fn arc4random_buf(buffer: *mut core::ffi::c_void, size: usize);
}

/// Заполнить `out` криптостойкими случайными байтами.
#[cfg(target_vendor = "apple")]
pub fn fill(out: &mut [u8]) -> Result<(), Error> {
    // SAFETY: указатель и длина описывают `out`, доступный для записи целиком;
    // arc4random_buf не завершается неудачей.
    unsafe { arc4random_buf(out.as_mut_ptr().cast(), out.len()) };
    Ok(())
}

/// Заполнить `out` криптостойкими случайными байтами.
#[cfg(not(target_vendor = "apple"))]
pub fn fill(out: &mut [u8]) -> Result<(), Error> {
    getrandom::fill(out).map_err(|_| Error::Failed("system entropy is unavailable".into()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn fills_whole_buffers_with_varying_bytes() {
        let mut first = [0u8; 64];
        let mut second = [0u8; 64];
        super::fill(&mut first).expect("entropy");
        super::fill(&mut second).expect("entropy");
        assert_ne!(first, second);
        assert!(first.iter().any(|byte| *byte != 0));
        // Пустой запрос и запрос крупнее одного блока getentropy (256 байт).
        super::fill(&mut []).expect("empty");
        let mut large = vec![0u8; 4096];
        super::fill(&mut large).expect("entropy");
        assert!(large[3840..].iter().any(|byte| *byte != 0));
    }
}
