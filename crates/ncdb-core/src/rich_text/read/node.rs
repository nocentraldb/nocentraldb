//! The document as nodes, read from Yrs as TipTap's binding writes it
//! (RFC 0013, "How Yjs works", "TipTap's mapping"): an `XmlElement` is a
//! node named after its type, with its attributes; an `XmlText` holds runs
//! of text whose formatting attributes are their marks, each mark's value
//! the map of its attributes. Nothing goes through a ProseMirror schema,
//! which deletes what it does not know (F16), so every node, attribute and
//! mark is kept.
//!
//! Yrs keeps attributes and marks in hash maps, whose order differs from
//! copy to copy; the nodes hold them sorted by name, so every read is a
//! function of the state (C7).

use std::collections::BTreeMap;

use ncdb_plugin::Data;
use yrs::types::ToJson;
use yrs::types::text::YChange;
use yrs::types::xml::{Xml, XmlFragment, XmlOut, XmlTextRef};
use yrs::{Any, Number, Out, ReadTxn, Text, Transact};

use super::super::document;

/// A node of the document.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Node {
    /// A node other than text: its type, its attributes and its content.
    Element {
        name: String,
        attrs: BTreeMap<String, Data>,
        children: Vec<Node>,
    },
    /// A run of text and its marks, sorted by name.
    Text { text: String, marks: Vec<Mark> },
}

/// A mark on a run of text.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Mark {
    pub(crate) name: String,
    pub(crate) attrs: BTreeMap<String, Data>,
}

/// The nodes of the document's root, none for a field nobody wrote or a
/// state Yrs cannot load, which only the core's own bytes are.
pub(crate) fn nodes(state: &[u8]) -> Vec<Node> {
    let Ok((doc, root)) = document::load(state) else {
        return Vec::new();
    };
    let txn = doc.transact();
    children(&txn, &root)
}

fn children<T: ReadTxn>(txn: &T, parent: &impl XmlFragment) -> Vec<Node> {
    let mut out = Vec::new();
    for child in parent.children(txn) {
        match child {
            XmlOut::Element(element) => out.push(Node::Element {
                name: element.tag().to_string(),
                attrs: element
                    .attributes(txn)
                    .filter_map(|(name, value)| Some((name.to_owned(), data(&json(txn, &value))?)))
                    .collect(),
                children: children(txn, &element),
            }),
            // No editor writes a fragment inside the root; its children
            // read in its place.
            XmlOut::Fragment(fragment) => out.extend(children(txn, &fragment)),
            XmlOut::Text(text) => out.extend(runs(txn, &text)),
        }
    }
    out
}

fn json<T: ReadTxn>(txn: &T, value: &Out) -> Any {
    match value {
        Out::Any(any) => any.clone(),
        other => other.to_json(txn),
    }
}

/// The runs of text of `text`. An embedded value is no part of TipTap's
/// mapping and is not read.
fn runs<T: ReadTxn>(txn: &T, text: &XmlTextRef) -> Vec<Node> {
    let mut out: Vec<Node> = Vec::new();
    for chunk in text.diff(txn, YChange::identity) {
        let Out::Any(Any::String(run)) = chunk.insert else {
            continue;
        };
        if run.is_empty() {
            continue;
        }
        let mut marks: Vec<Mark> = chunk
            .attributes
            .iter()
            .flat_map(|attrs| attrs.iter())
            .filter_map(|(name, value)| mark(name, value))
            .collect();
        marks.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| order(&a.attrs, &b.attrs)));
        match out.last_mut() {
            Some(Node::Text { text, marks: last }) if *last == marks => text.push_str(&run),
            _ => out.push(Node::Text {
                text: run.to_string(),
                marks,
            }),
        }
    }
    out
}

/// A total order of attribute maps, by their canonical bytes, for two marks
/// of one name.
fn order(a: &BTreeMap<String, Data>, b: &BTreeMap<String, Data>) -> std::cmp::Ordering {
    let bytes = |m: &BTreeMap<String, Data>| Data::Map(m.clone()).encode().unwrap_or_default();
    bytes(a).cmp(&bytes(b))
}

