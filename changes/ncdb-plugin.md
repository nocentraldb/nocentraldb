---
type: feature
tickets: [NOC-248]
components: [core]
---
## Problem
A lock could name a field of a plugin type, but nothing said how the core reaches the Rust code that implements it, so such a field was never folded.

## Use
A plugin is a crate that depends on the new crate `ncdb-plugin`, implements its trait `FieldType` for each of its types, and exports `plugin()`, which returns a `Plugin` of its namespace (RFC 0011). The core builds a registry from the plugins it is given, keyed by type id and rule, and folds a plugin field through its code. It parks an operation number below 64 itself, and drops argument keys from 64 up before the plugin sees them. A rule or `config` without code parks as `UnknownType`. Values cross as `Data`, RFC 0002's CBOR subset, in its canonical form. The Rust SDK hands plugins over in a later change (`OpenOptions::plugin`).
