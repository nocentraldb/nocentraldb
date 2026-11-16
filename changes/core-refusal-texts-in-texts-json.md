---
type: internal
tickets: [NOC-342]
components: [core]
---
The core's lock refusals, which `ncdb model build`, `ncdb model check` and opening with a refused lock show, move into `texts/texts.json` as texts of kind `core`, with when each one shows, together with the name kinds they name; they generate `crates/ncdb-core/src/texts.rs`, and the gate `repo:texts` fails on a text written in `model_lock/refusal.rs`. No wording changes. A text may now hold a brace written as Rust writes it, doubled.
