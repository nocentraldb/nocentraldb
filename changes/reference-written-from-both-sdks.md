---
type: feature
tickets: [NOC-354]
components: [sdk-rust, sdk-ts]
---
## Problem

An `nc.ref` field, a reference to an object in any space, could not be written from either SDK, and it read as unset. The Rust SDK had no value for a reference. In the browser, `{ space, id }` was refused with `invalidValue`, "a map value is $add or $remove with a list".

## Use

In TypeScript, write the `{ space, id }` a read gives, or `null` to clear it:

```ts
await orders.update(model.orders, orderId, { customer: { space: customers.id, id: customerId } });
(await orders.get(model.orders, orderId))?.customer; // { space, id }
```

In Rust, write a `Reference`, read it with `get::<Reference>`, and clear it with `Value::clear()`:

```rust
orders.update("orders", order, [("customer", Value::from(Reference::new(customers.id(), customer)))]).await?;
let customer = orders.get("orders", order).await?.unwrap().get::<Reference>("customer")?;
```

A reference written by one SDK reads back as a reference on the other.
