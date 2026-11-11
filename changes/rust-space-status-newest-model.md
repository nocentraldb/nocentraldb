---
type: feature
tickets: [NOC-197]
components: [sdk-rust]
---
## Problem
A Rust app could not tell that some of a space's data needs a newer version of the app, or that some of it belongs to another app's model and no upgrade will show it.

## Use
`space.status().current().newest_model` gives the highest version of the app's own model that the space's parked data needs, read from each change's writer's model. `parked` counts changes of another model under their own reason, `ParkReason::AnotherModel`.

```rust
let status = project.status().current();
if status.newest_model.is_some_and(|needed| needed > MY_MODEL_VERSION) {
    show("Some data needs a newer version of this app");
}
if status.parked.contains_key(&ParkReason::AnotherModel) {
    show("Some data was written by another app");
}
```
