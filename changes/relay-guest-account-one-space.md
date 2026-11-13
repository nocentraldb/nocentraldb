---
type: feature
tickets: [NOC-283]
components: [relay]
---
## Problem
A relay that admits only listed accounts refused every space from any other account, so a guest invited into a space could never create its account space, which its devices need once it pairs a second one (RFC 0010).

## Use
Nothing to configure. On a relay started with `--allow-account` or `--allow-invitation`, an account that is not listed but whose member belongs to a space a listed account created may create exactly one space; a second one is refused with `not_allowed`, as is any space of an account that belongs to no such space.
