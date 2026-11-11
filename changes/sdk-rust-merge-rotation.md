---
type: feature
tickets: [NOC-220]
components: [sdk-rust]
---
## Problem
After a member left a space, the others kept writing under a key the leaver still held, and `status().exposed` stayed true, until an admin happened to add or remove someone.

## Use
Nothing to call. An admin's SDK that syncs sees the exposed key and rotates it on its own after a random delay of up to a minute (RFC 0008, merge rotation). The new key goes to the current members only, and `status().exposed` turns false once the rotation is folded. If another admin rotates first, this one does not.
