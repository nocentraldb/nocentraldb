//! Mentions (RFC 0013, "Mentions"): an inline node named `mention` whose
//! attributes `space` and `id` hold the target's space id and object id as
//! RFC 0009 writes them, 32 lowercase hexadecimal digits each. A node of
//! that name without both is no reference: it reads as any other node
//! outside the vocabulary, and `mentions()` leaves it out.

use std::collections::BTreeMap;

use ncdb_plugin::Data;

use super::node::Node;
use crate::ids::{ObjectId, SpaceId};

/// The reference a mention holds. It holds no name: an app reads the
/// target from the member's copy to show one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mention {
    /// The space the target lives in.
    pub space: SpaceId,
    /// The target's object id.
    pub id: ObjectId,
}

impl Mention {
    /// The Markdown form, `[@](ncdb:<space id>/<object id>)`: a link whose
    /// text is `@` and whose destination needs no escaping, so that it
    /// reads back as this reference.
    pub(crate) fn markdown(&self) -> String {
        format!("[@](ncdb:{}/{})", hex(&self.space.0), hex(&self.id.0))
    }
}

fn hex(bytes: &[u8; 16]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The references the mentions among `nodes` hold, in document order.
pub(crate) fn all(nodes: &[Node], out: &mut Vec<Mention>) {
    for node in nodes {
        if let Some(mention) = of(node) {
            out.push(mention);
        } else if let Node::Element { children, .. } = node {
            all(children, out);
        }
    }
}

/// The reference `node` holds, when it is a mention with two ids.
pub(crate) fn of(node: &Node) -> Option<Mention> {
    let Node::Element { name, attrs, .. } = node else {
        return None;
    };
    if name != "mention" {
        return None;
    }
    Some(Mention {
        space: SpaceId(id(attrs, "space")?),
        id: ObjectId(id(attrs, "id")?),
    })
}

fn id(attrs: &BTreeMap<String, Data>, key: &str) -> Option<[u8; 16]> {
    let Some(Data::Text(text)) = attrs.get(key) else {
        return None;
    };
    let digits = text.as_bytes();
    if digits.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for (n, byte) in out.iter_mut().enumerate() {
        *byte = (digit(digits[2 * n])? << 4) | digit(digits[2 * n + 1])?;
    }
    Some(out)
}

/// A lowercase hexadecimal digit's value.
fn digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}
