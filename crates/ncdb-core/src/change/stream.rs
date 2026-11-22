//! The payload map, encoded one operation at a time (NOC-422). A tree of
//! values takes many times the bytes of its encoding, so the payload is
//! never turned into one tree as a whole: the map's entries are written in
//! key order, and each operation is turned into a value, encoded and
//! dropped before the next.

use std::convert::Infallible;

use minicbor::Encoder;
use minicbor::encode::Write;

use super::{Operation, Payload};
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
                    put(&mut encoder, &operation.to_value());
                }
            }
        }
    }
}

fn put<W: Write<Error = Infallible>>(encoder: &mut Encoder<W>, value: &Value) {
    let _ = encoder.writer_mut().write_all(&cbor::encode(value));
}
