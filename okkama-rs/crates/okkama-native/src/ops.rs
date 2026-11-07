//! Операции хостовой криптографии над байтами (без unsafe). Каждая операция —
//! имя и список байтовых аргументов; целые передаются десятичным ASCII.
//! Случайность — только из аргумента-зерна (энтропию выбирает вызывающий) или
//! явной операции `random`.

use std::fmt;

use aes_gcm::aead::{Aead, KeyInit, Payload};
use blake2::digest::consts::{U16, U20, U28, U32, U48, U64};
use hmac::Mac;
use sha2::Digest;
use subtle::ConstantTimeEq;

/// Отказ операции: категория для переносимого кода отказа и текст.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Неверные аргументы: длины, ключи, параметры, кодировки.
    Invalid(String),
    /// Проверка подлинности не прошла (AEAD-тег, общий секрет из малой подгруппы).
    Authentication(String),
    /// Алгоритм или операция не поддерживаются.
    Unsupported(String),
    /// Сбой провайдера.
    Failed(String),
}

impl Error {
    /// Код для C ABI: 1 invalid, 2 authentication, 3 unsupported, 4 failed.
    pub fn code(&self) -> i32 {
        match self {
            Error::Invalid(_) => 1,
            Error::Authentication(_) => 2,
            Error::Unsupported(_) => 3,
            Error::Failed(_) => 4,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Error::Invalid(text)
            | Error::Authentication(text)
            | Error::Unsupported(text)
            | Error::Failed(text) => text,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for Error {}

type Result<T> = std::result::Result<T, Error>;

fn invalid(text: impl Into<String>) -> Error {
    Error::Invalid(text.into())
}

fn arity(args: &[&[u8]], count: usize, op: &str) -> Result<()> {
    if args.len() == count {
        Ok(())
    } else {
        Err(invalid(format!("{op} takes {count} arguments")))
    }
}

fn number(bytes: &[u8], what: &str) -> Result<u64> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| invalid(format!("{what} is not a decimal integer")))?;
    if text.is_empty() || text.len() > 19 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid(format!("{what} is not a decimal integer")));
    }
    text.parse()
        .map_err(|_| invalid(format!("{what} is not a decimal integer")))
}

fn fixed<const N: usize>(bytes: &[u8], what: &str) -> Result<[u8; N]> {
    bytes
        .try_into()
        .map_err(|_| invalid(format!("{what} must be {N} bytes")))
}

fn truth(value: bool) -> Vec<u8> {
    vec![u8::from(value)]
}

// ---- дайджесты с состоянием ----

/// Состояние дайджеста или MAC; клон — независимая ветвь (неизменяемые
/// successor-объекты std).
#[derive(Clone)]
pub enum Hasher {
    Sha224(sha2::Sha224),
    Sha256(sha2::Sha256),
    Sha384(sha2::Sha384),
    Sha512(sha2::Sha512),
    Sha512_256(sha2::Sha512_256),
    Sha3_224(sha3::Sha3_224),
    Sha3_256(sha3::Sha3_256),
    Sha3_384(sha3::Sha3_384),
    Sha3_512(sha3::Sha3_512),
    Shake128(shake::Shake128),
    Shake256(shake::Shake256),
    Blake2b16(blake2::Blake2b<U16>),
    Blake2b20(blake2::Blake2b<U20>),
    Blake2b28(blake2::Blake2b<U28>),
    Blake2b32(blake2::Blake2b<U32>),
    Blake2b48(blake2::Blake2b<U48>),
    Blake2b64(blake2::Blake2b<U64>),
    Blake2s16(blake2::Blake2s<U16>),
    Blake2s20(blake2::Blake2s<U20>),
    Blake2s28(blake2::Blake2s<U28>),
    Blake2s32(blake2::Blake2s<U32>),
    Blake2bMac(blake2::Blake2bMac512),
    Blake2sMac(blake2::Blake2sMac256),
    HmacSha224(hmac::Hmac<sha2::Sha224>),
    HmacSha256(hmac::Hmac<sha2::Sha256>),
    HmacSha384(hmac::Hmac<sha2::Sha384>),
    HmacSha512(hmac::Hmac<sha2::Sha512>),
    HmacSha3_256(hmac::SimpleHmac<sha3::Sha3_256>),
    HmacSha3_512(hmac::SimpleHmac<sha3::Sha3_512>),
}

impl fmt::Debug for Hasher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Hasher")
    }
}

fn hmac_key<M: hmac::digest::KeyInit>(key: &[u8]) -> Result<M> {
    M::new_from_slice(key).map_err(|_| invalid("invalid MAC key length"))
}

