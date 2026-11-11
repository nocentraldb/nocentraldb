---

title: Shredding a person
description: Shredding a person destroys their key.
sidebar: { order: 4 }
---

The schema marks personal fields. Each is designed to be encrypted with one key per person per space. To **shred** a person, a member performs a `work order` and a second account gives its `sign-off`; then the key is destroyed. A person's data can be shredded this way, and the audit trail of who changed what is designed to survive it.

**What it does not reach:** devices that never come back online, a modified client that ignores the shred, a relay backup from before the shred that a member obtains, exports and screenshots, and personal data in unmarked free text.

Whether destroying a key counts as erasure for you is for your data protection officer to decide. This is a design, not legal advice.
