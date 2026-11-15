# 2026-11-15 · NOC-320 · The parent learns its copies from the call and by lookup

- Decided by: Carsten, 2026-11-15, on NOC-320 (Q4 c)
- Why: RFC 0009's example reads `copyOfTaskA` without saying where it comes from. `Spaces::create_ad_hoc(AdHoc { copy, personal, local })` returns `AdHocCreated { space, notice, copies }`, with each source and its copy in the order of `copy`, for the call site; `Space::source_of(copy)` and `Space::copy_of(source_space, source)` find them later, when the parent writes results back.
- Instead of: only the returned list, which a parent that reopens its database no longer holds; or only the lookups, which make the call site look up what the call just made.
- Topics: sub-agents, ad-hoc-spaces, sdk-rust
