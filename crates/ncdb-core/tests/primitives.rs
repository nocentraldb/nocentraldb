//! The test vectors of the standards behind suite 1 (RFC 0003,
//! "Primitives"), run against the pinned crates, natively and in wasm32:
//! RFC 8032 (Ed25519), RFC 7748 (X25519 public keys), the IRTF XChaCha
//! draft (XChaCha20-Poly1305), RFC 5869 (HKDF-SHA256) and FIPS 180-4
//! (SHA-256). The values are copied from the standards and were checked
//! against Python's `cryptography` and PyNaCl when they were added.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use chacha20poly1305::XChaCha20Poly1305;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use hkdf::Hkdf;
use ncdb_core::keys::MemberKey;
use sha2::{Digest, Sha256};

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn hex32(text: &str) -> [u8; 32] {
    hex(text).try_into().unwrap()
}

/// RFC 8032, 7.1, TEST 1 to TEST 3: secret key, public key, message,
/// signature.
const ED25519: [(&str, &str, &str, &str); 3] = [
    (
        "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
        "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
        "",
        "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
    ),
    (
        "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
        "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
        "72",
        "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
    ),
    (
        "c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7",
        "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
        "af82",
        "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a",
    ),
];

#[test]
fn ed25519_rfc_8032() {
    for (secret, public, message, signature) in ED25519 {
        let key = SigningKey::from_bytes(&hex32(secret));
        assert_eq!(key.verifying_key().to_bytes(), hex32(public));
        let signed = key.sign(&hex(message));
        assert_eq!(signed.to_bytes().to_vec(), hex(signature));
        let verifying = VerifyingKey::from_bytes(&hex32(public)).unwrap();
        let expected = Signature::from_slice(&hex(signature)).unwrap();
        assert!(verifying.verify_strict(&hex(message), &expected).is_ok());
        // The member key derives the same public key from the same seed.
        let member = MemberKey::from_secrets(hex32(secret), [0; 32]);
        assert_eq!(member.public().signing, hex32(public));
    }
}

#[test]
fn x25519_public_keys_rfc_7748() {
    // RFC 7748, 6.1: Alice's and Bob's private and public keys.
    let pairs = [
        (
            "77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a",
            "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a",
        ),
        (
            "5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb",
            "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f",
        ),
    ];
    for (secret, public) in pairs {
        let member = MemberKey::from_secrets([1; 32], hex32(secret));
        assert_eq!(member.public().agreement, hex32(public));
    }
}

#[test]
fn xchacha20_poly1305_draft_irtf_cfrg_xchacha_a_3_1() {
    let plaintext = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
    let aad = hex("50515253c0c1c2c3c4c5c6c7");
    let key: [u8; 32] = std::array::from_fn(|i| 0x80 + i as u8);
    let nonce: [u8; 24] = std::array::from_fn(|i| 0x40 + i as u8);
    let expected = hex(
        "bd6d179d3e83d43b9576579493c0e939572a1700252bfaccbed2902c21396cbb
         731c7f1b0b4aa6440bf3a82f4eda7e39ae64c6708c54c216cb96b72e1213b452
         2f8c9ba40db5d945b11b69b982c1bb9e3f3fac2bc369488f76b2383565d3fff9
         21f9664c97637da9768812f615c68b13b52e
         c0875924c1c7987947deafd8780acf49",
    );
    let cipher = XChaCha20Poly1305::new(&key.into());
    let sealed = cipher
        .encrypt(
            &nonce.into(),
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .unwrap();
    assert_eq!(sealed, expected);
    let opened = cipher
        .decrypt(
            &nonce.into(),
            Payload {
                msg: &expected,
                aad: &aad,
            },
        )
        .unwrap();
    assert_eq!(opened, plaintext);
}

#[test]
fn hkdf_sha256_rfc_5869() {
    // A.1 (basic) and A.3 (no salt, no info), with L = 42.
    let cases: [(&[u8], &[u8], &str); 2] = [
        (
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
            &[0xf0, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9],
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865",
        ),
        (
            &[],
            &[],
            "8da4e775a563c18f715f802a063c5a31b8a11f5c5ee1879ec3454e5f3c738d2d9d201395faa4b61a96c8",
        ),
    ];
    for (salt, info, okm) in cases {
        let salt = (!salt.is_empty()).then_some(salt);
        let mut output = [0; 42];
        Hkdf::<Sha256>::new(salt, &[0x0b; 22])
            .expand(info, &mut output)
            .unwrap();
        assert_eq!(output.to_vec(), hex(okm));
    }
}

#[test]
fn sha256_fips_180_4() {
    let cases: [(&[u8], &str); 2] = [
        (
            b"abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        ),
    ];
    for (message, digest) in cases {
        assert_eq!(Sha256::digest(message).to_vec(), hex(digest));
    }
}
