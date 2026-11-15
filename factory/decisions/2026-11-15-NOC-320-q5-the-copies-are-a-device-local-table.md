# 2026-11-15 · NOC-320 · Which object each copy came from is a device-local table of the core

- Decided by: Carsten, 2026-11-15, on NOC-320 (Q5); the layout is the coding agent's, as the owner left it
- Why: the mapping cannot be computed from the log, and the sub-agent must never learn a source's id, so it lives on the parent only and must survive a rebuild. `ad_hoc_copies (space_id, copy_id, source_space_id, source_id)`, each 16 bytes, keyed by the ad-hoc space and the copy, with one copy per source in a space, lives beside the view like `confirmed_accounts`: no fold computes it, a rebuild keeps it, it never syncs, and the view format version stays as it is. `View::record_ad_hoc_copies` records a batch all or none.
- Instead of: a field in the copy, which would carry the source's id to the sub-agent; or a row in the platform keystore, which holds keys, not ids.
- Topics: sub-agents, ad-hoc-spaces, view, format
