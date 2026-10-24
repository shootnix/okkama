//! Известные ответы (NIST, RFC) и отказы операций хостовой криптографии.

use okkama_native::{Error, Hasher, call};

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap())
        .collect()
}

fn digest(algorithm: &str, data: &[u8], length: usize) -> Vec<u8> {
    let mut hasher = Hasher::new(algorithm, &[], length).unwrap();
    hasher.update(data);
    hasher.finish(length).unwrap()
}

#[test]
fn digests_match_reference_vectors() {
    let cases = [
        (
            "sha256",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            0,
        ),
        (
            "sha224",
            "23097d223405d8228642a477bda255b32aadbce4bda0b3f7e36c9da7",
            0,
        ),
        (
            "sha512-256",
            "53048e2681941ef99b2e29b76b4c7dabe4c2d0c634fc6d46e0e2f13107e7af23",
            0,
        ),
        (
            "sha3-256",
            "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532",
            0,
        ),
        (
            "blake2s",
            "508c5e8c327c14e2e1a72ba34eeb452f37458b209ed63a294d999b4c86675982",
            32,
        ),
        (
            "blake2b",
            "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d17d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923",
            64,
        ),
    ];
    for (algorithm, expected, length) in cases {
        assert_eq!(
            digest(algorithm, b"abc", length),
            hex(expected),
            "{algorithm}"
        );
    }
    assert_eq!(
        digest("shake128", b"", 32),
        hex("7f9c2ba4e88f827d616045507605853ed73b8093f6efbc88eb1a6eacfa66ef26")
    );
}

#[test]
fn digest_states_branch_without_changing_the_origin() {
    let mut origin = Hasher::new("sha256", &[], 0).unwrap();
    origin.update(b"ab");
    let mut branch = origin.clone();
    branch.update(b"c");
    assert_eq!(branch.finish(0).unwrap(), digest("sha256", b"abc", 0));
    assert_eq!(origin.finish(0).unwrap(), digest("sha256", b"ab", 0));
}

