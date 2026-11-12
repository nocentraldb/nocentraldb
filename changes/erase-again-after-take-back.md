---
type: feature
tickets: [NOC-258]
components: [core, sdk-rust]
---
## Problem
In a space with one account, once a provisional erasure was taken back, the person could never be erased again in that space: a later erasure of the same person changed nothing.

## Use
Erase the person again. Of two erasures of one person, the first that is not taken back takes effect, so a later one erases the person again, provisionally, with its own 7 days (RFC 0008, "The function"). `space.erasures().list()` shows the erasure taken back as `TakenBack` and the later one with its own state.
