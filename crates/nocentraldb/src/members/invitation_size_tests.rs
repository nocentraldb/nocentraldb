//! How large a database's own invitation is (NOC-594), so that a format
//! change that grows it is noticed: an app shows the invitation as a QR
//! code, and every byte makes that code denser. The sizes are RFC 0003's,
//! "Invitation"; the QR code each needs is in the TypeScript guide,
//! "Show the invitation as a QR code".

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so these tests run on both targets (NOC-175).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use crate::{Db, OpenOptions};

use super::tests::{database, now};

const LOCK: &str = include_str!("../../tests/model/model.lock.json");
const PREFIX: &str = "ncdb-invite-1:";

/// The invitation's text and the length of the bytes its text carries.
fn measure(db: &Db) -> (String, usize) {
    let text = now(db.invitation()).to_string();
    let rest = text.strip_prefix(PREFIX).expect("the invitation's prefix");
    let bytes = data_encoding::BASE64URL_NOPAD
        .decode(rest.as_bytes())
        .expect("unpadded base64url");
    (text, bytes.len())
}

/// Version 2, which a new device shows while it waits to be paired: 66
/// bytes, 102 characters, a QR code of version 6 at level M in byte mode.
#[test]
fn a_pairing_invitation_is_66_bytes_and_102_characters() {
    let db = now(Db::open(OpenOptions::new(LOCK).in_memory().pairing())).unwrap();

    let (text, bytes) = measure(&db);

    assert_eq!(bytes, 66);
    assert_eq!(text.len(), 102);
}

/// Version 1 with the account key statement, which a member of an account
/// shows to join a space: 267 bytes, 370 characters, a QR code of version
/// 15 at level M in byte mode.
#[test]
fn a_certified_invitation_is_267_bytes_and_370_characters() {
    let (text, bytes) = measure(&database());

    assert_eq!(bytes, 267);
    assert_eq!(text.len(), 370);
}
