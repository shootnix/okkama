//! Подписи сертификатов X.509: проверка `signatureValue` над `tbsCertificate`
//! открытым ключом издателя по `AlgorithmIdentifier` подписи, как
//! `X509Certificate.verify` JDK. Ключи — DER `SubjectPublicKeyInfo`.

use crate::ops::Error;
use sha2::Digest;

type Result<T> = std::result::Result<T, Error>;

fn invalid(text: &str) -> Error {
    Error::Invalid(text.into())
}

/// Узел DER: тег, содержимое и весь закодированный узел.
struct Tlv<'a> {
    tag: u8,
    content: &'a [u8],
    encoded: &'a [u8],
}

fn tlv(source: &[u8]) -> Result<(Tlv<'_>, &[u8])> {
    let bad = || invalid("invalid DER");
    let tag = *source.first().ok_or_else(bad)?;
    let first = usize::from(*source.get(1).ok_or_else(bad)?);
    let (length, header) = if first < 128 {
        (first, 2)
    } else {
        let width = first & 127;
        if width == 0 || width > 4 || source.len() < 2 + width {
            return Err(bad());
        }
        let length = source[2..2 + width].iter().fold(0usize, |acc, b| (acc << 8) | usize::from(*b));
        (length, 2 + width)
    };
    let end = header.checked_add(length).filter(|end| *end <= source.len()).ok_or_else(bad)?;
    Ok((
        Tlv {
            tag,
            content: &source[header..end],
            encoded: &source[..end],
        },
        &source[end..],
    ))
}

fn children(content: &[u8]) -> Result<Vec<Tlv<'_>>> {
    let mut out = Vec::new();
    let mut rest = content;
    while !rest.is_empty() {
        let (node, tail) = tlv(rest)?;
        out.push(node);
        rest = tail;
    }
    Ok(out)
}

/// Текст OID из содержимого DER.
fn oid(content: &[u8]) -> String {
    let mut arcs: Vec<u128> = Vec::new();
    let mut value: u128 = 0;
    for octet in content {
        value = (value << 7) | u128::from(octet & 127);
        if octet & 128 == 0 {
            arcs.push(value);
            value = 0;
        }
    }
    let Some((&first, rest)) = arcs.split_first() else {
        return String::new();
    };
    let root = if first < 40 { 0 } else if first < 80 { 1 } else { 2 };
    let mut text = format!("{root}.{}", first - 40 * root);
    for arc in rest {
        text.push_str(&format!(".{arc}"));
    }
    text
}

/// Алгоритм: OID и необязательные параметры.
fn algorithm(der: &[u8]) -> Result<(String, Option<Tlv<'_>>)> {
    let (node, _) = tlv(der)?;
    let parts = children(node.content)?;
    let identifier = parts
        .first()
        .filter(|part| part.tag == 6)
        .map(|part| oid(part.content))
        .ok_or_else(|| invalid("AlgorithmIdentifier"))?;
    let parameters = parts.into_iter().nth(1);
    Ok((identifier, parameters))
}

/// Открытый ключ SPKI: OID алгоритма, параметры и биты ключа.
fn public_key(spki: &[u8]) -> Result<(String, Option<String>, Vec<u8>)> {
    let (node, _) = tlv(spki)?;
    let parts = children(node.content)?;
    if parts.len() != 2 || parts[1].tag != 3 || parts[1].content.first() != Some(&0) {
        return Err(invalid("SubjectPublicKeyInfo"));
    }
    let (identifier, parameters) = algorithm(parts[0].encoded)?;
    let curve = parameters.filter(|p| p.tag == 6).map(|p| oid(p.content));
    Ok((identifier, curve, parts[1].content[1..].to_vec()))
}

fn unsigned(content: &[u8]) -> &[u8] {
    let mut bytes = content;
    while bytes.len() > 1 && bytes[0] == 0 {
        bytes = &bytes[1..];
    }
    bytes
}

fn rsa_key(bits: &[u8]) -> Result<rsa::RsaPublicKey> {
    let (node, _) = tlv(bits)?;
    let parts = children(node.content)?;
    if node.tag != 0x30 || parts.len() != 2 || parts[0].tag != 2 || parts[1].tag != 2 {
        return Err(invalid("RSAPublicKey"));
    }
    let n = unsigned(parts[0].content);
    let e = unsigned(parts[1].content);
    let width = |bytes: &[u8]| (bytes.len() * 8) as u32;
    let n = rsa::BoxedUint::from_be_slice(n, width(n)).map_err(|_| invalid("RSA modulus"))?;
    let e = rsa::BoxedUint::from_be_slice(e, width(e)).map_err(|_| invalid("RSA exponent"))?;
    // JDK принимает модули до 16384 бит.
    rsa::RsaPublicKey::new_with_max_size(n, e, 16384).map_err(|_| invalid("RSA public key"))
}