impl Hasher {
    /// Новое состояние: `algorithm` — имя алгоритма; `key` — ключ MAC
    /// (пустой у бесключевых); `length` — длина вывода BLAKE2.
    pub fn new(algorithm: &str, key: &[u8], length: usize) -> Result<Self> {
        let keyless = |hasher: Hasher| {
            if key.is_empty() {
                Ok(hasher)
            } else {
                Err(invalid(format!("{algorithm} takes no key")))
            }
        };
        match algorithm {
            "sha224" => keyless(Hasher::Sha224(sha2::Sha224::new())),
            "sha256" => keyless(Hasher::Sha256(sha2::Sha256::new())),
            "sha384" => keyless(Hasher::Sha384(sha2::Sha384::new())),
            "sha512" => keyless(Hasher::Sha512(sha2::Sha512::new())),
            "sha512-256" => keyless(Hasher::Sha512_256(sha2::Sha512_256::new())),
            "sha3-224" => keyless(Hasher::Sha3_224(sha3::Sha3_224::new())),
            "sha3-256" => keyless(Hasher::Sha3_256(sha3::Sha3_256::new())),
            "sha3-384" => keyless(Hasher::Sha3_384(sha3::Sha3_384::new())),
            "sha3-512" => keyless(Hasher::Sha3_512(sha3::Sha3_512::new())),
            "shake128" => keyless(Hasher::Shake128(shake::Shake128::default())),
            "shake256" => keyless(Hasher::Shake256(shake::Shake256::default())),
            "blake2b" => keyless(match length {
                16 => Hasher::Blake2b16(Default::default()),
                20 => Hasher::Blake2b20(Default::default()),
                28 => Hasher::Blake2b28(Default::default()),
                32 => Hasher::Blake2b32(Default::default()),
                48 => Hasher::Blake2b48(Default::default()),
                64 => Hasher::Blake2b64(Default::default()),
                _ => {
                    return Err(invalid(
                        "BLAKE2b output length must be 16, 20, 28, 32, 48 or 64",
                    ));
                }
            }),
            "blake2s" => keyless(match length {
                16 => Hasher::Blake2s16(Default::default()),
                20 => Hasher::Blake2s20(Default::default()),
                28 => Hasher::Blake2s28(Default::default()),
                32 => Hasher::Blake2s32(Default::default()),
                _ => return Err(invalid("BLAKE2s output length must be 16, 20, 28 or 32")),
            }),
            "blake2b-mac" => {
                if key.is_empty() || key.len() > 64 {
                    return Err(invalid("BLAKE2b key must be 1..64 bytes"));
                }
                Ok(Hasher::Blake2bMac(hmac_key(key)?))
            }
            "blake2s-mac" => {
                if key.is_empty() || key.len() > 32 {
                    return Err(invalid("BLAKE2s key must be 1..32 bytes"));
                }
                Ok(Hasher::Blake2sMac(hmac_key(key)?))
            }
            "hmac-sha224" => Ok(Hasher::HmacSha224(hmac_key(key)?)),
            "hmac-sha256" => Ok(Hasher::HmacSha256(hmac_key(key)?)),
            "hmac-sha384" => Ok(Hasher::HmacSha384(hmac_key(key)?)),
            "hmac-sha512" => Ok(Hasher::HmacSha512(hmac_key(key)?)),
            "hmac-sha3-256" => Ok(Hasher::HmacSha3_256(hmac_key(key)?)),
            "hmac-sha3-512" => Ok(Hasher::HmacSha3_512(hmac_key(key)?)),
            other => Err(Error::Unsupported(format!(
                "unknown digest algorithm {other}"
            ))),
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        use shake::Update;
        match self {
            Hasher::Sha224(state) => Digest::update(state, data),
            Hasher::Sha256(state) => Digest::update(state, data),
            Hasher::Sha384(state) => Digest::update(state, data),
            Hasher::Sha512(state) => Digest::update(state, data),
            Hasher::Sha512_256(state) => Digest::update(state, data),
            Hasher::Sha3_224(state) => Digest::update(state, data),
            Hasher::Sha3_256(state) => Digest::update(state, data),
            Hasher::Sha3_384(state) => Digest::update(state, data),
            Hasher::Sha3_512(state) => Digest::update(state, data),
            Hasher::Shake128(state) => Update::update(state, data),
            Hasher::Shake256(state) => Update::update(state, data),
            Hasher::Blake2b16(state) => Digest::update(state, data),
            Hasher::Blake2b20(state) => Digest::update(state, data),
            Hasher::Blake2b28(state) => Digest::update(state, data),
            Hasher::Blake2b32(state) => Digest::update(state, data),
            Hasher::Blake2b48(state) => Digest::update(state, data),
            Hasher::Blake2b64(state) => Digest::update(state, data),
            Hasher::Blake2s16(state) => Digest::update(state, data),
            Hasher::Blake2s20(state) => Digest::update(state, data),
            Hasher::Blake2s28(state) => Digest::update(state, data),
            Hasher::Blake2s32(state) => Digest::update(state, data),
            Hasher::Blake2bMac(state) => Mac::update(state, data),
            Hasher::Blake2sMac(state) => Mac::update(state, data),
            Hasher::HmacSha224(state) => Mac::update(state, data),
            Hasher::HmacSha256(state) => Mac::update(state, data),
            Hasher::HmacSha384(state) => Mac::update(state, data),
            Hasher::HmacSha512(state) => Mac::update(state, data),
            Hasher::HmacSha3_256(state) => Mac::update(state, data),
            Hasher::HmacSha3_512(state) => Mac::update(state, data),
        }
    }

    /// Результат копии состояния (само состояние не меняется); `length` —
    /// длина вывода XOF (SHAKE), у остальных игнорируется.
    pub fn finish(&self, length: usize) -> Result<Vec<u8>> {
        use shake::{ExtendableOutput, XofReader};
        Ok(match self.clone() {
            Hasher::Sha224(state) => state.finalize().to_vec(),
            Hasher::Sha256(state) => state.finalize().to_vec(),
            Hasher::Sha384(state) => state.finalize().to_vec(),
            Hasher::Sha512(state) => state.finalize().to_vec(),
            Hasher::Sha512_256(state) => state.finalize().to_vec(),
            Hasher::Sha3_224(state) => state.finalize().to_vec(),
            Hasher::Sha3_256(state) => state.finalize().to_vec(),
            Hasher::Sha3_384(state) => state.finalize().to_vec(),
            Hasher::Sha3_512(state) => state.finalize().to_vec(),
            Hasher::Shake128(state) => {
                xof_length(length)?;
                let mut out = vec![0; length];
                state.finalize_xof().read(&mut out);
                out
            }
            Hasher::Shake256(state) => {
                xof_length(length)?;
                let mut out = vec![0; length];
                state.finalize_xof().read(&mut out);
                out
            }
            Hasher::Blake2b16(state) => state.finalize().to_vec(),
            Hasher::Blake2b20(state) => state.finalize().to_vec(),
            Hasher::Blake2b28(state) => state.finalize().to_vec(),
            Hasher::Blake2b32(state) => state.finalize().to_vec(),
            Hasher::Blake2b48(state) => state.finalize().to_vec(),
            Hasher::Blake2b64(state) => state.finalize().to_vec(),
            Hasher::Blake2s16(state) => state.finalize().to_vec(),
            Hasher::Blake2s20(state) => state.finalize().to_vec(),
            Hasher::Blake2s28(state) => state.finalize().to_vec(),
            Hasher::Blake2s32(state) => state.finalize().to_vec(),
            Hasher::Blake2bMac(state) => state.finalize().into_bytes().to_vec(),
            Hasher::Blake2sMac(state) => state.finalize().into_bytes().to_vec(),
            Hasher::HmacSha224(state) => state.finalize().into_bytes().to_vec(),
            Hasher::HmacSha256(state) => state.finalize().into_bytes().to_vec(),
            Hasher::HmacSha384(state) => state.finalize().into_bytes().to_vec(),
            Hasher::HmacSha512(state) => state.finalize().into_bytes().to_vec(),
            Hasher::HmacSha3_256(state) => state.finalize().into_bytes().to_vec(),
            Hasher::HmacSha3_512(state) => state.finalize().into_bytes().to_vec(),
        })
    }
}

fn xof_length(length: usize) -> Result<()> {
    if (1..=1 << 20).contains(&length) {
        Ok(())
    } else {
        Err(invalid("XOF output length must be 1..1048576"))
    }
}

/// Одноразовый дайджест (для операций, принимающих имя хеша).
fn digest(algorithm: &str, data: &[u8]) -> Result<Vec<u8>> {
    let mut hasher = Hasher::new(algorithm, &[], 0)?;
    hasher.update(data);
    hasher.finish(0)
}

// ---- операции ----

/// Выполнить операцию `op` над аргументами.
pub fn call(op: &str, args: &[&[u8]]) -> Result<Vec<u8>> {
    match op {
        "random" => {
            arity(args, 1, op)?;
            let size = number(args[0], "size")?;
            if size > 1 << 24 {
                return Err(invalid("random size exceeds 16777216"));
            }
            let mut out = vec![0; size as usize];
            getrandom::fill(&mut out)
                .map_err(|_| Error::Failed("system entropy is unavailable".into()))?;
            Ok(out)
        }
        "ct-equal" => {
            arity(args, 2, op)?;
            Ok(truth(
                args[0].len() == args[1].len() && bool::from(args[0].ct_eq(args[1])),
            ))
        }
        "digest" => {
            arity(args, 2, op)?;
            let algorithm = std::str::from_utf8(args[0]).map_err(|_| invalid("algorithm name"))?;
            digest(algorithm, args[1])
        }
        _ if op.starts_with("hkdf-") => hkdf(&op[5..], args),
        _ if op.starts_with("pbkdf2-") => pbkdf2(&op[7..], args),
        "argon2" => argon2_raw(args),
        "argon2-phc" => argon2_phc(args),
        "argon2-verify" => argon2_verify(args),
        _ if op.starts_with("aead-seal:") => aead(&op[10..], args, true),
        _ if op.starts_with("aead-open:") => aead(&op[10..], args, false),
        _ if op.starts_with("ed25519-") => ed25519(&op[8..], args),
        _ if op.starts_with("x25519-") => x25519(&op[7..], args),
        _ if op.starts_with("ecdsa-") => ecdsa(&op[6..], args),
        _ if op.starts_with("rsa-") => rsa_op(&op[4..], args),
        "pkcs8-algorithm" => {
            arity(args, 1, op)?;
            pkcs8_algorithm(args[0])
        }
        "spki-algorithm" => {
            arity(args, 1, op)?;
            spki_algorithm(args[0])
        }
        "x509-key" => {
            arity(args, 1, op)?;
            Ok(crate::x509::key(args[0])?.into_bytes())
        }
        "x509-verify" => {
            arity(args, 4, op)?;
            Ok(truth(crate::x509::verify(args[0], args[1], args[2], args[3])?))
        }
        other => Err(Error::Unsupported(format!(
            "unknown crypto operation {other}"
        ))),
    }
}

fn hkdf(hash: &str, args: &[&[u8]]) -> Result<Vec<u8>> {
    arity(args, 4, "hkdf")?;
    let (ikm, salt, info) = (args[0], args[1], args[2]);
    let length = number(args[3], "length")? as usize;
    let salt = if salt.is_empty() { None } else { Some(salt) };
    let mut out = vec![0; length];
    let expanded = match hash {
        "sha256" => hkdf::Hkdf::<sha2::Sha256>::new(salt, ikm)
            .expand(info, &mut out)
            .is_ok(),
        "sha384" => hkdf::Hkdf::<sha2::Sha384>::new(salt, ikm)
            .expand(info, &mut out)
            .is_ok(),
        "sha512" => hkdf::Hkdf::<sha2::Sha512>::new(salt, ikm)
            .expand(info, &mut out)
            .is_ok(),
        other => return Err(Error::Unsupported(format!("unknown HKDF hash {other}"))),
    };
    if length == 0 || !expanded {
        return Err(invalid("HKDF output length must be 1..255 hash lengths"));
    }
    Ok(out)
}

fn pbkdf2(hash: &str, args: &[&[u8]]) -> Result<Vec<u8>> {
    arity(args, 4, "pbkdf2")?;
    let (password, salt) = (args[0], args[1]);
    let rounds = number(args[2], "iterations")?;
    let length = number(args[3], "length")? as usize;
    if rounds == 0 || rounds > u64::from(u32::MAX) {
        return Err(invalid("PBKDF2 iterations must be 1..4294967295"));
    }
    if !(1..=1 << 16).contains(&length) {
        return Err(invalid("PBKDF2 output length must be 1..65536"));
    }
    let rounds = rounds as u32;
    let mut out = vec![0; length];
    match hash {
        "sha256" => pbkdf2::pbkdf2_hmac::<sha2::Sha256>(password, salt, rounds, &mut out),
        "sha384" => pbkdf2::pbkdf2_hmac::<sha2::Sha384>(password, salt, rounds, &mut out),
        "sha512" => pbkdf2::pbkdf2_hmac::<sha2::Sha512>(password, salt, rounds, &mut out),
        "sha1" => {
            return Err(Error::Unsupported(
                "PBKDF2-HMAC-SHA1 is not provided".into(),
            ));
        }
        other => return Err(Error::Unsupported(format!("unknown PBKDF2 hash {other}"))),
    }
    Ok(out)
}

fn argon2_params(args: &[&[u8]], length: Option<usize>) -> Result<argon2::Argon2<'static>> {
    let memory = number(args[0], "memory")?;
    let time = number(args[1], "iterations")?;
    let lanes = number(args[2], "parallelism")?;
    let params = argon2::Params::new(
        u32::try_from(memory).map_err(|_| invalid("Argon2 memory"))?,
        u32::try_from(time).map_err(|_| invalid("Argon2 iterations"))?,
        u32::try_from(lanes).map_err(|_| invalid("Argon2 parallelism"))?,
        length,
    )
    .map_err(|error| invalid(format!("invalid Argon2 parameters: {error}")))?;
    Ok(argon2::Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        params,
    ))
}

