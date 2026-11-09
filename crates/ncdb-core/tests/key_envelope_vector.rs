//! The key envelope's test vectors. RFC 0003 gives none of its own, so
//! tests/vectors/key_envelope.py writes one with an implementation of
//! RFC 9180 from Python's cryptography primitives, which it checks against
//! RFC 9180's vector A.2.1 and opens with cryptography's own HPKE. The suite
//! itself runs RFC 9180's vector A.2.1 here, as RFC 0003 decision 3 asks.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::convert::Infallible;

use hpke::rand_core::{TryCryptoRng, TryRng};
use hpke::{Deserializable, Kem as _, OpModeS, Serializable};
use ncdb_core::cbor::{self, Value};
use ncdb_core::envelope::{self, KeyEnvelope, suite};
use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_core::keys::{MemberKey, SpaceKey};

const MEMBER_ID: &str = include_str!("vectors/key-envelope.member-id.hex");
const INFO: &str = include_str!("vectors/key-envelope.info.hex");
const ENC: &str = include_str!("vectors/key-envelope.enc.hex");
const CIPHERTEXT: &str = include_str!("vectors/key-envelope.ciphertext.hex");
const FIELDS: &str = include_str!("vectors/key-envelope.fields.hex");

const SPACE: SpaceId = SpaceId([
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
]);
const GENERATION: u64 = (1 << 48) | 0xa1b2_c3d4_e5f6;

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn recipient() -> MemberKey {
    MemberKey::from_secrets(
        std::array::from_fn(|i| 0x10 + i as u8),
        std::array::from_fn(|i| 0x60 + i as u8),
    )
}

fn space_key() -> SpaceKey {
    SpaceKey::from_bytes(std::array::from_fn(|i| 0xc0 + i as u8))
}

fn ikm() -> [u8; 32] {
    std::array::from_fn(|i| 0x90 + i as u8)
}

fn vector() -> KeyEnvelope {
    KeyEnvelope {
        recipient: MemberId(hex(MEMBER_ID).try_into().unwrap()),
        generation: GENERATION,
        encapsulated_key: hex(ENC),
        ciphertext: hex(CIPHERTEXT),
    }
}

#[test]
fn the_recipient_has_the_vectors_member_id() {
    assert_eq!(recipient().id().0.to_vec(), hex(MEMBER_ID));
}

#[test]
fn the_info_binds_space_generation_and_recipient() {
    assert_eq!(
        envelope::info(SPACE, GENERATION, recipient().id()),
        hex(INFO)
    );
}

#[test]
fn sealing_with_the_vectors_ephemeral_gives_its_bytes() {
    let sealed = envelope::seal_with_ephemeral(
        &space_key(),
        SPACE,
        GENERATION,
        &recipient().public(),
        ikm(),
    );
    assert_eq!(sealed, Ok(vector()));
}

#[test]
fn the_vector_opens_to_the_space_key() {
    let opened = envelope::open(&vector(), SPACE, &recipient()).unwrap();
    // A space key does not show its bytes; sealing with the opened key and
    // the same ephemeral gives the vector again only if it is the same key.
    let again =
        envelope::seal_with_ephemeral(&opened, SPACE, GENERATION, &recipient().public(), ikm());
    assert_eq!(again, Ok(vector()));
}

#[test]
fn the_fields_encode_to_the_vectors_bytes() {
    assert_eq!(cbor::encode(&Value::Map(vector().fields())), hex(FIELDS));
}

#[test]
fn the_vectors_bytes_decode_to_the_fields() {
    let Value::Map(fields) = cbor::decode(&hex(FIELDS)).unwrap() else {
        panic!("the fields are a map");
    };
    assert_eq!(KeyEnvelope::from_fields(&fields), Ok(vector()));
}

/// Hands out a fixed ikmE once, as RFC 9180's vectors need.
struct FixedIkm(Option<[u8; 32]>);

impl TryRng for FixedIkm {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        unreachable!("HPKE draws the ephemeral key's input in one call")
    }

    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        unreachable!("HPKE draws the ephemeral key's input in one call")
    }

    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Infallible> {
        dst.copy_from_slice(&self.0.take().expect("one ephemeral key per seal"));
        Ok(())
    }
}

impl TryCryptoRng for FixedIkm {}

// RFC 9180, A.2.1 and A.2.1.1, sequence number 0: DHKEM(X25519,
// HKDF-SHA256), HKDF-SHA256, ChaCha20Poly1305, base mode.
const A2_IKM_R: &str = "1ac01f181fdf9f352797655161c58b75c656a6cc2716dcb66372da835542e1df";
const A2_PK_R: &str = "4310ee97d88cc1f088a5576c77ab0cf5c3ac797f3d95139c6c84b5429c59662a";
const A2_SK_R: &str = "8057991eef8f1f1af18f4a9491d16a1ce333f695d4db8e38da75975c4478e0fb";
const A2_IKM_E: &str = "909a9b35d3dc4713a5e72a4da274b55d3d3821a37e5d099e74a647db583a904b";
const A2_ENC: &str = "1afa08d3dec047a643885163f1180476fa7ddb54c6a8029ea33f95796bf2ac4a";
const A2_INFO: &str = "4f6465206f6e2061204772656369616e2055726e";
const A2_PT: &str = "4265617574792069732074727574682c20747275746820626561757479";
const A2_AAD: &str = "436f756e742d30";
const A2_CT: &str = "1c5250d8034ec2b784ba2cfd69dbdb8af406cfe3ff938e131f0def8c8b60b4db\
                     21993c62ce81883d2dd1b51a28";

#[test]
fn the_suite_derives_rfc_9180_a2_recipient_key() {
    // The private key is not compared: hpke 0.15 clamps it when it
    // serializes it, and RFC 9180 prints it unclamped. The public key and
    // opening with the RFC's private key below show it is the same key.
    let (_, public) = suite::Kem::derive_keypair(&hex(A2_IKM_R));
    assert_eq!(public.to_bytes().to_vec(), hex(A2_PK_R));
}

#[test]
fn the_suite_seals_rfc_9180_a2() {
    let public = <suite::Kem as hpke::Kem>::PublicKey::from_bytes(&hex(A2_PK_R)).unwrap();
    let mut rng = FixedIkm(Some(hex(A2_IKM_E).try_into().unwrap()));
    let (enc, ciphertext) = hpke::single_shot_seal_with_rng::<suite::Aead, suite::Kdf, suite::Kem>(
        &OpModeS::Base,
        &public,
        &hex(A2_INFO),
        &hex(A2_PT),
        &hex(A2_AAD),
        &mut rng,
    )
    .unwrap();
    assert_eq!(enc.to_bytes().to_vec(), hex(A2_ENC));
    assert_eq!(ciphertext, hex(A2_CT));
}

#[test]
fn the_suite_opens_rfc_9180_a2() {
    let private = <suite::Kem as hpke::Kem>::PrivateKey::from_bytes(&hex(A2_SK_R)).unwrap();
    let enc = <suite::Kem as hpke::Kem>::EncappedKey::from_bytes(&hex(A2_ENC)).unwrap();
    let plaintext = hpke::single_shot_open::<suite::Aead, suite::Kdf, suite::Kem>(
        &hpke::OpModeR::Base,
        &private,
        &enc,
        &hex(A2_INFO),
        &hex(A2_CT),
        &hex(A2_AAD),
    )
    .unwrap();
    assert_eq!(plaintext, hex(A2_PT));
}
