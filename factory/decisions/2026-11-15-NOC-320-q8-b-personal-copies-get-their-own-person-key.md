# 2026-11-15 · NOC-320 · Personal fields in copies get a person key of the ad-hoc space, and wait

- Decided by: Carsten, 2026-11-15, on NOC-320 (Q8 b)
- Why: RFC 0003 keeps one person key per person per space, with a person-key id random per space so the relay cannot link a person across spaces; the person-key commitment, record and wrap all bind the space id. The ad-hoc space therefore gets a fresh person key for the person, and the copy keeps the person id (`Create::same_person_as`), so an erasure work order that finds the person's keys by person id reaches the copy. `personal: true` waits for the cross-space erasure ticket; until then nothing makes an erasure reach the copy.
- Instead of: the same key, or the same person-key id, in both spaces, which the formats accept but RFC 0003 decided against.
- Topics: sub-agents, ad-hoc-spaces, personal-fields, erasure