/// `argon2(password, salt, memory_kib, iterations, parallelism, length)` — сырой вывод Argon2id.
fn argon2_raw(args: &[&[u8]]) -> Result<Vec<u8>> {
    arity(args, 6, "argon2")?;
    let length = number(args[5], "length")? as usize;
    let hasher = argon2_params(&args[2..5], Some(length))?;
    let mut out = vec![0; length];
    hasher
        .hash_password_into(args[0], args[1], &mut out)
        .map_err(|error| invalid(format!("Argon2: {error}")))?;
    Ok(out)
}

/// `argon2-phc(password, salt, memory_kib, iterations, parallelism)` — строка PHC.
fn argon2_phc(args: &[&[u8]]) -> Result<Vec<u8>> {
    use argon2::PasswordHasher;
    arity(args, 5, "argon2-phc")?;
    let hasher = argon2_params(&args[2..5], None)?;
    let hash = hasher
        .hash_password_with_salt(args[0], args[1])
        .map_err(|error| invalid(format!("Argon2: {error}")))?;
    Ok(hash.to_string().into_bytes())
}

/// `argon2-verify(phc, password)` — `[1]`/`[0]`.
fn argon2_verify(args: &[&[u8]]) -> Result<Vec<u8>> {
    use argon2::PasswordVerifier;
    arity(args, 2, "argon2-verify")?;
    let text = std::str::from_utf8(args[0]).map_err(|_| invalid("PHC string is not UTF-8"))?;
    let hash = argon2::password_hash::phc::PasswordHash::new(text)
        .map_err(|error| invalid(format!("invalid PHC string: {error}")))?;
    if hash.algorithm.as_str() != "argon2id" {
        return Err(Error::Unsupported(
            "only argon2id PHC strings are verified".into(),
        ));
    }
    Ok(truth(
        argon2::Argon2::default()
            .verify_password(args[1], &hash)
            .is_ok(),
    ))
}

