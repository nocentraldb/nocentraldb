# 2026-11-23 · NOC-453 · Any member may lower its own role with members.setRole; only an admin changes another member's role or removes another member

- Decided by: Carsten, 2026-11-23, on NOC-453 (A), through pm
- Why: RFC 0008 ("Roles and rights") lets any member leave or lower its own role, and the Rust SDK has done so since NOC-169 (#202); RFC 0009 said "Only an admin may call them" of `setRole` and `remove`. RFC 0008 is the RFC about membership, and lowering one's own rights takes nothing from anyone else, so RFC 0009 now follows it. Nothing changes for developers.
- Instead of: making the code follow RFC 0009, so that only admins call `setRole`, even on themselves (B): a breaking change in behaviour and a change to RFC 0008, after which a member who wants fewer rights would have to leave the space or ask an admin.
- Topics: rfc-0009, rfc-0008, members, roles
