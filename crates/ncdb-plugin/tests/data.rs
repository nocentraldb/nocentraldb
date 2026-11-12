//! `Data`, what a plugin reads and writes (RFC 0011, "The data a plugin
//! reads and writes"): RFC 0002's CBOR subset of null, booleans, integers,
//! floats, text, bytes, arrays and maps with text keys, in RFC 0002's
//! canonical form, which a plugin's state and the app's values share.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeMap;

use ncdb_plugin::Data;

fn map(entries: &[(&str, Data)]) -> Data {
    Data::Map(
        entries
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect::<BTreeMap<_, _>>(),
    )
}

/// Each kind of value and its canonical bytes (RFC 8949, section 4.2.1,
/// as RFC 0002's profile takes it).
fn cases() -> Vec<(Data, Vec<u8>)> {
    vec![
        (Data::Null, vec![0xf6]),
        (Data::Bool(false), vec![0xf4]),
        (Data::Bool(true), vec![0xf5]),
        (Data::Integer(0), vec![0x00]),
        (Data::Integer(23), vec![0x17]),
        (Data::Integer(24), vec![0x18, 0x18]),
        (Data::Integer(-1), vec![0x20]),
        (Data::Integer(-500), vec![0x39, 0x01, 0xf3]),
        (
            Data::Integer(i128::from(u64::MAX)),
            vec![0x1b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        ),
        (
            Data::Integer(-1 - i128::from(u64::MAX)),
            vec![0x3b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        ),
        (Data::Float(1.5), vec![0xf9, 0x3e, 0x00]),
        (Data::Float(0.1), {
            let mut bytes = vec![0xfb];
            bytes.extend_from_slice(&0.1f64.to_be_bytes());
            bytes
        }),
        (Data::Text("a".into()), vec![0x61, 0x61]),
        (Data::Bytes(vec![1, 2]), vec![0x42, 0x01, 0x02]),
        (
            Data::Array(vec![Data::Integer(1), Data::Null]),
            vec![0x82, 0x01, 0xf6],
        ),
        // Keys sort by their encoded bytes: "b" and "a" as "a", "b"; a
        // shorter key first.
        (
            map(&[
                ("b", Data::Integer(1)),
                ("a", Data::Integer(2)),
                ("aa", Data::Null),
            ]),
            vec![0xa3, 0x61, 0x61, 0x02, 0x61, 0x62, 0x01, 0x62, 0x61, 0x61, 0xf6],
        ),
    ]
}

#[test]
fn every_kind_of_value_encodes_to_its_canonical_bytes_and_back() {
    for (data, bytes) in cases() {
        assert_eq!(data.encode().unwrap(), bytes, "{bytes:02x?}");
        assert_eq!(Data::decode(&bytes).unwrap(), data, "{bytes:02x?}");
    }
}

#[test]
fn an_integer_outside_cbors_range_does_not_encode() {
    let above = Data::Integer(i128::from(u64::MAX) + 1);
    let below = Data::Integer(-2 - i128::from(u64::MAX));
    assert!(above.encode().is_err());
    assert!(below.encode().is_err());
    assert!(Data::Array(vec![above]).encode().is_err());
}

#[test]
fn bytes_that_are_not_canonical_or_not_data_are_refused() {
    for bytes in [
        // 1 in two bytes.
        &[0x18, 0x01][..],
        // A map with an integer key.
        &[0xa1, 0x01, 0x01],
        // Keys out of order.
        &[0xa2, 0x61, 0x62, 0x01, 0x61, 0x61, 0x02],
        // A float that has a shorter form.
        &[0xfb, 0x3f, 0xf8, 0, 0, 0, 0, 0, 0],
        // A tag.
        &[0xc1, 0x01],
        // Trailing bytes.
        &[0x01, 0x01],
        // Nothing.
        &[],
    ] {
        assert!(Data::decode(bytes).is_err(), "{bytes:02x?}");
    }
}

#[test]
fn data_never_shows_its_content_in_debug_output() {
    let secret = map(&[("name", Data::Text("Ada Lovelace".into()))]);
    let shown = format!("{secret:?}");
    assert!(!shown.contains("Ada"), "{shown}");
    assert!(!shown.contains("name"), "{shown}");
}
