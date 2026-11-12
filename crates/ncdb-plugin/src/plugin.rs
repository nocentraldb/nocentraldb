//! A plugin (RFC 0011, "A plugin and the registry"): one crate, one
//! namespace. Its `plugin()` returns a [`Plugin`] that lists the types it
//! implements by name, and for each the rule versions, each with its code.
//!
//! The core keeps each type's code behind the object-safe [`ErasedType`]
//! and [`ErasedField`], which work on the bytes the view stores: it decodes
//! the stored state, applies, and encodes it again (RFC 0011, "What the
//! core does around it"). They are the core's, hidden from the docs.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::data::Data;
use crate::field::{ConfigError, FieldType, InvalidIntent, Origin, Outcome, ParkReason, Written};
use crate::ids::OperationId;

/// The field types of one namespace, by name and rule version, as a
/// plugin's crate exports them from `plugin()`.
#[derive(Clone)]
pub struct Plugin {
    namespace: String,
    types: BTreeMap<(String, u64), Arc<dyn ErasedType>>,
}

impl Plugin {
    /// A plugin of `namespace`, the key of the lock's `plugins` entry
    /// that names this crate, with no types yet.
    pub fn new(namespace: impl Into<String>) -> Plugin {
        Plugin {
            namespace: namespace.into(),
            types: BTreeMap::new(),
        }
    }

    /// The plugin with the type `name`, rule `rule`, implemented by
    /// `code`. The same name and rule given again replaces the earlier
    /// code.
    pub fn field_type<T: FieldType>(
        mut self,
        name: impl Into<String>,
        rule: u64,
        code: T,
    ) -> Plugin {
        self.types.insert((name.into(), rule), Arc::new(code));
        self
    }

    /// The plugin's namespace.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Each type's name and rule, in order of name, then rule.
    pub fn types(&self) -> impl Iterator<Item = (&str, u64)> + '_ {
        self.types.keys().map(|(name, rule)| (name.as_str(), *rule))
    }

    /// The code of the type `name` at `rule`, for the core's registry.
    #[doc(hidden)]
    pub fn code(&self, name: &str, rule: u64) -> Option<Arc<dyn ErasedType>> {
        self.types.get(&(name.to_owned(), rule)).cloned()
    }
}

impl fmt::Debug for Plugin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Plugin")
            .field("namespace", &self.namespace)
            .field("types", &self.types().collect::<Vec<_>>())
            .finish()
    }
}

/// Why the core could not apply an operation to a stored state.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyError {
    /// The plugin parks the operation.
    Park(ParkReason),
    /// The stored bytes are not a state the plugin wrote.
    Corrupt,
}

/// Why the core could not turn a write intent into operations.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteError {
    /// The plugin refuses the intent.
    Invalid(InvalidIntent),
    /// The stored bytes are not a state the plugin wrote.
    Corrupt,
}

/// A registered type's code before a field configures it: the core's view
/// of a [`FieldType`].
#[doc(hidden)]
pub trait ErasedType: Send + Sync {
    /// The code configured for one field.
    fn configure(
        &self,
        rule: u64,
        config: Option<&Data>,
    ) -> Result<Arc<dyn ErasedField>, ConfigError>;
}

/// One field's code over the bytes the view stores. A missing state is the
/// state of a field nobody has written.
#[doc(hidden)]
pub trait ErasedField: Send + Sync {
    /// The bytes of the state of a field nobody has written.
    fn initial(&self) -> Vec<u8>;
    /// The state after one operation, as bytes, or why not.
    fn apply(
        &self,
        state: Option<&[u8]>,
        number: u64,
        arguments: &BTreeMap<u64, Data>,
        origin: Origin,
    ) -> Result<Vec<u8>, ApplyError>;
    /// What the app reads, or `None` for bytes the plugin did not write.
    fn read(&self, state: Option<&[u8]>) -> Option<Data>;
    /// Where the operation `id` went, or `None` when the state never
    /// received it or the bytes are no state.
    fn outcome(&self, state: Option<&[u8]>, id: OperationId) -> Option<Outcome>;
    /// The operations that carry out `intent`, or why not.
    fn write(&self, state: Option<&[u8]>, intent: &Data) -> Result<Vec<Written>, WriteError>;
}

impl<T: FieldType> ErasedType for T {
    fn configure(
        &self,
        rule: u64,
        config: Option<&Data>,
    ) -> Result<Arc<dyn ErasedField>, ConfigError> {
        Ok(Arc::new(Configured(FieldType::configure(
            self, rule, config,
        )?)))
    }
}

/// A field type configured for one field.
struct Configured<T>(T);

impl<T: FieldType> Configured<T> {
    fn state(&self, bytes: Option<&[u8]>) -> Option<T::State> {
        match bytes {
            None => Some(T::State::default()),
            Some(bytes) => self.0.decode_state(bytes),
        }
    }
}

impl<T: FieldType> ErasedField for Configured<T> {
    fn initial(&self) -> Vec<u8> {
        self.0.state_bytes(&T::State::default())
    }

    fn apply(
        &self,
        state: Option<&[u8]>,
        number: u64,
        arguments: &BTreeMap<u64, Data>,
        origin: Origin,
    ) -> Result<Vec<u8>, ApplyError> {
        let mut state = self.state(state).ok_or(ApplyError::Corrupt)?;
        let operation = self
            .0
            .decode(number, arguments, origin)
            .map_err(ApplyError::Park)?;
        self.0.apply(&mut state, &operation);
        Ok(self.0.state_bytes(&state))
    }

    fn read(&self, state: Option<&[u8]>) -> Option<Data> {
        Some(self.0.read(&self.state(state)?))
    }

    fn outcome(&self, state: Option<&[u8]>, id: OperationId) -> Option<Outcome> {
        self.0.outcome(&self.state(state)?, id)
    }

    fn write(&self, state: Option<&[u8]>, intent: &Data) -> Result<Vec<Written>, WriteError> {
        let state = self.state(state).ok_or(WriteError::Corrupt)?;
        self.0.write(&state, intent).map_err(WriteError::Invalid)
    }
}
