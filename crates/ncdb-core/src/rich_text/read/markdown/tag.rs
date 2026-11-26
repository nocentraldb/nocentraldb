//! The HTML tag of a node or mark outside the vocabulary (RFC 0013,
//! "Reading"): its name and its attributes, in the order of their names.
//! A text attribute reads as it is, any other value as its JSON; `&` and
//! `"` are written as character references, and in a table cell `|` too.

use std::collections::BTreeMap;

use ncdb_plugin::Data;

/// The opening tag of `name` with `attrs`.
pub(super) fn open(name: &str, attrs: &BTreeMap<String, Data>, table: bool) -> String {
    let mut out = format!("<{name}");
    for (key, value) in attrs {
        let value = match value {
            Data::Text(text) => text.clone(),
            other => json(other),
        };
        out.push_str(&format!(" {key}=\"{}\"", attribute(&value, table)));
    }
    out.push('>');
    out
}

/// The closing tag of `name`.
pub(super) fn close(name: &str) -> String {
    format!("</{name}>")
}

fn attribute(value: &str, table: bool) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '|' if table => out.push_str("&#124;"),
            c => out.push(c),
        }
    }
    out
}

/// `value` as JSON, map keys in order. Bytes read as a text of lowercase
/// hexadecimal digits, and a float that is no number as `null`.
fn json(value: &Data) -> String {
    match value {
        Data::Null => "null".to_owned(),
        Data::Bool(b) => b.to_string(),
        Data::Integer(n) => n.to_string(),
        Data::Float(f) if f.is_finite() => f.to_string(),
        Data::Float(_) => "null".to_owned(),
        Data::Text(text) => string(text),
        Data::Bytes(bytes) => string(&bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()),
        Data::Array(items) => format!(
            "[{}]",
            items.iter().map(json).collect::<Vec<_>>().join(",")
        ),
        Data::Map(map) => format!(
            "{{{}}}",
            map.iter()
                .map(|(k, v)| format!("{}:{}", string(k), json(v)))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

fn string(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_default()
}
