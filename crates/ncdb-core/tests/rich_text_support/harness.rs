//! The adapter that plugs `ncdb/richText` into the convergence harness
//! (RFC 0013, "How rich text meets the plugin contract"): members edit a
//! TipTap-shaped document with Yrs under their own client ids, and a copy
//! folds what it receives as the view does, parking what the core parks
//! and folding the field again, in stamp order, when an operation
//! displaces others that gave its ids other content.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use ncdb_core::cbor::{Key, Value};
use ncdb_core::change::Operation;
use ncdb_core::clock::Hlc;
use ncdb_core::field::{FieldType, Outcome as CoreOutcome};
use ncdb_core::field_op::{Encoding, Origin};
use ncdb_core::ids::{ChangeId, MemberId, ObjectId, OperationId};
use ncdb_core::rich_text::{Edit, Fit, Read, RichText, State, client_id};
use proptest::prelude::*;
use yrs::types::text::Text;
use yrs::types::xml::{Xml, XmlElementPrelim, XmlFragment, XmlIn, XmlOut, XmlTextPrelim};
use yrs::updates::decoder::Decode;
use yrs::{Any, Doc, OffsetKind, Options, ReadTxn, Transact, Update, XmlFragmentRef};

use super::ROOT;
use ncdb_harness::{Author, Generator, Member, Mergeable, Op, OpId, Outcome};

/// One rich-text operation as the harness carries it.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub stamp: u64,
    pub update: Vec<u8>,
    pub clients: Vec<(u64, [u8; 8])>,
}

/// The core's operation id for the harness's, ordered alike.
pub fn operation_id(id: OpId) -> OperationId {
    let mut change = [0u8; 32];
    change[..4].copy_from_slice(&id.to_be_bytes());
    OperationId {
        change: ChangeId(change),
        position: 0,
    }
}

fn op_id(id: OperationId) -> OpId {
    OpId::from_be_bytes(id.change.0[..4].try_into().unwrap())
}

pub fn member_id(member: Member) -> MemberId {
    MemberId([member.wrapping_add(1); 32])
}

/// The nonce every member writes under, through its one lease.
pub const NONCE: [u8; 8] = [7; 8];

/// The operation 5 that carries `body`.
pub fn operation(body: &Body) -> Operation {
    let clients = body
        .clients
        .iter()
        .map(|(client, nonce)| (Key::Unsigned(*client), Value::Bytes(nonce.to_vec())))
        .collect();
    Operation::new(
        ObjectId([1; 16]),
        "notes".to_owned(),
        Some("body".to_owned()),
        5,
        Some(Value::Map(
            [
                (Key::Unsigned(1), Value::Bytes(body.update.clone())),
                (Key::Unsigned(2), Value::Map(clients)),
            ]
            .into(),
        )),
    )
}

/// The core's edit for a harness operation, or `None` when it parks.
pub fn decode(op: &Op<Body>) -> Option<Edit> {
    let origin = Origin {
        id: operation_id(op.id),
        author: member_id(op.author),
        stamp: Hlc(op.body.stamp),
    };
    RichText.decode(&operation(&op.body), origin).ok()
}

/// A copy of the field as the view folds it: every operation received,
/// the ones parked, and the state.
#[derive(Clone, Debug, Default)]
pub struct Copy {
    received: BTreeMap<OpId, Option<Edit>>,
    parked: BTreeSet<OpId>,
    pub state: State,
}

impl Copy {
    /// Folds the field again from every operation that decodes, in stamp
    /// order, parking each that does not fit, as the view does when an
    /// operation displaces others.
    fn refold(&mut self) {
        self.state = State::default();
        self.parked.clear();
        let mut edits: Vec<(OpId, &Edit)> = Vec::new();
        for (id, edit) in &self.received {
            match edit {
                Some(edit) => edits.push((*id, edit)),
                None => {
                    self.parked.insert(*id);
                }
            }
        }
        edits.sort_by_key(|(_, edit)| edit.order());
        for (id, edit) in edits {
            if RichText.try_apply(&mut self.state, edit).is_err() {
                self.parked.insert(id);
            }
        }
    }

