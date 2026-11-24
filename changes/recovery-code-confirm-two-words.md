---
type: breaking
tickets: [NOC-502]
components: [sdk-rust, sdk-ts]
---
## Problem

Confirming the recovery code took one tap. A person who never wrote the words down learned it only once every device was lost, and then the account was gone, because the SDK had already forgotten the code.

## Use

Ask for two of the words back before the SDK forgets the code. `challenge()` gives two positions, counted from 1, that the SDK picked at random; `confirm()` takes the words typed at them, in that order. Letter case and spacing do not matter. A wrong word fails with `invalidValue`, naming its position but never a word, and the code stays.

```ts
const code = await db.account.recoveryCode();
if (code) {
  show(code.words());
  const [first, second] = code.challenge();
  await code.confirm([ask(first), ask(second)]);
}
```

```rust
if let Some(code) = db.account().recovery_code().await? {
    show(code.words());
    let [first, second] = code.challenge();
    code.confirm([&ask(first), &ask(second)]).await?;
}
```

## Migration

Replace `code.confirm()` with a step that asks the person for the words at the positions of `code.challenge()` and passes them to `code.confirm([first, second])`. In Rust, `confirm` now borrows the code instead of consuming it, so the same object can be confirmed again after a typo.
