//! The member key and the space key of RFC 0003 ("The keys"): the public
//! halves, the member id, and that no secret shows in `Debug` output.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::ids::MemberId;
use ncdb_core::keys::{MemberKey, MemberPublicKey, SpaceKey};

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn hex32(text: &str) -> [u8; 32] {
    hex(text).try_into().unwrap()
}

// RFC 8032, 7.1, TEST 1: the Ed25519 secret key and its public key.
const ED25519_SECRET: &str = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";
const ED25519_PUBLIC: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
// RFC 7748, 6.1: Alice's X25519 private key and its public key.
const X25519_SECRET: &str = "77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a";
const X25519_PUBLIC: &str = "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a";
// SHA-256("nocentraldb/member-id" || ED25519_PUBLIC || X25519_PUBLIC),
// computed with Python's hashlib.
const MEMBER_ID: &str = "07804931258083e8b89a57cefd9048e6572af5d000fffa68141a573c313934ce";

fn member() -> MemberKey {
    MemberKey::from_secrets(hex32(ED25519_SECRET), hex32(X25519_SECRET))
}

#[test]
fn the_public_halves_are_the_standard_public_keys() {
    let public = member().public();
    assert_eq!(public.signing, hex32(ED25519_PUBLIC));
    assert_eq!(public.agreement, hex32(X25519_PUBLIC));
}

#[test]
fn the_member_id_hashes_the_context_and_both_public_keys() {
    let public = MemberPublicKey {
        signing: hex32(ED25519_PUBLIC),
        agreement: hex32(X25519_PUBLIC),
    };
    assert_eq!(public.id(), MemberId(hex32(MEMBER_ID)));
    assert_eq!(member().id(), MemberId(hex32(MEMBER_ID)));
}

#[test]
fn the_member_id_changes_with_either_public_key() {
    let public = member().public();
    let mut other_signing = public;
    other_signing.signing[0] ^= 1;
    let mut other_agreement = public;
    other_agreement.agreement[31] ^= 0x80;
    assert_ne!(other_signing.id(), public.id());
    assert_ne!(other_agreement.id(), public.id());
}

#[test]
fn generated_keys_differ() {
    let (a, b) = (MemberKey::generate(), MemberKey::generate());
    assert_ne!(a.public().signing, b.public().signing);
    assert_ne!(a.public().agreement, b.public().agreement);
    assert_ne!(format!("{:?}", SpaceKey::generate()), "");
}

/// Every byte of a secret, as hex, as a list of decimal bytes and as
/// `[..]` debug output, so that any way of printing it is caught.
fn leaks(output: &str, secret: &[u8]) -> bool {
    let hex: String = secret.iter().map(|b| format!("{b:02x}")).collect();
    let upper = hex.to_uppercase();
    let decimal = secret
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    output.contains(&hex) || output.contains(&upper) || output.contains(&decimal)
}

#[test]
fn secret_keys_do_not_show_in_debug_output() {
    let key = member();
    for output in [format!("{key:?}"), format!("{key:#?}")] {
        assert!(output.contains("redacted"), "{output}");
        assert!(!leaks(&output, &hex(ED25519_SECRET)), "{output}");
        assert!(!leaks(&output, &hex(X25519_SECRET)), "{output}");
    }
    let space = SpaceKey::from_bytes([0x42; 32]);
    for output in [format!("{space:?}"), format!("{space:#?}")] {
        assert!(output.contains("redacted"), "{output}");
        assert!(!leaks(&output, &[0x42; 32]), "{output}");
    }
}

#[test]
fn public_keys_show_in_debug_output() {
    let output = format!("{:?}", member().public());
    assert!(output.contains(ED25519_PUBLIC), "{output}");
    assert!(output.contains(X25519_PUBLIC), "{output}");
}
