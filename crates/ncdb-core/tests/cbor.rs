//! The canonical CBOR encoding of RFC 0002: one byte representation per
//! value, and a decoder that rejects every other.

#[path = "support/props.rs"]
mod props;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeMap;

use ncdb_core::cbor::{self, DecodeError, Key, Reason, Value};
use proptest::prelude::*;

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn map(entries: Vec<(Key, Value)>) -> Value {
    Value::Map(entries.into_iter().collect::<BTreeMap<_, _>>())
}

fn text(s: &str) -> Value {
    Value::Text(s.to_owned())
}

fn reason(bytes: &str) -> Reason {
    match cbor::decode(&hex(bytes)) {
        Ok(_) => panic!("{bytes} decoded, but it breaks the profile"),
        Err(error) => error.reason,
    }
}

#[test]
fn generated_values_round_trip() {
    props::check(1024, props::value(), |value| {
        let bytes = cbor::encode(&value);
        prop_assert_eq!(cbor::decode(&bytes), Ok(value.clone()));
        // The same value always yields the same bytes.
        prop_assert_eq!(cbor::encode(&value), bytes);
        Ok(())
    });
}

/// The examples of RFC 8949, Appendix A, that the profile allows.
#[test]
fn encodes_the_examples_of_rfc_8949() {
    let examples: Vec<(Value, &str)> = vec![
        (Value::Unsigned(0), "00"),
        (Value::Unsigned(23), "17"),
        (Value::Unsigned(24), "1818"),
        (Value::Unsigned(1000), "1903e8"),
        (Value::Unsigned(1_000_000), "1a000f4240"),
        (Value::Unsigned(u64::MAX), "1bffffffffffffffff"),
        (Value::Negative(0), "20"),
        (Value::Negative(999), "3903e7"),
        (Value::Negative(u64::MAX), "3bffffffffffffffff"),
        (Value::Float(0.0), "f90000"),
        (Value::Float(-0.0), "f98000"),
        (Value::Float(1.0), "f93c00"),
        (Value::Float(1.1), "fb3ff199999999999a"),
        (Value::Float(1.5), "f93e00"),
        (Value::Float(65504.0), "f97bff"),
        (Value::Float(100000.0), "fa47c35000"),
        (Value::Float(3.4028234663852886e38), "fa7f7fffff"),
        (Value::Float(1.0e300), "fb7e37e43c8800759c"),
        (Value::Float(5.960464477539063e-8), "f90001"),
        (Value::Float(0.00006103515625), "f90400"),
        (Value::Float(-4.0), "f9c400"),
        (Value::Float(-4.1), "fbc010666666666666"),
        (Value::Float(f64::INFINITY), "f97c00"),
        (Value::Float(f64::NEG_INFINITY), "f9fc00"),
        (Value::Float(f64::NAN), "f97e00"),
        (Value::Bool(false), "f4"),
        (Value::Bool(true), "f5"),
        (Value::Null, "f6"),
        (Value::Bytes(vec![]), "40"),
        (Value::Bytes(vec![1, 2, 3, 4]), "4401020304"),
        (text(""), "60"),
        (text("a"), "6161"),
        (text("IETF"), "6449455446"),
        (text("\u{00fc}"), "62c3bc"),
        (text("\u{6c34}"), "63e6b0b4"),
        (Value::Array(vec![]), "80"),
        (
            Value::Array(vec![
                Value::Unsigned(1),
                Value::Array(vec![Value::Unsigned(2), Value::Unsigned(3)]),
            ]),
            "8201820203",
        ),
        (
            Value::Array((1..=25).map(Value::Unsigned).collect()),
            "98190102030405060708090a0b0c0d0e0f101112131415161718181819",
        ),
        (map(vec![]), "a0"),
        (
            map(vec![
                (Key::Unsigned(1), Value::Unsigned(2)),
                (Key::Unsigned(3), Value::Unsigned(4)),
            ]),
            "a201020304",
        ),
        (
            map(vec![
                (Key::Text("a".into()), Value::Unsigned(1)),
                (
                    Key::Text("b".into()),
                    Value::Array(vec![Value::Unsigned(2), Value::Unsigned(3)]),
                ),
            ]),
            "a26161016162820203",
        ),
    ];
    for (value, expected) in examples {
        assert_eq!(
            cbor::encode(&value),
            hex(expected),
            "encoding of {expected}"
        );
        assert_eq!(
            cbor::decode(&hex(expected)),
            Ok(value),
            "decoding of {expected}"
        );
    }
}

