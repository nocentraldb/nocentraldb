//! Documents written the way TipTap's binding writes them (RFC 0013, "How
//! Yjs works", "TipTap's mapping"): a node is an `XmlElement` named after
//! its type, with its attributes; consecutive text is one `XmlText`, whose
//! formatting attributes are its marks, each mark's value the map of its
//! attributes. The state is what the core folds from that update.

use std::collections::HashMap;
use std::sync::Arc;

use ncdb_core::rich_text::{RichText, State};
use ncdb_harness::Op;
use yrs::types::text::Text;
use yrs::types::xml::{Xml, XmlElementPrelim, XmlFragment, XmlTextPrelim};
use yrs::{Any, Doc, ReadTxn, StateVector, Transact, TransactionMut};

use crate::rich_text_support::harness::{Body, NONCE, client, decode, member_doc};

/// A node of a document, as the test describes it.
pub enum N {
    /// An element: its name, its attributes and its children.
    El(&'static str, Vec<(&'static str, Any)>, Vec<N>),
    /// A run of text and its marks: each mark's name and its attributes.
    Text(String, Vec<(&'static str, Vec<(&'static str, Any)>)>),
}

/// An element.
pub fn el(name: &'static str, attrs: &[(&'static str, Any)], children: Vec<N>) -> N {
    N::El(name, attrs.to_vec(), children)
}

/// A paragraph holding `children`.
pub fn p(children: Vec<N>) -> N {
    el("paragraph", &[], children)
}

/// A run of text without marks.
pub fn t(text: &str) -> N {
    N::Text(text.to_owned(), Vec::new())
}

/// A run of text with marks that have no attributes.
pub fn m(text: &str, marks: &[&'static str]) -> N {
    N::Text(
        text.to_owned(),
        marks.iter().map(|mark| (*mark, Vec::new())).collect(),
    )
}

/// A run of text with one mark that has attributes.
pub fn ma(text: &str, mark: &'static str, attrs: &[(&'static str, Any)]) -> N {
    N::Text(text.to_owned(), vec![(mark, attrs.to_vec())])
}

/// A text attribute or mark attribute value.
pub fn s(value: &str) -> Any {
    Any::from(value)
}

/// An integer attribute value, as Yjs writes a JavaScript integer.
pub fn i(value: i64) -> Any {
    Any::from(value)
}

/// Writes `nodes` into `parent`.
fn write(txn: &mut TransactionMut, parent: &impl XmlFragment, nodes: &[N]) {
    let mut at = parent.len(txn);
    let mut i = 0;
    while i < nodes.len() {
        match &nodes[i] {
            N::El(name, attrs, children) => {
                let element = parent.insert(txn, at, XmlElementPrelim::empty(*name));
                for (key, value) in attrs {
                    element.insert_attribute(txn, *key, value.clone());
                }
                write(txn, &element, children);
                i += 1;
            }
            N::Text(..) => {
                let text = parent.insert(txn, at, XmlTextPrelim::new(""));
                let mut offset = 0;
                while let Some(N::Text(run, marks)) = nodes.get(i) {
                    let attrs: HashMap<Arc<str>, Any> = marks
                        .iter()
                        .map(|(name, attrs)| {
                            let attrs: HashMap<String, Any> = attrs
                                .iter()
                                .map(|(k, v)| ((*k).to_owned(), v.clone()))
                                .collect();
                            (Arc::<str>::from(*name), Any::from(attrs))
                        })
                        .collect();
                    text.insert_with_attributes(txn, offset, run, attrs);
                    offset += run.encode_utf16().count() as u32;
                    i += 1;
                }
            }
        }
        at += 1;
    }
}

/// The update that writes `nodes` into an empty document, as member 0.
pub fn update(nodes: &[N]) -> Vec<u8> {
    let doc: Doc = member_doc(client(0), &[]);
    let root = doc.get_or_insert_xml_fragment("default");
    {
        let mut txn = doc.transact_mut();
        write(&mut txn, &root, nodes);
    }
    doc.transact()
        .encode_state_as_update_v1(&StateVector::default())
}

/// The updates of member 0 appending each step's nodes to one document,
/// each made on the ones before.
pub fn steps(steps: Vec<Vec<N>>) -> Vec<Vec<u8>> {
    let doc: Doc = member_doc(client(0), &[]);
    let root = doc.get_or_insert_xml_fragment("default");
    steps
        .iter()
        .map(|nodes| {
            let mut txn = doc.transact_mut();
            write(&mut txn, &root, nodes);
            txn.encode_update_v1()
        })
        .collect()
}

/// The state the core folds from `updates`, each an operation of member 0
/// stamped in this order.
pub fn fold(updates: &[Vec<u8>]) -> State {
    fold_in(updates, &(0..updates.len()).collect::<Vec<_>>())
}

/// The state the core folds from `updates`, stamped in their order and
/// arriving in `order`.
pub fn fold_in(updates: &[Vec<u8>], order: &[usize]) -> State {
    let mut state = State::default();
    for &n in order {
        let update = &updates[n];
        let op = Op {
            id: n as u32,
            author: 0,
            body: Body {
                stamp: n as u64,
                update: update.clone(),
                clients: vec![(client(0), NONCE)],
            },
        };
        let edit = decode(&op).expect("the update decodes");
        RichText.try_apply(&mut state, &edit).expect("the update folds");
    }
    state
}

/// The state of a document holding `nodes`.
pub fn state(nodes: Vec<N>) -> State {
    fold(&[update(&nodes)])
}
