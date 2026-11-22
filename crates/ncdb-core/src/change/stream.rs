//! The payload map, encoded one operation at a time (NOC-422). A tree of
//! values takes many times the bytes of its encoding, so the payload is
//! never turned into one tree as a whole: the map's entries are written in
//! key order, and each operation is turned into a value, encoded and
//! dropped before the next.

use std::convert::Infallible;

use std::collections::BTreeMap;

use minicbor::encode::Write;
use minicbor::{Decoder, Encoder};

use super::{Operation, PAYLOAD_DEPTH, Payload};
use crate::cbor::{self, Value};

const INFALLIBLE: &str = "writing to a Vec or a count cannot fail";

/// One entry of the payload map, before it is written.
enum Entry<'a> {
    Value(Value),
    Borrowed(&'a Value),
    Operations(&'a [Operation]),
}

/// A writer that only counts the bytes written to it.
struct Count(usize);

impl Write for Count {
    type Error = Infallible;

    fn write_all(&mut self, buf: &[u8]) -> Result<(), Infallible> {
        self.0 += buf.len();
        Ok(())
    }
}

/// The length of the payload map's canonical bytes without padding,
/// counted without holding them.
pub(super) fn length(payload: &Payload) -> usize {
    let mut count = Count(0);
    write(payload, None, &mut count);
    count.0
}

/// Appends the canonical bytes of the payload map, with `padding` as key 5
/// in place of the payload's own, to `out`.
pub(super) fn encode_into(payload: &Payload, padding: Option<&[u8]>, out: &mut Vec<u8>) {
    write(payload, padding, out);
}

fn write<W: Write<Error = Infallible>>(payload: &Payload, padding: Option<&[u8]>, out: W) {
    let mut entries = vec![
        (1, Entry::Value(Value::Unsigned(payload.format))),
        (2, Entry::Value(Value::Unsigned(payload.clock.0))),
        (
            3,
            Entry::Value(Value::Array(
                payload
                    .dependencies
                    .iter()
                    .map(|id| Value::Bytes(id.0.to_vec()))
                    .collect(),
            )),
        ),
        (4, Entry::Operations(&payload.operations)),
    ];
    if let Some(padding) = padding {
        entries.push((5, Entry::Value(Value::Bytes(padding.to_vec()))));
    }
    entries.extend(
        payload
            .extensions
            .iter()
            .map(|(key, value)| (key, Entry::Borrowed(value))),
    );
    // Known and unknown keys never repeat; an unsigned key's canonical
    // bytes order like the number.
    entries.sort_by_key(|(key, _)| *key);
    let mut encoder = Encoder::new(out);
    encoder.map(entries.len() as u64).expect(INFALLIBLE);
    for (key, entry) in entries {
        encoder.u64(key).expect(INFALLIBLE);
        match entry {
            Entry::Value(value) => put(&mut encoder, &value),
            Entry::Borrowed(value) => put(&mut encoder, value),
            Entry::Operations(operations) => {
                encoder.array(operations.len() as u64).expect(INFALLIBLE);
                for operation in operations {
                    operation_into(&mut encoder, operation);
                }
            }
        }
    }
}

fn put<W: Write<Error = Infallible>>(encoder: &mut Encoder<W>, value: &Value) {
    let _ = encoder.writer_mut().write_all(&cbor::encode(value));
}

/// Writes one operation map, its value as the bytes it keeps.
fn operation_into<W: Write<Error = Infallible>>(encoder: &mut Encoder<W>, operation: &Operation) {
    let mut entries = vec![
        (1, Entry::Value(Value::Bytes(operation.object.0.to_vec()))),
        (2, Entry::Value(Value::Text(operation.collection.clone()))),
        (4, Entry::Value(Value::Unsigned(operation.op))),
    ];
    if let Some(field) = &operation.field {
        entries.push((3, Entry::Value(Value::Text(field.clone()))));
    }
    entries.extend(
        operation
            .extensions
            .iter()
            .map(|(key, value)| (key, Entry::Borrowed(value))),
    );
    entries.sort_by_key(|(key, _)| *key);
    let value = operation.value.as_deref();
    let count = entries.len() + usize::from(value.is_some());
    encoder.map(count as u64).expect(INFALLIBLE);
    let mut value_written = value.is_none();
    for (key, entry) in entries {
        if key > 5 && !value_written {
            value_into(encoder, value);
            value_written = true;
        }
        encoder.u64(key).expect(INFALLIBLE);
        match entry {
            Entry::Value(value) => put(encoder, &value),
            Entry::Borrowed(value) => put(encoder, value),
            Entry::Operations(_) => unreachable!("an operation holds no operations"),
        }
    }
    if !value_written {
        value_into(encoder, value);
    }
}

/// Writes key 5 and the value's kept bytes.
fn value_into<W: Write<Error = Infallible>>(encoder: &mut Encoder<W>, value: Option<&[u8]>) {
    if let Some(bytes) = value {
        encoder.u64(5).expect(INFALLIBLE);
        let _ = encoder.writer_mut().write_all(bytes);
    }
}

/// Decodes a payload map one entry, and one operation, at a time; `None`
/// for bytes it does not take as they are, which [`Payload::decode`] then
/// decodes as one tree to say why. Each entry, and each operation, is
/// checked by the same canonical decoding as the whole map would be; the
/// map's and the array's own heads, and the order of the keys, here.
pub(super) fn decode(bytes: &[u8]) -> Option<Payload> {
    let mut decoder = Decoder::new(bytes);
    let entries = decoder.map().ok()??;
    canonical_head(bytes, 0, decoder.position(), |e| e.map(entries))?;
    let mut fields = BTreeMap::new();
    let mut operations = None;
    let mut previous = None;
    for _ in 0..entries {
        let at = decoder.position();
        let key = decoder.u64().ok()?;
        canonical_head(bytes, at, decoder.position(), |e| e.u64(key))?;
        if previous.is_some_and(|previous| previous >= key) {
            return None;
        }
        previous = Some(key);
        if key == 4 {
            operations = Some(operations_of(bytes, &mut decoder)?);
            continue;
        }
        let start = decoder.position();
        decoder.skip().ok()?;
        let value = cbor::decode_at(&bytes[start..decoder.position()], PAYLOAD_DEPTH + 1).ok()?;
        fields.insert(key, value);
    }
    if decoder.position() != bytes.len() {
        return None;
    }
    Payload::of(fields, operations?).ok()
}

/// The operations of key 4's array, each decoded on its own.
fn operations_of(bytes: &[u8], decoder: &mut Decoder<'_>) -> Option<Vec<Operation>> {
    let at = decoder.position();
    let count = decoder.array().ok()??;
    canonical_head(bytes, at, decoder.position(), |e| e.array(count))?;
    // Each operation takes at least a byte: no more than are left.
    let left = bytes.len() - decoder.position();
    let mut operations = Vec::with_capacity(usize::try_from(count).ok()?.min(left));
    for _ in 0..count {
        let start = decoder.position();
        decoder.skip().ok()?;
        let item = &bytes[start..decoder.position()];
        let value = cbor::decode_at(item, PAYLOAD_DEPTH + 2).ok()?;
        operations.push(Operation::from_value(value).ok()?);
    }
    Some(operations)
}

/// `Some` when `bytes[start..end]` is the canonical head `write` writes.
fn canonical_head(
    bytes: &[u8],
    start: usize,
    end: usize,
    write: impl FnOnce(
        &mut Encoder<Vec<u8>>,
    ) -> Result<&mut Encoder<Vec<u8>>, minicbor::encode::Error<Infallible>>,
) -> Option<()> {
    let mut encoder = Encoder::new(Vec::new());
    write(&mut encoder).expect(INFALLIBLE);
    (encoder.writer().as_slice() == &bytes[start..end]).then_some(())
}
