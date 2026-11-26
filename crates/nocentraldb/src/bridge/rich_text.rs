//! The bridge's calls for the TypeScript SDK's rich-text editor binding
//! (RFC 0013, "An editor: the binding"; NOC-461), as the module docs of
//! [`super`] list them. The bridge keeps each binding's lease by a handle,
//! and gives every lease back when the database closes.

use std::collections::BTreeMap;

use ncdb_core::cbor::Value;

use super::{Failed, entries, get, invalid, object_id, space_id, text};
use crate::db::Db;
use crate::error::{Error, ErrorCode};
use crate::rich_text::{self, Binding, RichTextUpdate};

/// The bindings open in the worker, by handle.
#[derive(Default)]
pub(super) struct Bindings {
    open: BTreeMap<u64, Binding>,
    next: u64,
}

impl Bindings {
    /// `richText.open` `{"space": id, "collection": text, "id": id,
    /// "field": text}`: leases a client id for the field and answers
    /// `{"lease": n, "client": n, "clock": n, "writable": bool, "state":
    /// bytes}`, the field's document as a Yjs update. An object the copy
    /// does not hold is `notFound`, a field that is not rich text
    /// `invalidValue`.
    pub(super) fn open(&mut self, db: &Db, args: &Value) -> Result<Value, Failed> {
        let space = space_id(args)?;
        let (collection, field) = (text(args, "collection")?, text(args, "field")?);
        let object = object_id(args, "id")?;
        let mut inner = db.lock();
        held(&inner, space)?;
        let opened = rich_text::open(&mut inner, space, collection, object, field)?;
        self.next += 1;
        let lease = opened.binding.lease;
        self.open.insert(self.next, opened.binding);
        Ok(entries([
            ("lease", Value::Unsigned(self.next)),
            ("client", Value::Unsigned(lease.client)),
            ("clock", Value::Unsigned(lease.clock)),
            ("writable", Value::Bool(opened.writable)),
            ("state", Value::Bytes(opened.document)),
        ]))
    }

    /// `richText.pull` `{"lease": n, "stateVector": bytes}`: the part of
    /// the field's document a binding with that Yjs state vector does not
    /// hold, as a Yjs update in bytes.
    pub(super) fn pull(&self, db: &Db, args: &Value) -> Result<Value, Failed> {
        let binding = self.binding(args)?;
        let state_vector = bytes(args, "stateVector")?;
        let inner = db.lock();
        let document = rich_text::document(
            &inner,
            binding.space,
            &binding.collection,
            binding.object,
            &binding.field,
        )?;
        rich_text::missing(&document, state_vector)
            .map(Value::Bytes)
            .ok_or_else(|| invalid(crate::texts::bridge::NOT_ASTATE_VECTOR))
    }

    /// `richText.write` `{"lease": n, "update": bytes}`: writes the Yjs
    /// update as one change, under the binding's client id, and answers
    /// `{"change": id}`. A reader is `notAllowed`; an update that would
    /// park on fold is `invalidValue`, and nothing is written.
    pub(super) async fn write(&self, db: &Db, args: &Value) -> Result<Value, Failed> {
        let binding = self.binding(args)?.clone();
        let update = RichTextUpdate {
            update: bytes(args, "update")?.to_vec(),
            client: binding.lease.client,
            nonce: binding.lease.nonce,
        };
        let space = db.spaces().get(binding.space).await?;
        let value = update.value();
        let written = space
            .update(
                &binding.collection,
                binding.object,
                [(binding.field.as_str(), value)],
            )
            .await?;
        Ok(entries([(
            "change",
            Value::Text(written.change.to_string()),
        )]))
    }

    /// `richText.close` `{"lease": n}`: gives the binding's client id
    /// back, with the clock it reached. Answers `null`.
    pub(super) fn close(&mut self, db: &Db, args: &Value) -> Result<Value, Failed> {
        let handle = lease(args)?;
        let binding = self
            .open
            .remove(&handle)
            .ok_or_else(|| no_binding(handle))?;
        rich_text::release(&mut db.lock(), &binding)?;
        Ok(Value::Null)
    }

    /// Gives every open binding's client id back, before the database
    /// closes.
    pub(super) fn close_all(&mut self, db: &Db) {
        let mut inner = db.lock();
        for binding in std::mem::take(&mut self.open).values() {
            let _ = rich_text::release(&mut inner, binding);
        }
    }

    fn binding(&self, args: &Value) -> Result<&Binding, Failed> {
        let handle = lease(args)?;
        self.open.get(&handle).ok_or_else(|| no_binding(handle))
    }
}

/// The space must be one the member holds.
fn held(inner: &crate::db::Inner, space: crate::id::SpaceId) -> Result<(), Failed> {
    match inner.spaces.contains_key(&space.0) {
        true => Ok(()),
        false => Err(Error::new(
            ErrorCode::NotFound,
            crate::texts::error::no_such_space(space),
        )
        .into()),
    }
}

fn lease(args: &Value) -> Result<u64, Failed> {
    match get(args, "lease") {
        Some(Value::Unsigned(handle)) => Ok(*handle),
        _ => Err(invalid(crate::texts::bridge::argument_is_anumber("lease"))),
    }
}

fn bytes<'a>(args: &'a Value, key: &str) -> Result<&'a [u8], Failed> {
    match get(args, key) {
        Some(Value::Bytes(bytes)) => Ok(bytes),
        _ => Err(invalid(crate::texts::bridge::argument_is_bytes(key))),
    }
}

fn no_binding(handle: u64) -> Failed {
    invalid(crate::texts::bridge::no_such_binding(handle))
}