    /// The document's state, for a member that edits from this copy.
    pub fn document(&self) -> Vec<u8> {
        self.state.document().to_vec()
    }
}

impl Mergeable for Copy {
    type Change = Body;
    type Read = Read;

    fn apply(&mut self, op: &Op<Body>) {
        if self.received.contains_key(&op.id) {
            return;
        }
        let edit = decode(op);
        self.received.insert(op.id, edit.clone());
        let Some(edit) = edit else {
            self.parked.insert(op.id);
            return;
        };
        match RichText.fit(&self.state, &edit) {
            Fit::Displaces(_) => self.refold(),
            _ => {
                if RichText.try_apply(&mut self.state, &edit).is_err() {
                    self.parked.insert(op.id);
                }
            }
        }
    }

    fn state_bytes(&self) -> Vec<u8> {
        RichText.state_bytes(&self.state)
    }

    fn read(&self) -> Read {
        RichText.read(&self.state)
    }

    fn outcome(&self, id: OpId) -> Outcome {
        if self.parked.contains(&id) {
            return Outcome::Parked;
        }
        match RichText.outcome(&self.state, operation_id(id)) {
            Some(CoreOutcome::Visible) => Outcome::Visible,
            Some(CoreOutcome::Replaced(by)) => Outcome::Replaced(op_id(by)),
            Some(CoreOutcome::Lost) => Outcome::Lost,
            None => panic!("operation #{id} was received but has no outcome"),
        }
    }

    fn round_trips(&self) -> bool {
        State::from_bytes(&self.state.bytes()).is_ok_and(|read| read == self.state)
    }
}

/// What a member does in one edit, drawn by the harness: the kind of edit,
/// two positions and a word.
pub type Raw = (u8, u16, u16, u8);

const WORDS: &[&str] = &["ship", "Friday", " ", "für", "日本", "😀", "a", "lost"];

/// The client id `member` writes under.
pub fn client(member: Member) -> u64 {
    client_id(&member_id(member), &NONCE)
}

/// A member's document loaded from `state`, writing under `client`.
pub fn member_doc(client: u64, state: &[u8]) -> Doc {
    let doc = Doc::with_options(Options {
        client_id: yrs::ClientID::new(client),
        offset_kind: OffsetKind::Utf16,
        ..Options::default()
    });
    let _ = doc.get_or_insert_xml_fragment(ROOT);
    // A broken fold's document may not load whole; its member edits what
    // loads, and the harness then finds the copies apart.
    if let Ok(update) = Update::decode_v1(state) {
        let _ = doc.transact_mut().apply_update(update);
    }
    doc
}