fn aead(algorithm: &str, args: &[&[u8]], seal: bool) -> Result<Vec<u8>> {
    arity(args, 4, "aead")?;
    let (key, nonce, aad, data) = (args[0], args[1], args[2], args[3]);
    let payload = Payload { msg: data, aad };
    let failed = || Error::Authentication("authentication failed".into());
    macro_rules! run {
        ($cipher:ty, $key_len:expr, $nonce_len:expr) => {{
            if key.len() != $key_len {
                return Err(invalid(format!(
                    "{algorithm} key must be {} bytes",
                    $key_len
                )));
            }
            if nonce.len() != $nonce_len {
                return Err(invalid(format!(
                    "{algorithm} nonce must be {} bytes",
                    $nonce_len
                )));
            }
            let cipher = <$cipher>::new_from_slice(key).map_err(|_| invalid("AEAD key"))?;
            let nonce = nonce.try_into().map_err(|_| invalid("AEAD nonce"))?;
            if seal {
                cipher
                    .encrypt(nonce, payload)
                    .map_err(|_| Error::Failed("AEAD encryption".into()))
            } else {
                cipher.decrypt(nonce, payload).map_err(|_| failed())
            }
        }};
    }
    match algorithm {
        "aes-128-gcm" => run!(aes_gcm::Aes128Gcm, 16, 12),
        "aes-256-gcm" => run!(aes_gcm::Aes256Gcm, 32, 12),
        "chacha20-poly1305" => run!(chacha20poly1305::ChaCha20Poly1305, 32, 12),
        "xchacha20-poly1305" => run!(chacha20poly1305::XChaCha20Poly1305, 32, 24),
        other => Err(Error::Unsupported(format!("unknown AEAD {other}"))),
    }
}

