//! `model.lock.json`, as RFC 0005 (`docs/rfc/0005-schema-and-model-lock.md`)
//! defines it: the model of an app that `ncdb model build` writes from the
//! TypeScript schema, and that the core and both SDKs read.
//!
//! - [`ModelLock`] is the lock in memory; [`ModelLock::from_json`] reads it
//!   ([`read`]), and [`ModelLock::to_canonical_json`] writes it in its one
//!   canonical form, with the model hash ([`write`]).
//! - [`check`] holds the additive-change rule: what a lock must hold on its
//!   own, and which changes from a previous lock are refused, each as a
//!   [`Refusal`] with the message RFC 0005 gives it.
//! - [`build`] turns the description of a schema (a [`Draft`]) and the
//!   previous lock into the next lock, as `ncdb model build` does.
//!
//! The fold asks the lock which collections and fields the member's model
//! knows ([`ModelLock::knows_field`]) and which field type each field has
//! ([`ModelLock::field`]).

use std::collections::BTreeMap;
use std::fmt;

pub mod build;
pub mod check;
pub mod read;
mod refusal;
pub mod write;

pub use build::{Built, Draft, DraftCollection, build};
pub use read::LockError;
pub use refusal::{NameKind, Refusal};

/// The version of the lock's own format that this core reads and writes.
pub const FORMAT: u64 = 1;

/// The type id namespace of the built-in field types.
pub const BUILT_IN: &str = "ncdb";

/// The merge rule version of every built-in field type: RFC 0004's rule.
pub const BUILT_IN_RULE: u64 = 1;

/// A model lock: the model's identity, its plugins and its collections.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelLock {
    /// `model`: id, version and hash.
    pub model: ModelInfo,
    /// `plugins`, by namespace.
    pub plugins: BTreeMap<String, Plugin>,
    /// `collections`, by name.
    pub collections: BTreeMap<String, Collection>,
}

/// The `model` entry of a lock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelInfo {
    /// The model's lineage, drawn by its first build and never changed.
    pub id: ModelId,
    /// The model version: 1 for the first build, one higher for each build
    /// that changes the model.
    pub version: u64,
    /// `sha256:` and the hex SHA-256 of the model's content (see
    /// [`ModelLock::compute_hash`]).
    pub hash: String,
}

/// A `model.id`: 128 random bits, written as 32 lower-case hex digits.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModelId(pub [u8; 16]);

impl ModelId {
    /// A new model id from the platform's secure random generator.
    ///
    /// # Panics
    ///
    /// When the platform has no secure generator.
    pub fn random() -> ModelId {
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes).expect("the platform's secure random generator failed");
        ModelId(bytes)
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

impl fmt::Debug for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ModelId({self})")
    }
}

/// A `plugins` entry: the packages that implement the namespace's field
/// types.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Plugin {
    /// `npm`: the npm package.
    pub npm: Option<String>,
    /// `crate`: the Rust crate.
    pub crate_name: Option<String>,
}

/// A collection of the model.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Collection {
    /// The model version that added the collection.
    pub since: u64,
    /// The model version that deprecated the collection, if it is.
    pub deprecated: Option<u64>,
    /// The stored fields, by name.
    pub fields: BTreeMap<String, Field>,
    /// The computed fields, by name only (a later RFC designs them).
    pub computed: BTreeMap<String, Named>,
    /// The views, by name only (a later RFC designs them).
    pub views: BTreeMap<String, Named>,
    /// The declared indexes, by name.
    pub indexes: BTreeMap<String, Index>,
}

/// A name that takes its place in a collection's scope: a computed field
/// or a view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Named {
    /// The model version that added it.
    pub since: u64,
}

/// A declared index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Index {
    /// The fields in index order.
    pub fields: Vec<IndexField>,
    /// How many leading fields a query fixes with equality.
    pub equal: u64,
    /// Whether the index spans every space of the member or runs in one.
    pub scope: Scope,
    /// The model version that added the index in this form.
    pub since: u64,
}

/// Which rows an index orders (RFC 0005, Amendment 3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scope {
    /// Every space of the member: the lock leaves `scope` out.
    #[default]
    Member,
    /// One space at a time: the lock writes `"scope": "space"`.
    Space,
}

/// One field of an index, with its direction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexField {
    /// The field's name.
    pub field: String,
    /// Its sort direction.
    pub direction: Direction,
}

/// The sort direction of an index field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// `"asc"`.
    Asc,
    /// `"desc"`.
    Desc,
}

impl Direction {
    /// The word the lock writes.
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::Asc => "asc",
            Direction::Desc => "desc",
        }
    }
}

