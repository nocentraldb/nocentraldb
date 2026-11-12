//! The registry of plugin field types (RFC 0011, "A plugin and the
//! registry"; Carsten, 2026-11-12, NOC-232): what the core holds at open,
//! for each type id and rule, the code that implements it. A field finds
//! its code by exactly the two values its lock names, `type` and `rule`.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use ncdb_plugin::{ErasedType, Plugin};

/// The namespace RFC 0005 reserves for the built-in types.
pub const BUILT_IN: &str = "ncdb";

/// Each registered type id, `<namespace>/<name>`, and rule, with its code.
#[derive(Clone, Default)]
pub struct Registry {
    types: BTreeMap<(String, u64), Arc<dyn ErasedType>>,
}

/// Why a set of plugins is refused: a mistake of the app, refused at open,
/// since folding one type id by two codes would let two members merge one
/// field by different rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    /// Two plugins name the same namespace.
    DuplicateNamespace(String),
    /// A plugin names the namespace of the built-in types.
    ReservedNamespace(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::DuplicateNamespace(namespace) => {
                write!(f, "two plugins name the namespace {namespace}")
            }
            RegistryError::ReservedNamespace(namespace) => write!(
                f,
                "a plugin names the namespace {namespace}, which is reserved for the built-in types"
            ),
        }
    }
}

impl std::error::Error for RegistryError {}

impl Registry {
    /// The registry of `plugins`, or the first refusal.
    pub fn new(plugins: impl IntoIterator<Item = Plugin>) -> Result<Registry, RegistryError> {
        let mut registry = Registry::default();
        let mut namespaces: Vec<String> = Vec::new();
        for plugin in plugins {
            let namespace = plugin.namespace().to_owned();
            if namespace == BUILT_IN {
                return Err(RegistryError::ReservedNamespace(namespace));
            }
            if namespaces.contains(&namespace) {
                return Err(RegistryError::DuplicateNamespace(namespace));
            }
            for (name, rule) in plugin.types() {
                let code = plugin.code(name, rule).expect("a listed type has code");
                registry
                    .types
                    .insert((format!("{namespace}/{name}"), rule), code);
            }
            namespaces.push(namespace);
        }
        Ok(registry)
    }

    /// Whether code for `type_id` at `rule` is registered.
    pub fn holds(&self, type_id: &str, rule: u64) -> bool {
        self.code(type_id, rule).is_some()
    }

    /// The code for `type_id` at `rule`, when registered.
    pub(crate) fn code(&self, type_id: &str, rule: u64) -> Option<&Arc<dyn ErasedType>> {
        self.types.get(&(type_id.to_owned(), rule))
    }
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.types.keys()).finish()
    }
}
