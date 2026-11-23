//! The frame and the header map, written and read without a tree of
//! values (NOC-422). A tree would copy the payload's ciphertext into a
//! byte string value, and checking it canonical would copy it again: for a
//! change of megabytes, several copies at once. Here the header's payload
//! is written from, and read into, its one buffer, and the frame's header
//! bytes and signature are ranges of the frame.

use std::collections::BTreeMap;
use std::ops::Range;

use minicbor::Decoder;

use super::{HEADER_DEPTH, Header};
use crate::cbor::{self, Value};

/// CBOR's major types this module writes heads of.
const UNSIGNED: u8 = 0;
const BYTES: u8 = 2;
const ARRAY: u8 = 4;
const MAP: u8 = 5;

/// The canonical head of major type `major` carrying `n`: the shortest.
fn head(major: u8, n: u64) -> Vec<u8> {
    let major = major << 5;
    match n {
        0..24 => vec![major | n as u8],
        24..0x100 => vec![major | 24, n as u8],
        0x100..0x1_0000 => [&[major | 25][..], &(n as u16).to_be_bytes()].concat(),
        0x1_0000..0x1_0000_0000 => [&[major | 26][..], &(n as u32).to_be_bytes()].concat(),
        _ => [&[major | 27][..], &n.to_be_bytes()].concat(),
    }
}

/// The frame `[version, header bytes, signature]`, written once at its
/// size, and the ranges of the header bytes and the signature in it.
pub(super) fn encode(
    version: u64,
    header: &[u8],
    signature: &[u8],
) -> (Vec<u8>, Range<usize>, Range<usize>) {
    let header_head = head(BYTES, header.len() as u64);
    let signature_head = head(BYTES, signature.len() as u64);
    let mut frame = head(ARRAY, 3);
    frame.extend_from_slice(&head(UNSIGNED, version));
    frame.reserve_exact(header_head.len() + header.len() + signature_head.len() + signature.len());
    frame.extend_from_slice(&header_head);
    let header_at = frame.len();
    frame.extend_from_slice(header);
    let header_range = header_at..frame.len();
    frame.extend_from_slice(&signature_head);
    let signature_at = frame.len();
    frame.extend_from_slice(signature);
    let signature_range = signature_at..frame.len();
    (frame, header_range, signature_range)
}

/// The frame's version and the ranges of its header bytes and signature,
/// or `None` for bytes that are no canonical array of an unsigned integer
/// and two byte strings; [`super::Change::decode`] then says why.
pub(super) fn split(bytes: &[u8]) -> Option<(u64, Range<usize>, Range<usize>)> {
    let mut decoder = Decoder::new(bytes);
    let count = decoder.array().ok()??;
    expect(bytes, 0, &decoder, ARRAY, count)?;
    (count == 3).then_some(())?;
    let at = decoder.position();
    let version = decoder.u64().ok()?;
    expect(bytes, at, &decoder, UNSIGNED, version)?;
    let header = byte_string(bytes, &mut decoder)?;
    let signature = byte_string(bytes, &mut decoder)?;
    (decoder.position() == bytes.len()).then_some((version, header, signature))
}

/// The range of the canonical byte string at the decoder's position.
fn byte_string(bytes: &[u8], decoder: &mut Decoder<'_>) -> Option<Range<usize>> {
    let at = decoder.position();
    let length = decoder.bytes().ok()?.len();
    let end = decoder.position();
    let start = end - length;
    (bytes[at..start] == head(BYTES, length as u64)[..]).then_some(start..end)
}

/// `Some` when the bytes from `at` to the decoder's position are the
/// canonical head of `major` carrying `n`.
fn expect(bytes: &[u8], at: usize, decoder: &Decoder<'_>, major: u8, n: u64) -> Option<()> {
    (bytes[at..decoder.position()] == head(major, n)[..]).then_some(())
}

/// The canonical bytes of the header map, its payload written from its
/// buffer; without the payload (key 6) for the associated data.
pub(super) fn encode_header(header: &Header, with_payload: bool) -> Vec<u8> {
    let mut entries: Vec<(u64, Option<Value>)> = header
        .known_without_payload()
        .into_iter()
        .map(|(key, value)| (key, Some(value)))
        .collect();
    if with_payload {
        entries.push((6, None));
    }
    entries.extend(
        header
            .extensions
            .iter()
            .map(|(key, value)| (key, Some(value.clone()))),
    );
    entries.sort_by_key(|(key, _)| *key);
    let payload = if with_payload {
        header.payload.len() + 9
    } else {
        0
    };
    let mut out = Vec::with_capacity(payload + 128);
    out.extend_from_slice(&head(MAP, entries.len() as u64));
    for (key, value) in entries {
        out.extend_from_slice(&head(UNSIGNED, key));
        match value {
            Some(value) => out.extend_from_slice(&cbor::encode(&value)),
            None => {
                out.extend_from_slice(&head(BYTES, header.payload.len() as u64));
                out.extend_from_slice(&header.payload);
            }
        }
    }
    out
}

/// The header map's entries, its payload (key 6) read into one buffer and
/// every other entry by the canonical decoding; `None` for bytes it does
/// not take as they are, which [`Header::decode`] then decodes as one tree
/// to say why.
pub(super) fn header_entries(bytes: &[u8]) -> Option<BTreeMap<u64, Value>> {
    let mut decoder = Decoder::new(bytes);
    let count = decoder.map().ok()??;
    expect(bytes, 0, &decoder, MAP, count)?;
    let mut entries = BTreeMap::new();
    let mut previous = None;
    for _ in 0..count {
        let at = decoder.position();
        let key = decoder.u64().ok()?;
        expect(bytes, at, &decoder, UNSIGNED, key)?;
        if previous.is_some_and(|previous| previous >= key) {
            return None;
        }
        previous = Some(key);
        let value = if key == 6 {
            let range = byte_string(bytes, &mut decoder)?;
            Value::Bytes(bytes[range].to_vec())
        } else {
            let start = decoder.position();
            decoder.skip().ok()?;
            cbor::decode_at(&bytes[start..decoder.position()], HEADER_DEPTH + 1).ok()?
        };
        entries.insert(key, value);
    }
    (decoder.position() == bytes.len()).then_some(entries)
}
