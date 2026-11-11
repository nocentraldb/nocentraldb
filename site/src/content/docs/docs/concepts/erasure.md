---

title: Erasure
description: Erasing a person destroys a key.
sidebar: { order: 4 }
---

The schema marks personal fields. Each is encrypted with one key per person per space. Erasing the person destroys those keys, which reaches archives and old snapshots too, while the audit trail of who changed what survives.

**What it does not reach:** devices that never come back online, a modified client that ignores the erasure, a relay backup from before the erasure that a member obtains, exports and screenshots, and personal data in unmarked free text.