fn ed25519(op: &str, args: &[&[u8]]) -> Result<Vec<u8>> {
    use ed25519_dalek::pkcs8::{
        DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey,
    };
    use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
    let signing = |seed: &[u8]| -> Result<SigningKey> {
        Ok(SigningKey::from_bytes(&fixed::<32>(seed, "Ed25519 seed")?))
    };
    let verifying = |raw: &[u8]| -> Result<VerifyingKey> {
        VerifyingKey::from_bytes(&fixed::<32>(raw, "Ed25519 public key")?)
            .map_err(|_| invalid("invalid Ed25519 public key"))
    };
    match op {
        "public" => {
            arity(args, 1, op)?;
            Ok(signing(args[0])?.verifying_key().to_bytes().to_vec())
        }
        "sign" => {
            arity(args, 2, op)?;
            Ok(signing(args[0])?.sign(args[1]).to_bytes().to_vec())
        }
        "verify" => {
            arity(args, 3, op)?;
            let key = verifying(args[0])?;
            let signature =
                ed25519_dalek::Signature::from_bytes(&fixed::<64>(args[2], "Ed25519 signature")?);
            Ok(truth(key.verify_strict(args[1], &signature).is_ok()))
        }
        "pkcs8" => {
            arity(args, 1, op)?;
            // PKCS#8 v1 (RFC 8410, 48 байт) без открытого ключа — как OpenSSL.
            let pair = ed25519_dalek::pkcs8::KeypairBytes {
                secret_key: fixed::<32>(args[0], "Ed25519 seed")?,
                public_key: None,
            };
            let der = pair
                .to_pkcs8_der()
                .map_err(|_| Error::Failed("PKCS#8 encoding".into()))?;
            Ok(der.as_bytes().to_vec())
        }
        "from-pkcs8" => {
            arity(args, 1, op)?;
            let key = SigningKey::from_pkcs8_der(args[0])
                .map_err(|_| invalid("invalid Ed25519 PKCS#8 key"))?;
            Ok(key.to_bytes().to_vec())
        }
        "spki" => {
            arity(args, 1, op)?;
            let der = verifying(args[0])?
                .to_public_key_der()
                .map_err(|_| Error::Failed("SPKI encoding".into()))?;
            Ok(der.as_bytes().to_vec())
        }
        "from-spki" => {
            arity(args, 1, op)?;
            let key = VerifyingKey::from_public_key_der(args[0])
                .map_err(|_| invalid("invalid Ed25519 SPKI key"))?;
            Ok(key.to_bytes().to_vec())
        }
        other => Err(Error::Unsupported(format!(
            "unknown Ed25519 operation {other}"
        ))),
    }
}

