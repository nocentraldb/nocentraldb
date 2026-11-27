//! An update Yrs encoded, written again with every map in key order.
//!
//! Yrs keeps a value's maps (a mark's attributes, a node attribute that
//! holds an object, a subdocument's options) in hash maps and encodes them
//! in the order the hash map gives, which differs from map to map. So two
//! copies that hold the same document can encode it differently. This
//! writes each such map with its entries sorted by key, byte by byte, and
//! leaves every other byte as it is: in lib0's encoding of values the
//! entries are only reordered, and in the JSON a mark's value is written
//! as, objects are written with sorted keys and every other value as Yrs
//! writes it.

use yrs::Any;

use super::{ARRAY, Cursor, Malformed, OBJECT, Value, read_all};

/// `bytes`, an update in encoding v1, with every map in key order. The
/// result encodes the same document.
pub(in crate::rich_text) fn canonical(bytes: &[u8]) -> Result<Vec<u8>, Malformed> {
    let (_, _, values) = read_all(bytes)?;
    let mut out = Vec::with_capacity(bytes.len());
    let mut copied = 0;
    for value in values {
        let (start, end) = match value {
            Value::Any(start, end) | Value::Json(start, end) => (start, end),
        };
        out.extend_from_slice(&bytes[copied..start]);
        let mut cursor = Cursor::new(&bytes[..end]);
        cursor.at = start;
        match value {
            Value::Any(..) => any(&mut cursor, &mut out)?,
            Value::Json(..) => json(cursor.string()?, &mut out),
        }
        copied = end;
    }
    out.extend_from_slice(&bytes[copied..]);
    Ok(out)
}

/// The value at `cursor`, in lib0's encoding, with its maps in key order.
fn any(cursor: &mut Cursor<'_>, out: &mut Vec<u8>) -> Result<(), Malformed> {
    let start = cursor.at;
    match cursor.byte()? {
        OBJECT => {
            let n = cursor.count()?;
            out.extend_from_slice(&cursor.bytes[start..cursor.at]);
            let mut entries = Vec::new();
            for _ in 0..n {
                let key_start = cursor.at;
                cursor.string()?;
                let mut entry = cursor.bytes[key_start..cursor.at].to_vec();
                let key_len = entry.len();
                any(cursor, &mut entry)?;
                entries.push((key_len, entry));
            }
            // By the key's bytes, after its length; then by the whole entry,
            // for a map that names a key twice.
            entries.sort_by(|(a_len, a), (b_len, b)| {
                key(a, *a_len).cmp(key(b, *b_len)).then_with(|| a.cmp(b))
            });
            for (_, entry) in entries {
                out.extend_from_slice(&entry);
            }
        }
        ARRAY => {
            let n = cursor.count()?;
            out.extend_from_slice(&cursor.bytes[start..cursor.at]);
            for _ in 0..n {
                any(cursor, out)?;
            }
        }
        _ => {
            cursor.at = start;
            cursor.any(0)?;
            out.extend_from_slice(&cursor.bytes[start..cursor.at]);
        }
    }
    Ok(())
}

/// The bytes of an encoded key: its string without the length in front.
fn key(entry: &[u8], len: usize) -> &[u8] {
    let mut cursor = Cursor::new(&entry[..len]);
    cursor.string().map(str::as_bytes).unwrap_or(&entry[..len])
}

/// `text`, JSON Yrs wrote, as a counted string with its objects' keys in
/// order. Text that is no JSON stays as it is.
fn json(text: &str, out: &mut Vec<u8>) {
    let written = match Any::from_json(text) {
        Ok(value) => {
            let mut written = String::with_capacity(text.len());
            write_json(&value, &mut written);
            written
        }
        Err(_) => text.to_owned(),
    };
    uint(out, written.len() as u64);
    out.extend_from_slice(written.as_bytes());
}

fn write_json(value: &Any, out: &mut String) {
    match value {
        Any::Map(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by(|(a, _), (b, _)| a.as_bytes().cmp(b.as_bytes()));
            out.push('{');
            for (n, (key, value)) in entries.into_iter().enumerate() {
                if n > 0 {
                    out.push(',');
                }
                scalar(&Any::from(key.as_str()), out);
                out.push(':');
                write_json(value, out);
            }
            out.push('}');
        }
        Any::Array(items) => {
            out.push('[');
            for (n, item) in items.iter().enumerate() {
                if n > 0 {
                    out.push(',');
                }
                write_json(item, out);
            }
            out.push(']');
        }
        other => scalar(other, out),
    }
}

/// A value that holds no map or array, as Yrs writes it. Yrs writes from
/// the start of the string it is given, so it gets an empty one.
fn scalar(value: &Any, out: &mut String) {
    let mut written = String::new();
    value.to_json(&mut written);
    out.push_str(&written);
}

/// lib0's unsigned varint.
fn uint(out: &mut Vec<u8>, mut n: u64) {
    loop {
        let byte = (n & 0x7f) as u8;
        n >>= 7;
        if n == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}
