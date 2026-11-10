# 2026-11-10 · NOC-149 · RFC 0009's S32 follows the code: a missing keystore entry is storageFailed, not keyStoreUnavailable

- Decided by: coding agent (NOC-149), as pm's ticket and #134 say
- Why: NOC-144 listed, among the safe subsets the owner accepted, `keyStoreUnavailable` for an existing database whose keystore entry is missing. #134 builds `StorageFailed` for that case (`crates/nocentraldb/src/keys/store.rs`), as for a missing key file, and NOC-149 asks for that. The RFC records what the code does, and the PR names the difference so the owner can see it.
- Instead of: writing NOC-144's wording into RFC 0009, which would put the RFC out of step with the merged code.
- Topics: rfc-0009, errors, sdk-rust, on-disk-format