fn x25519(op: &str, args: &[&[u8]]) -> Result<Vec<u8>> {
    use x25519_dalek::{PublicKey, StaticSecret};
    match op {
        "public" => {
            arity(args, 1, op)?;
            let secret = StaticSecret::from(fixed::<32>(args[0], "X25519 private key")?);
            Ok(PublicKey::from(&secret).as_bytes().to_vec())
        }
        "shared" => {
            arity(args, 2, op)?;
            let secret = StaticSecret::from(fixed::<32>(args[0], "X25519 private key")?);
            let peer = PublicKey::from(fixed::<32>(args[1], "X25519 public key")?);
            let shared = secret.diffie_hellman(&peer);
            if !shared.was_contributory() {
                return Err(Error::Authentication(
                    "X25519 peer key is not contributory".into(),
                ));
            }
            Ok(shared.as_bytes().to_vec())
        }
        other => Err(Error::Unsupported(format!(
            "unknown X25519 operation {other}"
        ))),
    }
}

macro_rules! ecdsa_curve {
    ($curve:ident, $op:expr, $args:expr, $field:expr) => {{
        use $curve::ecdsa::signature::{Signer, Verifier};
        use $curve::ecdsa::{Signature, SigningKey, VerifyingKey};
        use $curve::pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey};
        let args = $args;
        match $op {
            // Скаляр из байт энтропии: вне диапазона — повтор с новыми байтами у вызывающего.
            "from-scalar" => {
                arity(args, 1, "ecdsa from-scalar")?;
                let secret = $curve::SecretKey::from_slice(args[0]).map_err(|_| invalid("scalar out of range"))?;
                let der = secret.to_pkcs8_der().map_err(|_| Error::Failed("PKCS#8 encoding".into()))?;
                Ok(der.as_bytes().to_vec())
            }
            "public" => {
                arity(args, 1, "ecdsa public")?;
                let key = SigningKey::from_pkcs8_der(args[0]).map_err(|_| invalid("invalid ECDSA PKCS#8 key"))?;
                let der = key.verifying_key().to_public_key_der().map_err(|_| Error::Failed("SPKI encoding".into()))?;
                Ok(der.as_bytes().to_vec())
            }
            "sign" => {
                arity(args, 2, "ecdsa sign")?;
                let key = SigningKey::from_pkcs8_der(args[0]).map_err(|_| invalid("invalid ECDSA PKCS#8 key"))?;
                let signature: Signature = key.sign(args[1]);
                Ok(signature.to_der().as_bytes().to_vec())
            }
            "verify" => {
                arity(args, 3, "ecdsa verify")?;
                let key = VerifyingKey::from_public_key_der(args[0]).map_err(|_| invalid("invalid ECDSA SPKI key"))?;
                let valid = match Signature::from_der(args[2]) {
                    Ok(signature) => key.verify(args[1], &signature).is_ok(),
                    Err(_) => false,
                };
                Ok(truth(valid))
            }
            other => Err(Error::Unsupported(format!("unknown ECDSA operation {other}"))),
        }
    }};
}