#[test]
fn map_keys_sort_by_the_bytes_of_their_encoding() {
    let value = map(vec![
        (Key::Text("aa".into()), Value::Null),
        (Key::Text("z".into()), Value::Null),
        (Key::Unsigned(100), Value::Null),
        (Key::Unsigned(10), Value::Null),
        (Key::Unsigned(24), Value::Null),
        (Key::Unsigned(23), Value::Null),
    ]);
    // 10, 23, 24, 100, "z", "aa": shorter encodings first, then bytewise.
    assert_eq!(
        cbor::encode(&value),
        hex("a6 0a f6 17 f6 1818 f6 1864 f6 617a f6 626161 f6")
    );
    props::check(512, (props::key(), props::key()), |(a, b)| {
        let encoded = |key: &Key| match key {
            Key::Unsigned(n) => cbor::encode(&Value::Unsigned(*n)),
            Key::Text(s) => cbor::encode(&Value::Text(s.clone())),
        };
        prop_assert_eq!(a.cmp(&b), encoded(&a).cmp(&encoded(&b)));
        Ok(())
    });
}

#[test]
fn every_half_precision_float_but_nan_is_canonical() {
    for bits in 0..=u16::MAX {
        let mut bytes = vec![0xf9];
        bytes.extend_from_slice(&bits.to_be_bytes());
        let nan = bits & 0x7c00 == 0x7c00 && bits & 0x03ff != 0;
        match cbor::decode(&bytes) {
            Ok(value) => {
                assert!(
                    !nan || bits == 0x7e00,
                    "{bits:04x} is a NaN other than 7e00"
                );
                assert_eq!(
                    cbor::encode(&value),
                    bytes,
                    "{bits:04x} encodes differently"
                );
            }
            Err(error) => {
                assert!(nan && bits != 0x7e00, "{bits:04x} was rejected: {error}");
                assert_eq!(error.reason, Reason::NotCanonical);
            }
        }
    }
}

#[test]
fn every_nan_is_one_value_and_negative_zero_is_not_zero() {
    let other_nan = f64::from_bits(0x7ff8_0000_0000_0001);
    assert_eq!(Value::Float(f64::NAN), Value::Float(other_nan));
    assert_eq!(cbor::encode(&Value::Float(other_nan)), hex("f97e00"));
    assert_eq!(cbor::encode(&Value::Float(-f64::NAN)), hex("f97e00"));
    assert_ne!(Value::Float(0.0), Value::Float(-0.0));
}

#[test]
fn rejects_indefinite_lengths() {
    for bytes in ["5f 4101 ff", "7f 6161 ff", "9f 01 ff", "bf 01 02 ff"] {
        assert_eq!(reason(bytes), Reason::IndefiniteLength, "{bytes}");
    }
}

#[test]
fn rejects_unsorted_and_duplicate_keys() {
    assert_eq!(reason("a2 02 00 01 00"), Reason::UnsortedKeys);
    assert_eq!(reason("a2 6162 00 6161 00"), Reason::UnsortedKeys);
    // Length first: "b" sorts before "aa".
    assert_eq!(reason("a2 626161 00 6162 00"), Reason::UnsortedKeys);
    assert_eq!(reason("a2 6161 00 01 00"), Reason::UnsortedKeys);
    assert_eq!(reason("a2 01 00 01 00"), Reason::DuplicateKey);
    assert_eq!(reason("a2 6161 00 6161 01"), Reason::DuplicateKey);
}

#[test]
fn rejects_keys_that_are_neither_unsigned_nor_text() {
    for bytes in [
        "a1 20 00",
        "a1 4100 00",
        "a1 80 00",
        "a1 f4 00",
        "a1 f93c00 00",
    ] {
        assert_eq!(reason(bytes), Reason::KeyType, "{bytes}");
    }
}

#[test]
fn rejects_tags() {
    for bytes in [
        "c0 6161",
        "c1 1a514b67b0",
        "c2 4101",
        "d8 20 6161",
        "d9 d9f7 00",
    ] {
        assert_eq!(reason(bytes), Reason::Tag, "{bytes}");
    }
}

