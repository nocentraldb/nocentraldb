//! Key material is wiped from memory when it is dropped (RFC 0003, "Keys
//! never reach logs, errors or telemetry"). Reading freed memory is
//! undefined behaviour, so this checks the type-level promise: every type
//! that holds key bytes implements `ZeroizeOnDrop`. Its red step is a
//! compile error, the only way a missing trait shows.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::keys::{MemberKey, SpaceKey};
use zeroize::ZeroizeOnDrop;

fn wiped_on_drop<T: ZeroizeOnDrop>() {}

#[test]
fn secret_keys_are_wiped_on_drop() {
    wiped_on_drop::<MemberKey>();
    wiped_on_drop::<SpaceKey>();
}
