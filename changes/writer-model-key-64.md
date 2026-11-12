---
type: feature
tickets: [NOC-197]
components: [core, sdk-rust]
---
## Problem
A change written by an app of another model, with field names of another meaning, would be folded by the rules of the member's own model. And an old app could not say which model version its parked data needs.

## Use
Nothing to call. Every change the SDK writes names the model that wrote it, the `model.id` and `model.version` of its lock, inside the encrypted payload (RFC 0002, "The writer's model"). A member parks a change of another model whole, with the reason "another model", and reads the newest model version its parked data of its own model needs, which the space status reports in a later change.
