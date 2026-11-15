# 2026-11-15 · NOC-320 · The Rust names of sub-agents and ad-hoc spaces

- Decided by: Carsten, 2026-11-15, on NOC-320 (Q10)
- Why: RFC 0009 names them in TypeScript only. In Rust: `OpenOptions::sub_agent`, `Spaces::create_ad_hoc`, `AdHoc`, `AdHocCreated`, `Space::source_of`, `Space::copy_of`, `Space::discard`, and `NoticeCode::AdHocSpaceCreated`, whose text says that the space limits what the sub-agent can decrypt, not what a process running as the same user can read. RFC 0009's table of Rust names gains each in the slice that builds it.
- Instead of: names of their own for Rust; the SDKs keep one vocabulary.
- Topics: sub-agents, ad-hoc-spaces, sdk-rust, naming
