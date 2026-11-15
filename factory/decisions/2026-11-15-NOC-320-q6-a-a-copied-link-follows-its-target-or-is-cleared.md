# 2026-11-15 · NOC-320 · A copied link points to the copy of its target, or is cleared

- Decided by: Carsten, 2026-11-15, on NOC-320 (Q6 a)
- Why: a link to an object that was not copied would carry that object's id into the ad-hoc space, which RFC 0003 forbids ("the sub-agent never learns the source's id"). A link, one or many, whose target is copied too points to the target's copy; one whose target is not copied is cleared, and the notice says how many were.
- Instead of: refusing a copy whose links leave the set, which would make the parent copy more than the task needs.
- Topics: sub-agents, ad-hoc-spaces