/// One edit of a TipTap-shaped document: blocks are paragraphs and
/// headings holding one text each. Returns the transaction's update.
pub fn edit(doc: &Doc, &(kind, a, b, word): &Raw) -> Vec<u8> {
    let body = doc.get_or_insert_xml_fragment(ROOT);
    let mut txn = doc.transact_mut();
    let blocks = body.len(&txn);
    let word = WORDS[usize::from(word) % WORDS.len()];
    let block = if blocks == 0 {
        0
    } else {
        u32::from(a) % blocks
    };
    let text = (blocks > 0).then(|| text_of(&body, &txn, block)).flatten();
    match (kind % 9, text) {
        (_, None) | (0, _) => {
            let tag = if b % 4 == 0 { "heading" } else { "paragraph" };
            let at = if blocks == 0 {
                0
            } else {
                u32::from(b) % (blocks + 1)
            };
            body.insert(
                &mut txn,
                at,
                XmlElementPrelim::new(tag, [XmlIn::from(XmlTextPrelim::new(word))]),
            );
        }
        (1 | 2, Some(text)) => {
            let at = boundary(&text, &txn, b);
            text.insert(&mut txn, at, word);
        }
        (3, Some(text)) => {
            let at = boundary(&text, &txn, b);
            let end = boundary(&text, &txn, b.wrapping_add(a));
            if at != end {
                text.remove_range(&mut txn, at.min(end), at.max(end) - at.min(end));
            }
        }
        (4, Some(text)) => {
            let at = boundary(&text, &txn, b);
            let end = boundary(&text, &txn, b.wrapping_add(a));
            let attrs = [mark(a, word)].into();
            if at != end {
                text.format(&mut txn, at.min(end), at.max(end) - at.min(end), attrs);
            }
        }
        (5, Some(_)) => {
            if let Some(XmlOut::Element(el)) = body.get(&txn, block) {
                el.insert_attribute(&mut txn, "level", (1 + b % 3).to_string());
            }
        }
        (6, Some(_)) => body.remove_range(&mut txn, block, 1),
        (7, Some(text)) => {
            // Split: the text after a point moves into a new block.
            let at = boundary(&text, &txn, b);
            let all = plain(&text, &txn);
            let tail = utf16_from(&all, at);
            let len = text.len(&txn);
            if at < len {
                text.remove_range(&mut txn, at, len - at);
            }
            body.insert(
                &mut txn,
                block + 1,
                XmlElementPrelim::new("paragraph", [XmlIn::from(XmlTextPrelim::new(tail))]),
            );
        }
        (_, Some(text)) => {
            // Join: the next block's text moves to the end of this one.
            if let Some(next) = text_of(&body, &txn, block + 1) {
                let moved = plain(&next, &txn);
                body.remove_range(&mut txn, block + 1, 1);
                let end = text.len(&txn);
                text.insert(&mut txn, end, &moved);
            }
        }
    }
    // Yrs writes a map in its hash map's order, which differs from run to
    // run; written in key order, a seed makes the same cases on every run
    // and target (C3).
    let update = txn.encode_update_v1();
    ncdb_core::rich_text::update::canonical(&update).unwrap_or(update)
}

/// A mark as TipTap writes it, picked by `pick`: a link with its four
/// attributes, bold with none, or either removed.
fn mark(pick: u16, word: &str) -> (Arc<str>, Any) {
    let link = || {
        let attrs: HashMap<String, Any> = [
            ("href", Any::from(format!("https://example.com/{word}"))),
            ("target", Any::from("_blank")),
            ("rel", Any::from("noopener noreferrer nofollow")),
            ("class", Any::Null),
        ]
        .map(|(key, value)| (key.to_owned(), value))
        .into();
        Any::from(attrs)
    };
    match pick % 4 {
        0 => (Arc::from("link"), link()),
        1 => (Arc::from("bold"), Any::from(HashMap::<String, Any>::new())),
        2 => (Arc::from("link"), Any::Null),
        _ => (Arc::from("bold"), Any::Null),
    }
}

/// Appends a paragraph holding `text`; returns the whole document, which
/// is what a member restored from an empty copy writes.
pub fn edit_text(doc: &Doc, text: &str) -> Vec<u8> {
    let body = doc.get_or_insert_xml_fragment(ROOT);
    let mut txn = doc.transact_mut();
    let at = body.len(&txn);
    body.insert(
        &mut txn,
        at,
        XmlElementPrelim::new("paragraph", [XmlIn::from(XmlTextPrelim::new(text))]),
    );
    drop(txn);
    doc.transact()
        .encode_state_as_update_v1(&yrs::StateVector::default())
}

/// The text a block holds, when it is an element that holds one.
fn text_of(body: &XmlFragmentRef, txn: &impl ReadTxn, block: u32) -> Option<yrs::XmlTextRef> {
    match body.get(txn, block)? {
        XmlOut::Element(el) => match el.get(txn, 0)? {
            XmlOut::Text(text) => Some(text),
            _ => None,
        },
        _ => None,
    }
}

