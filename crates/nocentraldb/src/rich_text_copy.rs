//! Re-creating a rich-text document in another space, for a move (RFC
//! 0016, "Its rich-text fields") and an ad-hoc copy (RFC 0013, "Moving and
//! copying the object"; NOC-539; the owner's decision on NOC-543).
//!
//! A document's items name their writers by client ids, which the other
//! space's members accept only from the members they derive from. So the
//! member that moves or copies the object writes the document again: from
//! its tree, node by node, under a client id it leases in the other space,
//! as one ordinary rich-text update. Every node, mark and attribute is
//! copied as Yjs holds it, a number as a number; deleted text and pending
//! content are not part of the tree and stay behind.
//!
//! Built only with the compiler cfg `ncdb_rich_text` (NOC-468).

use ncdb_core::ids::{ObjectId as CoreObjectId, SpaceId as CoreSpaceId};
use ncdb_core::view::Lease;
use yrs::types::text::{Diff, YChange};
use yrs::types::xml::{
    Xml, XmlElementPrelim, XmlElementRef, XmlFragment, XmlFragmentPrelim, XmlIn, XmlOut,
    XmlTextPrelim, XmlTextRef,
};
use yrs::types::{AsPrelim, Delta};
use yrs::updates::decoder::Decode;
use yrs::{
    ClientID, Doc, In, Options, Out, ReadTxn, StateVector, Text, Transact, TransactionMut, Update,
};

use crate::db::Inner;
use crate::error::{Error, storage};
use crate::rich_text::RichTextUpdate;
use crate::value::Value;

/// The root a rich-text document's editor binds (RFC 0013).
const ROOT: &str = "default";

/// A re-created document, to write into the other space, and the lease it
/// was written under, to release once the write folded.
pub(crate) struct ReCreated {
    pub(crate) value: Value,
    pub(crate) lease: Lease,
    pub(crate) clock: u64,
}

/// Leases a client id of this member for `field` of `object` in `space`
/// and re-creates `document` under it.
pub(crate) fn re_create(
    inner: &mut Inner,
    space: CoreSpaceId,
    collection: &str,
    object: CoreObjectId,
    field: &str,
    document: &[u8],
) -> Result<ReCreated, Error> {
    let member = inner.member.key().id();
    let lease = inner
        .view
        .lease_rich_text(member, space, collection, object, field)
        .map_err(storage)?;
    let (update, clock) = re_created(document, lease.client);
    let value = RichTextUpdate {
        update,
        client: lease.client,
        nonce: lease.nonce,
    }
    .value();
    Ok(ReCreated {
        value,
        lease,
        clock,
    })
}

/// Gives the leases back: with the clocks their documents reached when
/// the change that holds them was written, and as they were leased when
/// it was not, so that a client id's clocks never skip.
pub(crate) fn release(inner: &mut Inner, leased: &[ReCreated], written: bool) -> Result<(), Error> {
    for re_created in leased {
        let clock = match written {
            true => re_created.clock,
            false => re_created.lease.clock,
        };
        inner
            .view
            .release_rich_text(&re_created.lease, clock)
            .map_err(storage)?;
    }
    Ok(())
}

/// The update that writes the tree of `document` into an empty document
/// under `client`, and the clock `client` reaches.
pub(crate) fn re_created(document: &[u8], client: u64) -> (Vec<u8>, u64) {
    let source = Doc::new();
    if let Ok(update) = Update::decode_v1(document) {
        let _ = source.transact_mut().apply_update(update);
    }
    let target = Doc::with_options(Options {
        client_id: ClientID::new(client),
        ..Options::default()
    });
    let from = source.get_or_insert_xml_fragment(ROOT);
    let to = target.get_or_insert_xml_fragment(ROOT);
    {
        let read = source.transact();
        let mut write = target.transact_mut();
        children(&read, &from, &mut write, &to);
    }
    let txn = target.transact();
    let clock = u64::from(txn.state_vector().get(&ClientID::new(client)));
    (
        txn.encode_state_as_update_v1(&StateVector::default()),
        clock,
    )
}

/// Copies the children of `from` to the end of `to`, in order.
fn children<T: ReadTxn>(
    read: &T,
    from: &impl XmlFragment,
    write: &mut TransactionMut,
    to: &impl XmlFragment,
) {
    for child in from.children(read) {
        let at = to.len(write);
        match child {
            XmlOut::Element(element) => {
                let copy = to.insert(write, at, XmlElementPrelim::empty(element.tag().clone()));
                self::element(read, &element, write, &copy);
            }
            XmlOut::Text(text) => {
                let copy = to.insert(write, at, XmlTextPrelim::new(""));
                self::text(read, &text, write, &copy);
            }
            XmlOut::Fragment(fragment) => {
                let empty = XmlFragmentPrelim::new::<_, XmlIn>(Vec::new());
                let copy = to.insert(write, at, XmlIn::Fragment(empty));
                if let XmlOut::Fragment(copy) = copy {
                    children(read, &fragment, write, &copy);
                }
            }
        }
    }
}

/// Copies the attributes and children of an element.
fn element<T: ReadTxn>(
    read: &T,
    from: &XmlElementRef,
    write: &mut TransactionMut,
    to: &XmlElementRef,
) {
    attributes(read, from, write, to);
    children(read, from, write, to);
}

/// Copies the attributes of a node as Yjs holds them: a plain value as it
/// is, a shared one as a copy of it.
fn attributes<T: ReadTxn>(read: &T, from: &impl Xml, write: &mut TransactionMut, to: &impl Xml) {
    for (name, value) in from.attributes(read) {
        match value {
            Out::Any(any) => {
                to.insert_attribute(write, name, any);
            }
            other => {
                to.insert_attribute(write, name, other.as_prelim(read));
            }
        }
    }
}

/// Copies a text node: its attributes, and its runs with their marks, a
/// string as text and anything else as an embed, a copy of it.
fn text<T: ReadTxn>(read: &T, from: &XmlTextRef, write: &mut TransactionMut, to: &XmlTextRef) {
    attributes(read, from, write, to);
    let runs: Vec<Diff<YChange>> = from.diff(read, YChange::identity);
    let delta: Vec<Delta<In>> = runs
        .into_iter()
        .map(|run| {
            let insert = match run.insert {
                Out::Any(any) => In::Any(any),
                embed => embed.as_prelim(read),
            };
            Delta::Inserted(insert, run.attributes)
        })
        .collect();
    to.apply_delta(write, delta);
}
