---
type: internal
tickets: [NOC-196, NOC-132]
components: [core]
---
`ncdb-core`'s membership fold adds a sub-agent only when its certificate's account key statement (key 6) is under the same recovery key as its parent's, or both carry none, as RFC 0003 says a sub-agent carries its parent's account's statement; the erasure sign-off still compares account keys until NOC-201 is decided.
