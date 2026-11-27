//! A field that a person and an agent write: the person through a Yrs
//! document of their own, the agent through anchored edits, each folded
//! as the view folds operation 5.

#![allow(dead_code)]

use ncdb_core::ids::{ObjectId, SpaceId};
use ncdb_core::rich_text::anchor::{Anchored, Error, FieldKey, Reading, Versions, Written};
use ncdb_core::rich_text::read::{markdown, tree};
use ncdb_core::rich_text::{RichText, State};
use ncdb_core::view::Lease;
use ncdb_harness::{Member, Op};
use ncdb_plugin::Data;
use yrs::Doc;

use crate::doc::{N, update};
use crate::rich_text_support::harness::{Body, NONCE, client, decode, member_doc};

pub const PERSON: Member = 0;
pub const AGENT: Member = 1;
pub const SPACE: SpaceId = SpaceId([5; 16]);
pub const OBJECT: ObjectId = ObjectId([0x11; 16]);
/// The cache's time limit in these tests, in milliseconds.
pub const LIMIT: u64 = 60_000;
/// The clock of the reads and edits, in milliseconds.
pub const NOW: u64 = 1_000_000;

pub fn key() -> FieldKey {
    FieldKey {
        space: SPACE,
        object: OBJECT,
        collection: "notes".to_owned(),
        field: "body".to_owned(),
    }
}

/// The agent's lease of a fresh client id.
pub fn lease() -> Lease {
    Lease {
        space: SPACE,
        object: OBJECT,
        client: client(AGENT),
        nonce: NONCE,
        clock: 0,
    }
}

pub type Edit = Anchored;

pub fn replace(quote: &str, replacement: &str) -> Anchored {
    Anchored::Replace {
        quote: quote.to_owned(),
        replacement: replacement.to_owned(),
    }
}

pub fn after(quote: &str, markdown: &str) -> Anchored {
    Anchored::InsertAfter {
        quote: quote.to_owned(),
        markdown: markdown.to_owned(),
    }
}

pub fn append(markdown: &str) -> Anchored {
    Anchored::Append {
        markdown: markdown.to_owned(),
    }
}

/// A rich-text field, its operations stamped in the order they are
/// folded here.
pub struct Field {
    pub state: State,
    /// The person's document, which has seen every update folded here.
    pub person: Doc,
    next: u32,
}

impl Field {
    /// A field whose first content the person wrote.
    pub fn new(nodes: Vec<N>) -> Field {
        let first = update(&nodes);
        let mut field = Field {
            state: State::default(),
            person: member_doc(client(PERSON), &first),
            next: 0,
        };
        field.fold(PERSON, &first, vec![(client(PERSON), NONCE)]);
        field
    }

    /// Folds an operation of `author`; it must not park.
    pub fn fold(&mut self, author: Member, update: &[u8], clients: Vec<(u64, [u8; 8])>) {
        let op = Op {
            id: self.next,
            author,
            body: Body {
                stamp: u64::from(self.next),
                update: update.to_vec(),
                clients,
            },
        };
        self.next += 1;
        let edit = decode(&op).expect("the operation decodes");
        RichText
            .try_apply(&mut self.state, &edit)
            .expect("the operation folds");
    }

    /// Folds an update the person made in their document.
    pub fn person(&mut self, update: Vec<u8>) {
        self.fold(PERSON, &update, vec![(client(PERSON), NONCE)]);
    }

    /// Folds what the agent wrote, and gives it to the person's document.
    pub fn agent(&mut self, written: &Written) {
        let clients = written.clients.iter().map(|(c, n)| (*c, *n)).collect();
        self.fold(AGENT, &written.update, clients);
        crate::rich_text_support::apply(&self.person, &written.update);
    }

    pub fn markdown(&self) -> String {
        markdown(&self.state)
    }

    pub fn tree(&self) -> Data {
        tree(&self.state)
    }
}

/// An agent with its cache of versions and its one lease.
pub struct Agent {
    pub versions: Versions,
    pub lease: Lease,
}

impl Agent {
    pub fn new() -> Agent {
        Agent {
            versions: Versions::new(LIMIT),
            lease: lease(),
        }
    }

    pub fn read(&mut self, field: &Field) -> Reading {
        self.versions.read(key(), &field.state, NOW)
    }

    pub fn edit(&mut self, reading: &Reading, edits: &[Anchored]) -> Result<Written, Error> {
        self.versions
            .edit(&reading.version, &mut self.lease, edits, NOW)
    }
}

/// Reads `field`, makes `edits` at that version and folds them: the
/// Markdown after.
pub fn edited(field: &mut Field, edits: &[Anchored]) -> Result<String, Error> {
    let mut agent = Agent::new();
    let reading = agent.read(field);
    let written = agent.edit(&reading, edits)?;
    field.agent(&written);
    Ok(field.markdown())
}
