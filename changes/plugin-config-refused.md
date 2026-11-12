---
type: feature
tickets: [NOC-251]
components: [core, cli]
---
## Problem
Changing the `config` of a plugin field let members merge the field's existing values by different options, so their copies could diverge.

## Use
`ncdb model build` now refuses a change of a plugin field's `config`, and so does the check when a device opens the database, as they refuse a change of type (RFC 0005, "Refused"). To use new options, add a new field with the new `config` and deprecate the old one:

```text
$ ncdb model build
model.lock.json: refused, the change is not additive (model version stays 2)
  x tasks.votes: the config of acme/counter changes.
    The config is part of the type's merge rule: members would merge existing values differently.
    Add a new field with the new config instead, and deprecate tasks.votes.
```
