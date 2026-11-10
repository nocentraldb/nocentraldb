# 2026-11-10 · NOC-149 · RFC 0009's S32 follows the code: a missing keystore entry is storageFailed, not keyStoreUnavailable

- Decided by: Carsten, 2026-11-10, on NOC-144, and confirmed on NOC-149 as #134 builds it
- Why: NOC-144 listed, among the safe subsets the owner accepted, `keyStoreUnavailable` for an existing database whose keystore entry is missing. #134 builds `StorageFailed` for that case (`crates/nocentraldb/src/keys/store.rs`), as for a missing key file, and the owner confirmed that build on NOC-149 (option A). `keyStoreUnavailable` then always means that no keystore is reachable (S26), and a lost key is a storage failure.
- Instead of: `keyStoreUnavailable` for a missing keystore entry, as NOC-144's text said, which needed a code change.
- Topics: rfc-0009, errors, sdk-rust, on-disk-format