#[test]
fn macs_and_key_derivation_match_rfc_vectors() {
    let mut hmac = Hasher::new("hmac-sha256", &[0x0b; 20], 0).unwrap();
    hmac.update(b"Hi There");
    assert_eq!(
        hmac.finish(0).unwrap(),
        hex("b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7")
    );
    let okm = call(
        "hkdf-sha256",
        &[
            &[0x0b; 22],
            &hex("000102030405060708090a0b0c"),
            &hex("f0f1f2f3f4f5f6f7f8f9"),
            b"42",
        ],
    )
    .unwrap();
    assert_eq!(
        okm,
        hex("3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865")
    );
    let derived = call("pbkdf2-sha256", &[b"password", b"salt", b"1", b"32"]).unwrap();
    assert_eq!(
        derived,
        hex("120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b")
    );
    assert!(matches!(
        call("pbkdf2-sha256", &[b"p", b"s", b"0", b"32"]),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn argon2id_hashes_verify_and_reject_wrong_passwords() {
    let phc = call(
        "argon2-phc",
        &[b"hunter2", b"saltsaltsaltsalt", b"19456", b"2", b"1"],
    )
    .unwrap();
    let text = String::from_utf8(phc.clone()).unwrap();
    assert!(
        text.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"),
        "{text}"
    );
    assert_eq!(call("argon2-verify", &[&phc, b"hunter2"]).unwrap(), [1]);
    assert_eq!(call("argon2-verify", &[&phc, b"hunter3"]).unwrap(), [0]);
    let raw = call(
        "argon2",
        &[b"hunter2", b"saltsaltsaltsalt", b"19456", b"2", b"1", b"32"],
    )
    .unwrap();
    assert_eq!(raw.len(), 32);
}

#[test]
fn aead_matches_gcm_vectors_and_rejects_tampering() {
    let sealed = call(
        "aead-seal:aes-128-gcm",
        &[&[0; 16], &[0; 12], b"", &[0; 16]],
    )
    .unwrap();
    assert_eq!(
        sealed,
        hex("0388dace60b6a392f328c2b971b2fe78ab6e47d42cec13bdf53a67b21257bddf")
    );
    for algorithm in ["aes-256-gcm", "chacha20-poly1305", "xchacha20-poly1305"] {
        let nonce = vec![7; if algorithm.starts_with('x') { 24 } else { 12 }];
        let key = [9; 32];
        let sealed = call(
            &format!("aead-seal:{algorithm}"),
            &[&key, &nonce, b"header", b"secret"],
        )
        .unwrap();
        assert_eq!(
            call(
                &format!("aead-open:{algorithm}"),
                &[&key, &nonce, b"header", &sealed]
            )
            .unwrap(),
            b"secret"
        );
        let mut tampered = sealed.clone();
        tampered[0] ^= 1;
        assert!(matches!(
            call(
                &format!("aead-open:{algorithm}"),
                &[&key, &nonce, b"header", &tampered]
            ),
            Err(Error::Authentication(_))
        ));
        assert!(matches!(
            call(
                &format!("aead-open:{algorithm}"),
                &[&key, &nonce, b"other", &sealed]
            ),
            Err(Error::Authentication(_))
        ));
        assert!(matches!(
            call(
                &format!("aead-seal:{algorithm}"),
                &[&key[..31], &nonce, b"", b""]
            ),
            Err(Error::Invalid(_))
        ));
    }
}

#[test]
fn ed25519_and_x25519_match_rfc_vectors() {
    let seed = hex("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
    let public = call("ed25519-public", &[&seed]).unwrap();
    assert_eq!(
        public,
        hex("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a")
    );
    let signature = call("ed25519-sign", &[&seed, b""]).unwrap();
    assert_eq!(
        signature,
        hex(
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"
        )
    );
    assert_eq!(
        call("ed25519-verify", &[&public, b"", &signature]).unwrap(),
        [1]
    );
    assert_eq!(
        call("ed25519-verify", &[&public, b"x", &signature]).unwrap(),
        [0]
    );
    let pkcs8 = call("ed25519-pkcs8", &[&seed]).unwrap();
    assert_eq!(call("ed25519-from-pkcs8", &[&pkcs8]).unwrap(), seed);
    assert_eq!(call("pkcs8-algorithm", &[&pkcs8]).unwrap(), b"1.3.101.112");

    let alice = hex("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
    assert_eq!(
        call("x25519-public", &[&alice]).unwrap(),
        hex("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a")
    );
    let bob = hex("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f");
    assert_eq!(
        call("x25519-shared", &[&alice, &bob]).unwrap(),
        hex("4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742")
    );
    assert!(matches!(
        call("x25519-shared", &[&alice, &[0; 32]]),
        Err(Error::Authentication(_))
    ));
}

#[test]
fn ecdsa_signs_and_verifies_on_both_curves() {
    for (curve, width, oid) in [
        ("p256", 32, "1.2.840.10045.3.1.7"),
        ("p384", 48, "1.3.132.0.34"),
    ] {
        let scalar = vec![0x42; width];
        let private = call(&format!("ecdsa-{curve}-from-scalar"), &[&scalar]).unwrap();
        let public = call(&format!("ecdsa-{curve}-public"), &[&private]).unwrap();
        assert_eq!(
            String::from_utf8(call("spki-algorithm", &[&public]).unwrap()).unwrap(),
            format!("1.2.840.10045.2.1|{oid}")
        );
        let signature = call(&format!("ecdsa-{curve}-sign"), &[&private, b"message"]).unwrap();
        // RFC 6979: подпись детерминирована.
        assert_eq!(
            call(&format!("ecdsa-{curve}-sign"), &[&private, b"message"]).unwrap(),
            signature
        );
        assert_eq!(
            call(
                &format!("ecdsa-{curve}-verify"),
                &[&public, b"message", &signature]
            )
            .unwrap(),
            [1]
        );
        assert_eq!(
            call(
                &format!("ecdsa-{curve}-verify"),
                &[&public, b"massage", &signature]
            )
            .unwrap(),
            [0]
        );
        assert_eq!(
            call(
                &format!("ecdsa-{curve}-verify"),
                &[&public, b"message", b"junk"]
            )
            .unwrap(),
            [0]
        );
        assert!(call(&format!("ecdsa-{curve}-from-scalar"), &[&vec![0; width]]).is_err());
    }
}

#[test]
fn rsa_signatures_and_oaep_round_trip() {
    let private = call("rsa-generate", &[b"2048", &[1; 32]]).unwrap();
    let public = call("rsa-public", &[&private]).unwrap();
    assert_eq!(call("rsa-bits", &[&public]).unwrap(), b"2048");
    for scheme in ["pss", "pkcs1"] {
        let signature = if scheme == "pss" {
            call("rsa-pss-sign:sha256", &[&private, b"message", &[2; 32]]).unwrap()
        } else {
            call("rsa-pkcs1-sign:sha256", &[&private, b"message"]).unwrap()
        };
        assert_eq!(signature.len(), 256);
        let verify = format!("rsa-{scheme}-verify:sha256");
        assert_eq!(
            call(&verify, &[&public, b"message", &signature]).unwrap(),
            [1]
        );
        assert_eq!(
            call(&verify, &[&public, b"messagE", &signature]).unwrap(),
            [0]
        );
    }
    let cipher = call("rsa-oaep-encrypt:sha256", &[&public, b"secret", &[3; 32]]).unwrap();
    assert_eq!(
        call("rsa-oaep-decrypt:sha256", &[&private, &cipher, &[4; 32]]).unwrap(),
        b"secret"
    );
    let mut broken = cipher.clone();
    broken[10] ^= 1;
    assert!(matches!(
        call("rsa-oaep-decrypt:sha256", &[&private, &broken, &[4; 32]]),
        Err(Error::Authentication(_))
    ));
    assert!(matches!(
        call("rsa-generate", &[b"1024", &[1; 32]]),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn constant_time_equality_and_entropy() {
    assert_eq!(call("ct-equal", &[b"abc", b"abc"]).unwrap(), [1]);
    assert_eq!(call("ct-equal", &[b"abc", b"abd"]).unwrap(), [0]);
    assert_eq!(call("ct-equal", &[b"abc", b"ab"]).unwrap(), [0]);
    let first = call("random", &[b"32"]).unwrap();
    assert_eq!(first.len(), 32);
    assert_ne!(first, call("random", &[b"32"]).unwrap());
}
