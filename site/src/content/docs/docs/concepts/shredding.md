---

title: Erasing a person
description: Erasing a person destroys the keys of their personal fields.
sidebar: { order: 4 }
---

The schema marks **personal fields**. Each is encrypted with one key per person per space, the person key. To **erase** a person, a member destroys that person's keys in every space of its database where it may write. Erasing needs the **sign-off** of a second account; in a space with only one account, the erasure is provisional for 7 days and can be taken back. The audit trail of who changed what survives it, and every member can read which erasures happened, from `space.erasures`.

In 0.1, the Rust SDK erases (`db.erase`). The browser SDK reads the erasure audit, but cannot start an erasure yet.

**What it does not reach:** devices that never come back online, a modified client that ignores the erasure, a relay backup from before the erasure that a member obtains, exports and screenshots, and personal data in unmarked free text.

Whether destroying a key counts as erasure for you is for your data protection officer to decide. This page describes what the software does, and it is not legal advice.