#[test]
fn rejects_every_nan_but_one() {
    for bytes in [
        "f9 7e01",
        "f9 7c01",
        "f9 fe00",
        "fa 7fc00000",
        "fa 7f800001",
        "fb 7ff8000000000000",
        "fb 7ff0000000000001",
    ] {
        assert_eq!(reason(bytes), Reason::NotCanonical, "{bytes}");
    }
}

#[test]
fn rejects_floats_that_have_a_shorter_form() {
    for bytes in [
        "fa 3f800000",
        "fb 3ff0000000000000",
        "fa 00000000",
        "fb 8000000000000000",
        "fb 7ff0000000000000",
        "fb 40f86a0000000000",
    ] {
        assert_eq!(reason(bytes), Reason::NotCanonical, "{bytes}");
    }
}

#[test]
fn rejects_integers_and_lengths_that_have_a_shorter_form() {
    for bytes in [
        "18 17",
        "19 00ff",
        "1a 0000ffff",
        "1b 00000000ffffffff",
        "38 00",
        "58 01 00",
        "78 01 61",
        "98 01 00",
        "b8 01 00 00",
    ] {
        assert_eq!(reason(bytes), Reason::NotCanonical, "{bytes}");
    }
}

#[test]
fn rejects_other_simple_values_and_reserved_bytes() {
    for bytes in ["f7", "e0", "f0", "f8 20", "f8 ff", "1c", "3d", "5e", "ff"] {
        assert_eq!(reason(bytes), Reason::DisallowedSimple, "{bytes}");
    }
}

#[test]
fn rejects_invalid_utf8() {
    assert_eq!(reason("62 fffe"), Reason::InvalidUtf8);
    assert_eq!(reason("63 eda080"), Reason::InvalidUtf8);
}

#[test]
fn rejects_truncated_and_trailing_bytes() {
    for bytes in ["", "19 01", "62 61", "82 01", "a1 01", "fb 3ff0"] {
        assert_eq!(reason(bytes), Reason::Truncated, "{bytes:?}");
    }
    let error = cbor::decode(&hex("01 02")).unwrap_err();
    assert_eq!(
        error,
        DecodeError {
            offset: 1,
            reason: Reason::TrailingBytes
        }
    );
}

#[test]
fn a_huge_declared_length_fails_without_allocating_it() {
    assert_eq!(reason("9b ffffffffffffffff"), Reason::Truncated);
    assert_eq!(reason("bb ffffffffffffffff"), Reason::Truncated);
    assert_eq!(reason("5b ffffffffffffffff"), Reason::Truncated);
}

#[test]
fn the_error_names_offset_and_reason() {
    let error = cbor::decode(&hex("82 01 18 05")).unwrap_err();
    assert_eq!(error.offset, 2);
    assert_eq!(error.reason, Reason::NotCanonical);
    assert_eq!(
        error.to_string(),
        "not canonical CBOR at byte 2: a shorter or other canonical form exists"
    );
}

/// The value of a half-precision pattern, built from its bits through
/// single precision, with no floating-point arithmetic.
fn half_reference(half: u16) -> f32 {
    let sign = u32::from(half >> 15) << 31;
    let exponent = u32::from((half >> 10) & 0x1f);
    let mantissa = u32::from(half & 0x3ff);
    let bits = match (exponent, mantissa) {
        (0, 0) => sign,
        (0, _) => {
            // m * 2^-24, normalised: the highest set bit k gives 2^(k-24).
            let k = 31 - mantissa.leading_zeros();
            sign | (k + 127 - 24) << 23 | (mantissa << (23 - k)) & 0x7f_ffff
        }
        (0x1f, 0) => sign | 0x7f80_0000,
        (0x1f, _) => unreachable!("NaN"),
        _ => sign | (exponent + 127 - 15) << 23 | mantissa << 13,
    };
    f32::from_bits(bits)
}

#[test]
fn every_half_precision_float_has_its_exact_value() {
    for half in 0..=u16::MAX {
        if half & 0x7c00 == 0x7c00 && half & 0x03ff != 0 {
            continue;
        }
        let expected = f64::from(half_reference(half));
        let mut bytes = vec![0xf9];
        bytes.extend_from_slice(&half.to_be_bytes());
        match cbor::decode(&bytes) {
            Ok(Value::Float(x)) => assert_eq!(x.to_bits(), expected.to_bits(), "{half:04x}"),
            other => panic!("{half:04x} decoded as {other:?}"),
        }
        assert_eq!(cbor::encode(&Value::Float(expected)), bytes, "{half:04x}");
    }
}
