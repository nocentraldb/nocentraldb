---
type: feature
tickets: [NOC-318]
components: [sdk-rust]
---
## Problem

An agent that hands part of its task to a sub-agent had no way to give the sub-agent an identity of its own: the sub-agent would have needed the agent's member key, or an account it must never hold.

## Use

The sub-agent opens its database with `OpenOptions::sub_agent()`. Its invitation needs no certificate; the parent agent adds it to a space with `Members::add`, which signs the sub-agent's certificate with the agent's own member key, so the sub-agent answers through its parent to the account. A sub-agent creates no space of its own: its parent adds it to the spaces it works in.