/// The mark a formatting attribute stands for: its value is the map of the
/// mark's attributes. `null` and `false` set no mark; any other value sets
/// the mark without attributes.
fn mark(name: &str, value: &Any) -> Option<Mark> {
    let attrs = match value {
        Any::Null | Any::Undefined | Any::Bool(false) => return None,
        Any::Map(map) => map
            .iter()
            .filter_map(|(k, v)| Some((k.clone(), data(v)?)))
            .collect(),
        _ => BTreeMap::new(),
    };
    Some(Mark {
        name: mark_name(name).to_owned(),
        attrs,
    })
}

/// The mark's name: y-prosemirror names a mark that may occur more than
/// once on a run with a suffix of `--` and eight characters of a hash.
fn mark_name(attribute: &str) -> &str {
    let bytes = attribute.as_bytes();
    if bytes.len() > 10 {
        let (name, suffix) = attribute.split_at(attribute.len() - 10);
        let hash = &suffix.as_bytes()[2..];
        if suffix.starts_with("--")
            && hash
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
        {
            return name;
        }
    }
    attribute
}

/// A Yjs value as `Data`; `None` for `null` and `undefined`, which leave an
/// attribute unset.
pub(crate) fn data(value: &Any) -> Option<Data> {
    Some(match value {
        Any::Null | Any::Undefined => return None,
        Any::Bool(b) => Data::Bool(*b),
        Any::Number(Number::Int(n)) => Data::Integer(i128::from(*n)),
        Any::Number(Number::Float(f)) => Data::Float(*f),
        Any::String(s) => Data::Text(s.to_string()),
        Any::Buffer(b) => Data::Bytes(b.to_vec()),
        Any::Array(items) => Data::Array(
            items
                .iter()
                .map(|item| data(item).unwrap_or(Data::Null))
                .collect(),
        ),
        Any::Map(map) => Data::Map(
            map.iter()
                .map(|(k, v)| (k.clone(), data(v).unwrap_or(Data::Null)))
                .collect(),
        ),
    })
}

/// The ProseMirror JSON of `nodes` under a `doc` root: `{ type, attrs,
/// content, marks, text }`, `attrs`, `content` and `marks` only when they
/// hold something.
pub(crate) fn tree(nodes: &[Node]) -> Data {
    let mut doc = BTreeMap::new();
    doc.insert("type".to_owned(), Data::Text("doc".to_owned()));
    doc.insert(
        "content".to_owned(),
        Data::Array(nodes.iter().map(node).collect()),
    );
    Data::Map(doc)
}

fn node(node: &Node) -> Data {
    let mut out = BTreeMap::new();
    match node {
        Node::Element {
            name,
            attrs,
            children,
        } => {
            out.insert("type".to_owned(), Data::Text(name.clone()));
            if !attrs.is_empty() {
                out.insert("attrs".to_owned(), Data::Map(attrs.clone()));
            }
            if !children.is_empty() {
                let content = children.iter().map(self::node).collect();
                out.insert("content".to_owned(), Data::Array(content));
            }
        }
        Node::Text { text, marks } => {
            out.insert("type".to_owned(), Data::Text("text".to_owned()));
            out.insert("text".to_owned(), Data::Text(text.clone()));
            if !marks.is_empty() {
                let marks = marks
                    .iter()
                    .map(|mark| {
                        let mut out = BTreeMap::new();
                        out.insert("type".to_owned(), Data::Text(mark.name.clone()));
                        if !mark.attrs.is_empty() {
                            out.insert("attrs".to_owned(), Data::Map(mark.attrs.clone()));
                        }
                        Data::Map(out)
                    })
                    .collect();
                out.insert("marks".to_owned(), Data::Array(marks));
            }
        }
    }
    Data::Map(out)
}
