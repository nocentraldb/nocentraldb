# 2026-11-15 · NOC-325 · The core view finds a person's person-key ids by person id

- Decided by: pm, NOC-325, on the coding agent's proposal
- Why: an erasure finds a person's keys by person id in every space the account reaches (RFC 0003, "The person key"; RFC 0008, "Identifying the person"), and the view offers only reads by object id and pages of a declared index, so nothing finds the objects of a person. The person id sits in each object's hidden `$person` field (`{1: person id, 2: person-key id}`), which the view already stores. A small read in `ncdb-core`'s view, the person-key ids `$person` names for a person id in one space, gives the SDK what it needs without a scan of every collection. It is built after uWo4's view work (#335) merges, as both change `crates/ncdb-core/src/view`, and the owner reviews it.
- Instead of: the app passing the person's object in every space, which leaves spaces the app does not know of unerased, while RFC 0003 says the SDK finds the keys by person id; and a mapping the SDK builds in memory from folds, which a reopen loses and so needs the same lookup anyway.
- Topics: core, sdk-rust, personal-fields, erasure
