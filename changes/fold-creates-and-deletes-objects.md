---
type: breaking
tickets: [NOC-39]
components: [core]
---
## Problem

`ncdb-core` had objects that links resolve against, but no received change could create or delete one: nothing encoded or decoded RFC 0004 Amendment 1's create and delete, and the fold had no way to park an operation that does not decode.

## Use

`field_op::create` and `field_op::delete` write an object's create and delete exactly as RFC 0004 Amendment 1's examples E12 to E14 show, and `field_op::decode_object` reads them back. `Log::receive` now folds every create and delete of a received change into the copy's objects, so a link to a deleted object resolves to "deleted", and parks an operation that does not decode with its reason. A `Fold` hands out the objects and decodes field operations with their type's `field_op::Encoding`:

```rust
impl Fold for Copy {
    fn fold(&mut self, origin: Origin, operation: &Operation) -> Result<(), OperationParkReason> {
        let write = TITLE.decode(operation, origin)?; // parks what does not decode
        TITLE.apply(self.titles.entry(operation.object).or_default(), &write);
        Ok(())
    }

    fn objects(&mut self) -> &mut BTreeMap<ObjectId, Object> {
        &mut self.objects
    }
    // knows_collection and knows_field as before
}
```

## Migration

Two changes to the `receive::Fold` trait:

- `Fold::fold` now takes the operation's `field_op::Origin` (its id, author and stamp) instead of its `OperationId`, and returns `Result<(), OperationParkReason>`. Use `origin.id` where you used the id, and return `Ok(())` where you returned nothing, or the reason the operation parks.
- `Fold::objects` is new: return the copy's `BTreeMap<ObjectId, Object>`, which creates and deletes fold into. `Fold::fold` no longer receives operations without a field.