/// Сведения о ключе SPKI: `rsa <bits>`, `ec <bits>`, `ed25519` и т. п.
pub fn key(spki: &[u8]) -> Result<String> {
    let (identifier, curve, bits) = public_key(spki)?;
    match identifier.as_str() {
        "1.2.840.113549.1.1.1" | "1.2.840.113549.1.1.10" => {
            use rsa::traits::PublicKeyParts;
            Ok(format!("rsa {}", rsa_key(&bits)?.n().bits()))
        }
        "1.2.840.10045.2.1" => {
            let (field, size) = match curve.as_deref() {
                Some("1.2.840.10045.3.1.7") => (32, 256),
                Some("1.3.132.0.34") => (48, 384),
                Some("1.3.132.0.35") => (66, 521),
                _ => return Err(Error::Unsupported("unknown named curve".into())),
            };
            // ECPublicKeyImpl JDK: только несжатая точка нужной ширины.
            if bits.len() != 1 + 2 * field || bits[0] != 4 {
                return Err(invalid("EC point"));
            }
            Ok(format!("ec {size}"))
        }
        "1.3.101.112" => (bits.len() == 32).then(|| "ed25519".to_string()).ok_or_else(|| invalid("Ed25519 key")),
        "1.3.101.113" => (bits.len() == 57).then(|| "ed448".to_string()).ok_or_else(|| invalid("Ed448 key")),
        "1.3.101.110" => (bits.len() == 32).then(|| "x25519".to_string()).ok_or_else(|| invalid("X25519 key")),
        "1.3.101.111" => (bits.len() == 56).then(|| "x448".to_string()).ok_or_else(|| invalid("X448 key")),
        "1.2.840.10040.4.1" => Ok("dsa".to_string()),
        other => Ok(format!("other {other}")),
    }
}

#[derive(Clone, Copy)]
enum Hash {
    Sha1,
    Sha224,
    Sha256,
    Sha384,
    Sha512,
}

fn hash_oid(oid: &str) -> Option<Hash> {
    Some(match oid {
        "1.3.14.3.2.26" => Hash::Sha1,
        "2.16.840.1.101.3.4.2.4" => Hash::Sha224,
        "2.16.840.1.101.3.4.2.1" => Hash::Sha256,
        "2.16.840.1.101.3.4.2.2" => Hash::Sha384,
        "2.16.840.1.101.3.4.2.3" => Hash::Sha512,
        _ => return None,
    })
}

fn hashed(hash: Hash, message: &[u8]) -> Vec<u8> {
    match hash {
        Hash::Sha1 => sha1::Sha1::digest(message).to_vec(),
        Hash::Sha224 => sha2::Sha224::digest(message).to_vec(),
        Hash::Sha256 => sha2::Sha256::digest(message).to_vec(),
        Hash::Sha384 => sha2::Sha384::digest(message).to_vec(),
        Hash::Sha512 => sha2::Sha512::digest(message).to_vec(),
    }
}

fn pkcs1(key: &rsa::RsaPublicKey, hash: Hash, message: &[u8], signature: &[u8]) -> bool {
    let scheme = match hash {
        Hash::Sha1 => rsa::Pkcs1v15Sign::new::<sha1::Sha1>(),
        Hash::Sha224 => rsa::Pkcs1v15Sign::new::<sha2::Sha224>(),
        Hash::Sha256 => rsa::Pkcs1v15Sign::new::<sha2::Sha256>(),
        Hash::Sha384 => rsa::Pkcs1v15Sign::new::<sha2::Sha384>(),
        Hash::Sha512 => rsa::Pkcs1v15Sign::new::<sha2::Sha512>(),
    };
    key.verify(scheme, &hashed(hash, message), signature).is_ok()
}

/// RSASSA-PSS-params: хэш, MGF1 с хэшем, длина соли, trailer.
fn pss_parameters(parameters: Option<Tlv<'_>>) -> Result<(Hash, Hash, usize)> {
    let mut hash = Hash::Sha1;
    let mut mask = Hash::Sha1;
    let mut salt = 20usize;
    let Some(parameters) = parameters else {
        return Ok((hash, mask, salt));
    };
    for field in children(parameters.content)? {
        match field.tag {
            0xa0 => {
                let (identifier, _) = algorithm(field.content)?;
                hash = hash_oid(&identifier).ok_or_else(|| Error::Unsupported("PSS hash".into()))?;
            }
            0xa1 => {
                let (identifier, inner) = algorithm(field.content)?;
                if identifier != "1.2.840.113549.1.1.8" {
                    return Err(Error::Unsupported("PSS mask generation".into()));
                }
                let inner = inner.ok_or_else(|| invalid("MGF1 parameters"))?;
                let (identifier, _) = algorithm(inner.encoded)?;
                mask = hash_oid(&identifier).ok_or_else(|| Error::Unsupported("MGF1 hash".into()))?;
            }
            0xa2 => {
                let (value, _) = tlv(field.content)?;
                salt = unsigned(value.content).iter().fold(0usize, |acc, b| acc.saturating_mul(256).saturating_add(usize::from(*b)));
            }
            0xa3 => {
                let (value, _) = tlv(field.content)?;
                if unsigned(value.content) != [1] {
                    return Err(Error::Unsupported("PSS trailer".into()));
                }
            }
            _ => return Err(invalid("RSASSA-PSS-params")),
        }
    }
    Ok((hash, mask, salt))
}

