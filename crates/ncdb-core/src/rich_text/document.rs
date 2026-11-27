//! The field's document in Yrs (RFC 0013, "Folding", step 2): loaded from
//! the state's bytes, the update applied, and its whole state encoded
//! again, v1 with garbage collection. The root `default` is opened before
//! any transaction, and documents count offsets in UTF-16, as Yjs does
//! (NOC-456).

use yrs::types::xml::XmlFragment;
use yrs::updates::decoder::Decode;
use yrs::{
    ClientID, Doc, GetString, OffsetKind, Options, ReadTxn, StateVector, Transact, Update,
    XmlFragmentRef,
};

/// The root fragment TipTap's collaboration binds (RFC 0013, "The field in
/// the schema and the lock").
pub(super) const ROOT: &str = "default";

/// Yrs refused an update.
pub(super) struct Refused;

/// A document that never writes: the core only applies updates.
pub(super) fn load(state: &[u8]) -> Result<(Doc, XmlFragmentRef), Refused> {
    let doc = Doc::with_options(Options {
        client_id: ClientID::new(0),
        offset_kind: OffsetKind::Utf16,
        skip_gc: false,
        ..Options::default()
    });
    let root = doc.get_or_insert_xml_fragment(ROOT);
    if !state.is_empty() {
        apply_to(&doc, state)?;
    }
    Ok((doc, root))
}

fn apply_to(doc: &Doc, update: &[u8]) -> Result<(), Refused> {
    let update = Update::decode_v1(update).map_err(|_| Refused)?;
    doc.transact_mut().apply_update(update).map_err(|_| Refused)
}

/// The document's state after `update` is applied to `state`.
pub(super) fn apply(state: &[u8], update: &[u8]) -> Result<Vec<u8>, Refused> {
    let (doc, _) = load(state)?;
    apply_to(&doc, update)?;
    let encoded = encode(&doc);
    // Loaded again in one transaction, the document's items merge as far
    // as they can, whatever order they arrived in.
    let (again, _) = load(&encoded)?;
    Ok(encode(&again))
}

fn encode(doc: &Doc) -> Vec<u8> {
    doc.transact()
        .encode_state_as_update_v1(&StateVector::default())
}

/// Whether the root fragment of `state` holds a node; `true` for a state
/// Yrs cannot load, which only the core's own bytes are.
pub(super) fn holds_content(state: &[u8]) -> bool {
    match load(state) {
        Ok((doc, root)) => root.len(&doc.transact()) > 0,
        Err(Refused) => true,
    }
}

/// The root fragment of `state` as XML; empty for a state Yrs cannot load,
/// which only the core's own bytes are.
pub(super) fn xml(state: &[u8]) -> String {
    match load(state) {
        Ok((doc, root)) => root.get_string(&doc.transact()),
        Err(Refused) => String::new(),
    }
}