/// A position between two characters of `text`, in UTF-16 code units,
/// picked by `pick`.
fn boundary(text: &yrs::XmlTextRef, txn: &impl ReadTxn, pick: u16) -> u32 {
    let all = plain(text, txn);
    let mut at = 0u32;
    let mut points = vec![0u32];
    for c in all.chars() {
        at += c.len_utf16() as u32;
        points.push(at);
    }
    points[usize::from(pick) % points.len()]
}

/// The characters of `text`, without its formatting.
fn plain(text: &yrs::XmlTextRef, txn: &impl ReadTxn) -> String {
    text.diff(txn, |_| ())
        .into_iter()
        .filter_map(|diff| match diff.insert {
            yrs::Out::Any(Any::String(s)) => Some(s.to_string()),
            _ => None,
        })
        .collect()
}

/// The part of `s` from the UTF-16 offset `at`, a character boundary.
fn utf16_from(s: &str, at: u32) -> String {
    let mut n = 0u32;
    s.chars()
        .skip_while(|c| {
            let before = n;
            n += c.len_utf16() as u32;
            before < at
        })
        .collect()
}

/// Generates the edits members make, each from its author's copy, under
/// the author's client id, through one lease: its clocks continue.
pub struct Edits;

impl Generator<Copy> for Edits {
    type Raw = Raw;

    fn raw(&self, _: Member) -> BoxedStrategy<Raw> {
        any::<Raw>().boxed()
    }

    fn make(&self, author: &Author<'_, Copy>, raw: &Raw) -> Body {
        let client = client(author.member);
        let update = edit(&member_doc(client, &author.copy.document()), raw);
        body(author, update, client, NONCE)
    }

    fn malformed(&self) -> Option<BoxedStrategy<Body>> {
        Some(malformed())
    }
}

/// A body whose stamp is above every stamp its author had seen, listing
/// `client` when the update adds items.
pub fn body<S: Mergeable<Change = Body>>(
    author: &Author<'_, S>,
    update: Vec<u8>,
    client: u64,
    nonce: [u8; 8],
) -> Body {
    let stamp = author
        .seen
        .iter()
        .map(|op| op.body.stamp + 1)
        .max()
        .unwrap_or(0);
    let adds = ncdb_core::rich_text::update::read(&update)
        .map(|shape| !shape.runs.is_empty())
        .unwrap_or(true);
    Body {
        stamp,
        update,
        clients: if adds { vec![(client, nonce)] } else { vec![] },
    }
}

/// Malformed operations (C5): arbitrary bytes, an update cut short, and a
/// real update under a client id whose nonce does not derive it, or with
/// a client id it does not add.
pub fn malformed() -> BoxedStrategy<Body> {
    let real = (any::<Raw>(), any::<u8>()).prop_map(|(raw, member)| {
        let doc = member_doc(client(member % 3), &[]);
        (edit(&doc, &raw), client(member % 3))
    });
    prop_oneof![
        (proptest::collection::vec(any::<u8>(), 0..48), any::<u64>()).prop_map(|(update, c)| {
            Body {
                stamp: 0,
                update,
                clients: vec![(c, [0; 8])],
            }
        }),
        (real.clone(), any::<u16>()).prop_map(|((update, client), cut)| {
            let end = usize::from(cut) % update.len().max(1);
            Body {
                stamp: 1,
                update: update[..end].to_vec(),
                clients: vec![(client, NONCE)],
            }
        }),
        (real, any::<[u8; 8]>(), any::<bool>()).prop_map(|((update, client), other, drop)| {
            Body {
                stamp: 2,
                update,
                clients: match drop {
                    true => vec![],
                    false => vec![(client, other)],
                },
            }
        }),
    ]
    .boxed()
}