fn ecdsa(op: &str, args: &[&[u8]]) -> Result<Vec<u8>> {
    let (curve, action) = op
        .split_once('-')
        .ok_or_else(|| invalid("ECDSA operation"))?;
    match curve {
        "p256" => ecdsa_curve!(p256, action, args, 32),
        "p384" => ecdsa_curve!(p384, action, args, 48),
        other => Err(Error::Unsupported(format!("unknown ECDSA curve {other}"))),
    }
}

/// RSA через aws-lc: закрытые операции (генерация, подпись, расшифровка OAEP)
/// постоянного времени с ослеплением (RUSTSEC-2023-0071 у крейта `rsa`, #70).
/// Случайность генерации, соли PSS и OAEP берётся из CSPRNG aws-lc;
/// энтропия вызывающего остаётся аргументом ABI и не используется.
fn rsa_op(op: &str, args: &[&[u8]]) -> Result<Vec<u8>> {
    use aws_lc_rs::encoding::AsDer;
    use aws_lc_rs::rand::SystemRandom;
    use aws_lc_rs::rsa::{
        KeyPair, KeySize, OaepAlgorithm, OaepPrivateDecryptingKey, OaepPublicEncryptingKey, PrivateDecryptingKey,
        PublicEncryptingKey,
    };
    use aws_lc_rs::signature::{self, KeyPair as _, RsaEncoding};
    use crate::rsa_public::{Hash, PublicKey};
    let private = |der: &[u8]| -> Result<KeyPair> {
        KeyPair::from_pkcs8(der).map_err(|_| invalid("invalid RSA PKCS#8 key"))
    };
    // Открытый ключ SPKI для проверки подписей: те же модули, что у X.509.
    let public = |der: &[u8]| -> Result<PublicKey> {
        crate::x509::rsa_spki(der).ok_or_else(|| invalid("invalid RSA SPKI key"))
    };
    let (action, hash) = op.split_once(':').unwrap_or((op, ""));
    let (digest, pss, pkcs1, oaep): (Hash, &'static dyn RsaEncoding, &'static dyn RsaEncoding, &'static OaepAlgorithm) =
        match hash {
            "sha256" => (Hash::Sha256, &signature::RSA_PSS_SHA256, &signature::RSA_PKCS1_SHA256, &aws_lc_rs::rsa::OAEP_SHA256_MGF1SHA256),
            "sha384" => (Hash::Sha384, &signature::RSA_PSS_SHA384, &signature::RSA_PKCS1_SHA384, &aws_lc_rs::rsa::OAEP_SHA384_MGF1SHA384),
            "sha512" => (Hash::Sha512, &signature::RSA_PSS_SHA512, &signature::RSA_PKCS1_SHA512, &aws_lc_rs::rsa::OAEP_SHA512_MGF1SHA512),
            "" if matches!(action, "generate" | "public" | "bits") => {
                (Hash::Sha256, &signature::RSA_PSS_SHA256, &signature::RSA_PKCS1_SHA256, &aws_lc_rs::rsa::OAEP_SHA256_MGF1SHA256)
            }
            other => return Err(Error::Unsupported(format!("unknown RSA hash {other}"))),
        };
    let hashed = |message: &[u8]| -> Vec<u8> {
        match digest {
            Hash::Sha384 => sha2::Sha384::digest(message).to_vec(),
            Hash::Sha512 => sha2::Sha512::digest(message).to_vec(),
            _ => sha2::Sha256::digest(message).to_vec(),
        }
    };
    let sign = |key: &KeyPair, encoding: &'static dyn RsaEncoding, message: &[u8]| -> Result<Vec<u8>> {
        let mut out = vec![0; key.public_modulus_len()];
        key.sign(encoding, &SystemRandom::new(), message, &mut out)
            .map_err(|_| Error::Failed("RSA signing".into()))?;
        Ok(out)
    };
    match action {
        "generate" => {
            arity(args, 2, op)?;
            let size = match number(args[0], "bits")? {
                2048 => KeySize::Rsa2048,
                3072 => KeySize::Rsa3072,
                4096 => KeySize::Rsa4096,
                _ => return Err(invalid("RSA modulus must be 2048, 3072 or 4096 bits")),
            };
            let key = KeyPair::generate(size).map_err(|_| Error::Failed("RSA key generation".into()))?;
            Ok(key.as_der().map_err(|_| Error::Failed("PKCS#8 encoding".into()))?.as_ref().to_vec())
        }
        "public" => {
            arity(args, 1, op)?;
            let key = private(args[0])?;
            Ok(key.public_key().as_der().map_err(|_| Error::Failed("SPKI encoding".into()))?.as_ref().to_vec())
        }
        "bits" => {
            arity(args, 1, op)?;
            Ok(public(args[0])?.bits().to_string().into_bytes())
        }
        "pss-sign" => {
            arity(args, 3, op)?;
            sign(&private(args[0])?, pss, args[1])
        }
        "pss-verify" => {
            arity(args, 3, op)?;
            let salt = hashed(b"").len();
            Ok(truth(public(args[0])?.pss(digest, digest, salt, &hashed(args[1]), args[2])))
        }
        "pkcs1-sign" => {
            arity(args, 2, op)?;
            sign(&private(args[0])?, pkcs1, args[1])
        }
        "pkcs1-verify" => {
            arity(args, 3, op)?;
            Ok(truth(public(args[0])?.pkcs1(digest, &hashed(args[1]), args[2])))
        }
        "oaep-encrypt" => {
            arity(args, 3, op)?;
            let key = PublicEncryptingKey::from_der(args[0]).map_err(|_| invalid("invalid RSA SPKI key"))?;
            let key = OaepPublicEncryptingKey::new(key).map_err(|_| invalid("invalid RSA SPKI key"))?;
            let mut out = vec![0; key.ciphertext_size()];
            let sealed = key
                .encrypt(oaep, args[1], &mut out, None)
                .map_err(|_| invalid("message too long for RSA-OAEP"))?;
            Ok(sealed.to_vec())
        }
        "oaep-decrypt" => {
            arity(args, 3, op)?;
            let key = PrivateDecryptingKey::from_pkcs8(args[0]).map_err(|_| invalid("invalid RSA PKCS#8 key"))?;
            let key = OaepPrivateDecryptingKey::new(key).map_err(|_| invalid("invalid RSA PKCS#8 key"))?;
            let mut out = vec![0; key.min_output_size()];
            let opened = key
                .decrypt(oaep, args[1], &mut out, None)
                .map_err(|_| Error::Authentication("RSA-OAEP decryption failed".into()))?;
            Ok(opened.to_vec())
        }
        other => Err(Error::Unsupported(format!("unknown RSA operation {other}"))),
    }
}

/// OID алгоритма ключа PKCS#8 и, у EC, OID кривой: `oid` или `oid|curve`.
fn pkcs8_algorithm(der: &[u8]) -> Result<Vec<u8>> {
    let info =
        pkcs8::PrivateKeyInfoRef::try_from(der).map_err(|_| invalid("invalid PKCS#8 key"))?;
    Ok(algorithm_text(&info.algorithm).into_bytes())
}

fn spki_algorithm(der: &[u8]) -> Result<Vec<u8>> {
    let info = spki::SubjectPublicKeyInfoRef::try_from(der)
        .map_err(|_| invalid("invalid SubjectPublicKeyInfo"))?;
    Ok(algorithm_text(&info.algorithm).into_bytes())
}

fn algorithm_text(algorithm: &spki::AlgorithmIdentifierRef<'_>) -> String {
    let mut text = algorithm.oid.to_string();
    if let Some(parameters) = &algorithm.parameters
        && let Ok(curve) = parameters.decode_as::<spki::ObjectIdentifier>() {
            text.push('|');
            text.push_str(&curve.to_string());
        }
    text
}