/// A stored field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    /// Its field type, with what the type needs (options, targets, element).
    pub kind: Kind,
    /// The version of the field type's merge rule.
    pub rule: u64,
    /// The model version that added the field.
    pub since: u64,
    /// The person the field is about, when it is personal.
    pub personal: Option<Person>,
    /// The text of `.describe()`.
    pub description: Option<String>,
    /// When and why the field was deprecated.
    pub deprecated: Option<Deprecation>,
}

/// A field's deprecation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deprecation {
    /// The model version that deprecated the field (`deprecated`).
    pub since: u64,
    /// The text of `.deprecated()` (`deprecatedReason`).
    pub reason: String,
}

/// A field type, by type id, with the keys that type needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `ncdb/string`.
    String,
    /// `ncdb/int`.
    Int,
    /// `ncdb/float`.
    Float,
    /// `ncdb/boolean`.
    Boolean,
    /// `ncdb/choice`, with its options in schema order.
    Choice(Vec<String>),
    /// `ncdb/set`, with its element's type.
    Set(Element),
    /// `ncdb/link`, with the collections a target may be in.
    Link(Vec<String>),
    /// `ncdb/links`, with the collections a target may be in.
    Links(Vec<String>),
    /// A plugin's field type: its id `<namespace>/<name>` and its options.
    Plugin {
        /// The type id.
        type_id: String,
        /// `config`, as the plugin defines it.
        config: Option<serde_json::Value>,
    },
}

/// The element of an `ncdb/set`: a basic value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Element {
    /// `ncdb/string`.
    String,
    /// `ncdb/int`.
    Int,
    /// `ncdb/float`.
    Float,
    /// `ncdb/boolean`.
    Boolean,
    /// `ncdb/choice`, with its options.
    Choice(Vec<String>),
}

/// Whom a personal field is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Person {
    /// `"self"`: the object is the person.
    Itself,
    /// The person a link field of the same collection points to.
    Link(String),
}

impl Person {
    /// The value of `person` in the lock.
    pub fn as_str(&self) -> &str {
        match self {
            Person::Itself => "self",
            Person::Link(field) => field,
        }
    }
}

impl Kind {
    /// The field type id, such as `ncdb/string`.
    pub fn type_id(&self) -> &str {
        match self {
            Kind::String => "ncdb/string",
            Kind::Int => "ncdb/int",
            Kind::Float => "ncdb/float",
            Kind::Boolean => "ncdb/boolean",
            Kind::Choice(_) => "ncdb/choice",
            Kind::Set(_) => "ncdb/set",
            Kind::Link(_) => "ncdb/link",
            Kind::Links(_) => "ncdb/links",
            Kind::Plugin { type_id, .. } => type_id,
        }
    }

    /// Whether the type is a basic value (string, int, float, boolean,
    /// choice): the types that can be personal.
    pub fn is_basic(&self) -> bool {
        matches!(
            self,
            Kind::String | Kind::Int | Kind::Float | Kind::Boolean | Kind::Choice(_)
        )
    }

    /// Whether an index can use the type: a basic value or a single link.
    pub fn is_indexable(&self) -> bool {
        self.is_basic() || matches!(self, Kind::Link(_))
    }

    /// The type as a person reads it: the id, and for a set its element's
    /// id, such as `ncdb/set of ncdb/string`.
    pub fn describe(&self) -> String {
        match self {
            Kind::Set(element) => format!("ncdb/set of {}", element.type_id()),
            other => other.type_id().to_owned(),
        }
    }

    /// The options of a choice, or of a set of choices.
    pub fn options(&self) -> Option<&[String]> {
        match self {
            Kind::Choice(options) | Kind::Set(Element::Choice(options)) => Some(options),
            _ => None,
        }
    }

    /// The target collections of a link or links.
    pub fn targets(&self) -> Option<&[String]> {
        match self {
            Kind::Link(targets) | Kind::Links(targets) => Some(targets),
            _ => None,
        }
    }
}

impl Element {
    /// The element's type id.
    pub fn type_id(&self) -> &'static str {
        match self {
            Element::String => "ncdb/string",
            Element::Int => "ncdb/int",
            Element::Float => "ncdb/float",
            Element::Boolean => "ncdb/boolean",
            Element::Choice(_) => "ncdb/choice",
        }
    }
}

impl ModelLock {
    /// The collection of this name, if the model has it.
    pub fn collection(&self, collection: &str) -> Option<&Collection> {
        self.collections.get(collection)
    }

    /// The stored field of this collection and name, if the model has it.
    pub fn field(&self, collection: &str, field: &str) -> Option<&Field> {
        self.collection(collection)?.fields.get(field)
    }

    /// Whether the model knows the collection.
    pub fn knows_collection(&self, collection: &str) -> bool {
        self.collections.contains_key(collection)
    }

    /// Whether the model knows the stored field of the collection.
    pub fn knows_field(&self, collection: &str, field: &str) -> bool {
        self.field(collection, field).is_some()
    }
}
