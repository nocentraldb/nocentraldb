//! The writer's model in payload key 64 (RFC 0002, "The writer's model"),
//! against RFC 0002's byte-exact examples M1 and M2, which
//! `docs/rfc/vectors/0002/generate.py` writes with Python's cbor2,
//! independently of this crate: the model of RFC 0005's example lock,
//! `model.id` 6f1c0e2a9b7d4c3e8a5f0b1d2c3e4f5a and `model.version` 2.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::{Payload, WriterModel};

const M1: &str = include_str!("../../../docs/rfc/vectors/0002/M1.hex");
const M2: &str = include_str!("../../../docs/rfc/vectors/0002/M2.hex");

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn example() -> WriterModel {
    WriterModel {
        id: [
            0x6f, 0x1c, 0x0e, 0x2a, 0x9b, 0x7d, 0x4c, 0x3e, 0x8a, 0x5f, 0x0b, 0x1d, 0x2c, 0x3e,
            0x4f, 0x5a,
        ],
        version: 2,
    }
}

#[test]
fn m1_the_value_of_key_64_encodes_byte_exact() {
    assert_eq!(example().encode(), hex(M1));
}

#[test]
fn m2_a_payload_that_carries_it_decodes_to_the_writers_model() {
    let payload = Payload::decode(&hex(M2)).unwrap();

    assert_eq!(payload.writer_model(), Some(Ok(example())));
    assert_eq!(
        payload.encode(),
        hex(M2),
        "the payload re-encodes as it was"
    );
}

#[test]
fn m2_a_payload_given_the_writers_model_encodes_byte_exact() {
    let decoded = Payload::decode(&hex(M2)).unwrap();
    let mut payload = Payload::new(
        decoded.clock,
        decoded.dependencies.clone(),
        decoded.operations.clone(),
    );
    assert_eq!(payload.writer_model(), None);

    payload.set_writer_model(example());

    assert_eq!(payload.encode(), hex(M2));
    assert_eq!(
        payload.encode().len()
            - Payload::new(
                decoded.clock,
                decoded.dependencies.clone(),
                decoded.operations.clone()
            )
            .encode()
            .len(),
        23,
        "key 64 adds 23 bytes"
    );
}