fn pss(key: &rsa::RsaPublicKey, parameters: Option<Tlv<'_>>, message: &[u8], signature: &[u8]) -> Result<bool> {
    let (hash, mask, salt) = pss_parameters(parameters)?;
    if !std::mem::discriminant(&hash).eq(&std::mem::discriminant(&mask)) {
        return Err(Error::Unsupported("PSS with distinct MGF1 hash".into()));
    }
    let digest = hashed(hash, message);
    let valid = match hash {
        Hash::Sha1 => key.verify(rsa::Pss::<sha1::Sha1>::new_with_salt(salt), &digest, signature),
        Hash::Sha224 => key.verify(rsa::Pss::<sha2::Sha224>::new_with_salt(salt), &digest, signature),
        Hash::Sha256 => key.verify(rsa::Pss::<sha2::Sha256>::new_with_salt(salt), &digest, signature),
        Hash::Sha384 => key.verify(rsa::Pss::<sha2::Sha384>::new_with_salt(salt), &digest, signature),
        Hash::Sha512 => key.verify(rsa::Pss::<sha2::Sha512>::new_with_salt(salt), &digest, signature),
    };
    Ok(valid.is_ok())
}

fn ecdsa(spki: &[u8], curve: Option<&str>, hash: Hash, message: &[u8], signature: &[u8]) -> Result<bool> {
    use p256::ecdsa::signature::hazmat::PrehashVerifier;
    let digest = hashed(hash, message);
    macro_rules! verify {
        ($curve:ident) => {{
            use $curve::pkcs8::DecodePublicKey;
            let key = $curve::ecdsa::VerifyingKey::from_public_key_der(spki).map_err(|_| invalid("EC public key"))?;
            match $curve::ecdsa::DerSignature::try_from(signature) {
                Ok(signature) => key.verify_prehash(&digest, &signature).is_ok(),
                Err(_) => false,
            }
        }};
    }
    Ok(match curve {
        Some("1.2.840.10045.3.1.7") => verify!(p256),
        Some("1.3.132.0.34") => verify!(p384),
        Some("1.3.132.0.35") => verify!(p521),
        _ => return Err(Error::Unsupported("unknown named curve".into())),
    })
}

/// Подпись `signature` над `message` ключом `spki` по алгоритму `algorithm_der`.
pub fn verify(spki: &[u8], algorithm_der: &[u8], message: &[u8], signature: &[u8]) -> Result<bool> {
    let (identifier, parameters) = algorithm(algorithm_der)?;
    let (key_algorithm, curve, bits) = public_key(spki)?;
    let rsa_hash = match identifier.as_str() {
        "1.2.840.113549.1.1.5" => Some(Hash::Sha1),
        "1.2.840.113549.1.1.14" => Some(Hash::Sha224),
        "1.2.840.113549.1.1.11" => Some(Hash::Sha256),
        "1.2.840.113549.1.1.12" => Some(Hash::Sha384),
        "1.2.840.113549.1.1.13" => Some(Hash::Sha512),
        _ => None,
    };
    if let Some(hash) = rsa_hash {
        if key_algorithm != "1.2.840.113549.1.1.1" {
            return Ok(false);
        }
        return Ok(pkcs1(&rsa_key(&bits)?, hash, message, signature));
    }
    let ec_hash = match identifier.as_str() {
        "1.2.840.10045.4.1" => Some(Hash::Sha1),
        "1.2.840.10045.4.3.1" => Some(Hash::Sha224),
        "1.2.840.10045.4.3.2" => Some(Hash::Sha256),
        "1.2.840.10045.4.3.3" => Some(Hash::Sha384),
        "1.2.840.10045.4.3.4" => Some(Hash::Sha512),
        _ => None,
    };
    if let Some(hash) = ec_hash {
        if key_algorithm != "1.2.840.10045.2.1" {
            return Ok(false);
        }
        return ecdsa(spki, curve.as_deref(), hash, message, signature);
    }
    match identifier.as_str() {
        "1.2.840.113549.1.1.10" => {
            if key_algorithm != "1.2.840.113549.1.1.1" && key_algorithm != "1.2.840.113549.1.1.10" {
                return Ok(false);
            }
            pss(&rsa_key(&bits)?, parameters, message, signature)
        }
        "1.3.101.112" => {
            if key_algorithm != "1.3.101.112" {
                return Ok(false);
            }
            let key: [u8; 32] = bits.as_slice().try_into().map_err(|_| invalid("Ed25519 key"))?;
            let key = ed25519_dalek::VerifyingKey::from_bytes(&key).map_err(|_| invalid("Ed25519 key"))?;
            let Ok(signature) = <[u8; 64]>::try_from(signature) else {
                return Ok(false);
            };
            Ok(key.verify_strict(message, &ed25519_dalek::Signature::from_bytes(&signature)).is_ok())
        }
        other => Err(Error::Unsupported(format!("signature algorithm {other}"))),
    }
}
