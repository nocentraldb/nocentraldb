---
type: feature
tickets: [NOC-257, NOC-239]
components: [relay]
---
## Problem

The local relay lets a space be created only by members of the accounts it was started with, named by their account keys in hex. The SDK never shows an account key, so a developer who uses only the public API could not start a relay their app may create spaces on.

## Use

Start the relay with the text of one of the app's invitations; the relay admits that invitation's account:

```text
ncdb-relay --database relay.sqlite --allow-invitation "ncdb-invite-1:Aa8Go-Mp…"
```

The app gets the text from `db.invitation().await.to_string()`. The flag may be given more than once and combines with `--allow-account`. A text that is not an invitation stops the relay at start with exit code 2.
