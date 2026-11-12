# RFC 0011: Plugin field types

REQUEST FOR COMMENTS · NOVEMBER 2026 · Ticket NOC-235

This RFC defines how a developer writes a field type of their own, how the core finds the code of a type that `model.lock.json` names, and what a member does with a type it has no code for. [RFC 0004](0004-merge-rules-basic-types.md) fixes what every field type must meet, [the plugin contract](0004-merge-rules-basic-types.md#the-plugin-contract), and [RFC 0005](0005-schema-and-model-lock.md) fixes how a lock names a plugin's type, [`<namespace>/<name>` with a `plugins` entry](0005-schema-and-model-lock.md#how-a-field-type-is-referenced). Until now nothing said how such a type id reaches the Rust code that implements it. This RFC does.

It builds on [RFC 0001](0001-nocentraldb.md), which lets developers add field types as plugins ([The design in one page](0001-nocentraldb.md#the-design-in-one-page)); on [RFC 0002](0002-change-and-log-format.md), whose operation map carries a plugin's operations; on RFC 0004 and RFC 0005, above; on [RFC 0006](0006-local-view.md), which keeps each field's merge state as bytes the field type encodes ([Where the view lives](0006-local-view.md#where-the-view-lives)); and on [RFC 0009](0009-sdk-api.md), whose writes to a plugin field are "what the plugin's write function returns" ([Field operations per field type](0009-sdk-api.md#field-operations-per-field-type)).

**How to give feedback.** Start a discussion in the category [RFC feedback](https://github.com/nocentraldb/nocentraldb/discussions/categories/rfc-feedback), naming RFC 0011 and the section you mean, or open a pull request against this file to change its text. [Giving feedback](README.md#giving-feedback) says how, and what happens to your point.

**Status.** Decided. The owner decided this RFC's design as it stands (Carsten, 2026-11-12).

**Out of scope.** Rich text and streams, whose names RFC 0005 reserves and whose RFCs design them as field types of their own. Migrations from one rule version to another, which RFC 0004 leaves to a later RFC. Plugins that are not field types, such as computed fields or views. A registry of published plugins, or any way to find a plugin by name.

## Summary

| Question | Answer |
| --- | --- |
| What does a plugin author write? | A Rust crate that implements the trait `FieldType` from the crate `ncdb-plugin`, with its own state, operations and read value, and that exports `plugin()`. |
| What does the core add to RFC 0004's contract? | Two functions: `decode_state`, which reads a state back from its bytes, and `write`, which turns what the app writes into operations. |
| How do a plugin and the app exchange values? | As `Data`, one type for both SDKs: RFC 0002's CBOR subset of null, booleans, integers, floats, text, bytes, arrays and maps. |
| How does a type id reach its code? | One crate is one namespace. Its `Plugin` lists its types and the rule versions it implements, and the registry maps each type id and rule to its code. |
| How does an app hand plugins over? | `OpenOptions::plugin(acme_ncdb_counter::plugin())` in the Rust SDK, once per plugin. |
| And in TypeScript? | Not in v1: a TypeScript member parks plugin fields. `ncdb core build` comes next and builds an app's own wasm core with its plugins. |
| What if a type is not registered? | The database opens anyway. The field's operations park as `UnknownType`, the field reads as unset, and a write to it is refused with `unknownType`. |
| How is a plugin checked? | With `ncdb-harness`, a public crate that runs RFC 0004's checks C1 to C7 in the plugin's own CI. |

## Terms

| Term | Meaning in this RFC |
| --- | --- |
| Field type | A merge rule for one kind of field, as RFC 0004 defines it: a state, operations, apply, read and outcome. |
| Built-in type | A field type in the namespace `ncdb/`, which the core ships ([RFC 0005, "How a field type is referenced"](0005-schema-and-model-lock.md#how-a-field-type-is-referenced)). |
| Plugin | A Rust crate that implements the field types of one namespace and exports `plugin()`. |
| Plugin type | A field type of a plugin, named `<namespace>/<name>` in the lock. |
| Type id, rule | The field's `type` and `rule` in the lock, such as `acme/counter` and `2`. Together they name one merge rule, which never changes ([RFC 0004, "The plugin contract"](0004-merge-rules-basic-types.md#the-plugin-contract)). |
| Config | The field's `config` in the lock: the plugin type's options, as the plugin defines them. |
| Registry | What the core holds at open: for each type id and rule, the code that implements it. |
| Write intent | What the app writes to a plugin field, which the plugin's `write` turns into operations. |
| Registered | A type id and rule the registry holds. |

## Where things stand today

- [RFC 0004, "The plugin contract"](0004-merge-rules-basic-types.md#the-plugin-contract) lists what a field type defines and must prove, C1 to C7. It decided that a plugin is Rust compiled into the core, not code loaded at run time, because every member must run the identical rule natively and in WebAssembly. A plugin type numbers its operations from 64 up ([How field operations travel in a change](0004-merge-rules-basic-types.md#how-field-operations-travel-in-a-change)), and a member parks an operation it cannot decode ([Operations a member does not understand](0004-merge-rules-basic-types.md#operations-a-member-does-not-understand)).
- [RFC 0005](0005-schema-and-model-lock.md#how-a-field-type-is-referenced) names a plugin type `<namespace>/<name>`; the lock's `plugins` entry names the namespace's `crate` and `npm` package; a field reads `{ type, rule, since, config }`; and a client runs the exact rule the lock names. A plugin type cannot be indexed in v1 ([Indexes](0005-schema-and-model-lock.md#indexes)). An old client parks an operation on a type it does not know as "unknown type" ([Data from a newer model](0005-schema-and-model-lock.md#data-from-a-newer-model)).
- [RFC 0006](0006-local-view.md#where-the-view-lives) keeps each field's merge state in `fs_<collection>` as bytes the field type encodes, which lets a plugin keep any state it needs.
- [RFC 0009](0009-sdk-api.md#field-operations-per-field-type) writes a plugin field with "what the plugin's write function returns", and runs the core as wasm in the browser and as a native addon over the Rust SDK in Node.js ([Open and close](0009-sdk-api.md#open-and-close)).
- In the code, `ncdb-core`'s `FieldType` trait matches RFC 0004's contract but has no way to read a state back from its bytes and no write function: the view reads each built-in state by hand, and the Rust SDK builds the built-in operations itself. The view's fields are a closed list of the built-in types. The convergence harness is test code inside `ncdb-core`, which a plugin crate cannot use. A plugin field in a lock is left out of the view today, so its operations park as an unknown field.

## The field type a plugin implements

A plugin implements a typed trait, `FieldType`, and the core wraps it (Carsten, 2026-11-12, NOC-232). The author writes Rust types for the state, the operation and the read value, and the core keeps a boxed adapter that works on bytes, as `fs_<collection>` stores them. The trait holds:

- **`configure(rule, config)`**, called once per field when the core opens the lock. It reads the field's `config` and returns the configured type, or refuses a `config` it cannot read, which refuses the field as an unknown type would ([A type that is not registered](#a-type-that-is-not-registered)).
- **The initial state**, the state of a field nobody has written.
- **`state_bytes(state)`**, the state's canonical encoding: equal states, equal bytes (RFC 0004).
- **`decode_state(bytes)`**, which reads a state back from its bytes. It must round-trip: `decode_state(state_bytes(s))` gives a state equal to `s`, and the harness checks it. RFC 0004's list has no such function, because a built-in type's state never leaves the core; a plugin's state is stored by the view between two applies, so the core needs it back.
- **`decode(number, arguments, origin)`**, which reads one operation of the field from its number and its arguments, with the `Origin` the core gives it: the operation's id, its author and its stamp. It returns the operation or one of three park reasons: `UnknownOperation`, `UnknownArgument` or `MalformedArguments` ([RFC 0004, "Operations a member does not understand"](0004-merge-rules-basic-types.md#operations-a-member-does-not-understand)).
- **`apply(state, operation)`**, **`read(state)`** and **`outcome(state, operation id)`**, as RFC 0004 defines them.
- **`write(state, intent)`**, which turns a write intent of the app into the field's operations, each a number of 64 or more and its arguments. It sees the state the member's copy holds, so a write can name what it replaces, as RFC 0004's built-in writes do. RFC 0004 defines no write, because its built-in writes are RFC 0009's; a plugin's must come from the plugin.

**What the core does around it.** The core parks an operation whose number is below 64 as `UnknownOperation` before the plugin sees it, so a plugin cannot take a built-in operation's number. It applies RFC 0004's rule for argument keys, that a key from 4 to 63 must be understood and a key from 64 up may be ignored, by handing the plugin the arguments and parking what it does not understand. It decodes the stored state, applies, and encodes the state again on every apply, as it does for a built-in type.

**Not taken.** A trait over bytes only, which the core could call without an adapter: the core would be simpler, but every author would encode and decode CBOR by hand, and each would get it wrong in their own way. Extending the core's closed list of types by a macro or feature flags, which would give each app a core of its own: the core's API would then depend on the app, and two apps could not share one build. The cost of the adapter is one box per field type and a decode and encode of the state on every apply, which the built-in types already pay.

### The data a plugin reads and writes

What a plugin's `read` returns and what its `write` takes cross the SDKs as one public type, **`Data`**: null, a boolean, an integer, a float, a text, bytes, an array or a map with text keys. That is the subset of CBOR that [RFC 0002, "The CBOR profile"](0002-change-and-log-format.md#the-cbor-profile) allows, so a value read from a plugin field can be written into a change as it is, and both SDKs show it the same way. A type per plugin, which an app would import from the plugin's own package, would give typed reads but needs code generated for every SDK; `Data` needs none and can carry typed wrappers later. The Rust SDK reads a plugin field with `object.get::<Data>(name)` and writes it with `Value::from(data)`, which the core hands to the plugin's `write`.

## A plugin and the registry

**One crate is one namespace** (Carsten, 2026-11-12, NOC-232). A plugin's crate exports one function, `plugin()`, that returns a `Plugin`: its namespace, the types it implements by name, and for each type the rule versions it implements. The crate the lock names under `plugins` ([RFC 0005, "How a field type is referenced"](0005-schema-and-model-lock.md#how-a-field-type-is-referenced)) is the crate that exports it.

```rust
pub fn plugin() -> ncdb_plugin::Plugin {
    ncdb_plugin::Plugin::new("acme")
        .field_type("counter", 1, CounterV1)
        .field_type("counter", 2, CounterV2)
}
```

The core builds a **registry** from the plugins it is given. The registry maps a type id and rule, such as `acme/counter` and `2`, to the code that implements it, so a field finds its code by exactly the two values the lock names. The registry refuses two plugins for one namespace, and a plugin for the namespace `ncdb`, which RFC 0005 reserves for the built-in types; either is a mistake of the app, refused at open with `invalidValue`, since folding by two codes for one type id would let two members merge one field by different rules.

**The limit, stated plainly.** A plugin crate must keep every rule version it ever shipped, as the core keeps the built-in ones. A field never moves to another rule without a migration (RFC 0004), so data written under rule 1 is folded under rule 1 forever. A new release of a plugin that drops rule 1 makes every field that names it park, as if the type were not registered.

**Not taken.** Registering each type on its own, as `register::<Counter>()`: finer, but nothing ties a namespace to one crate, so two crates could each claim a type of `acme`. Registration at link time, through crates such as `inventory` or `linkme`: the app would write no code, but the registration is implicit, and on wasm32 it does not work reliably.

## Handing plugins to the core

A Rust app hands each plugin to the SDK when it opens the database, with **`OpenOptions::plugin`**, which it may call more than once (Carsten, 2026-11-12, NOC-232):

```rust
let db = Db::open(OpenOptions::new(include_str!("../model.lock.json"))
    .path("./data")
    .plugin(acme_ncdb_counter::plugin())
    .plugin(acme_ncdb_vote::plugin())).await?;
```

The SDK builds the registry and passes it to the core, whose `View::open` and lock reading take it, so the lock's plugin fields get their code before the view folds anything. A plugin that the lock does not name is ignored. The app's own `Cargo.toml` names the plugin's crate, so the plugin is compiled into the app's core as RFC 0004 decided: there is no loading at run time.

**Not taken.** One `OpenOptions::plugins(...)` that takes a set the app builds beforehand: one more type for the app to learn, for no gain over calling `plugin` once per crate.

## TypeScript

**There are no plugins in the TypeScript SDK in v1** (Carsten, 2026-11-12, NOC-232). A TypeScript member, in the browser or in Node.js, treats every plugin type as not registered: it parks the field's operations as `UnknownType`, reads the field as unset, and refuses a write to it with `unknownType` ([A type that is not registered](#a-type-that-is-not-registered)). It keeps every change and forwards it, so nothing is lost, and the merge result does not fork: a member that runs the plugin folds the same changes to the same state. What forks is what is available: a space shared by a Rust agent and a TypeScript app shows a plugin field on the agent only. [Agreement with GENESIS.md and the decided RFCs](#agreement-with-genesismd-and-the-decided-rfcs) states that limit against principle 7.

**Next, in a ticket of its own after the Rust path: `ncdb core build`.** It reads `plugins[*].crate` from the lock, generates a crate that registers those plugins, and builds the app's own wasm core from it, which the app passes to `nc.open({ core })`. Node.js then uses that wasm core, RFC 0009's fallback, unless the app builds its own native addon. The cost is a Rust toolchain on the developer's build machine; the user's device needs none.

**Not taken for now: plugins as wasm modules that every core loads at run time.** A native core would run them in an interpreter such as wasmi, and the browser would run them directly. It would need no Rust toolchain for TypeScript developers, would give a sandbox in which a plugin cannot reach keys or other spaces' content, and would run the same module everywhere. But it reverses RFC 0004's decision against code loaded at run time, needs an interface between the core and the module, and needs a limit on each call that stops at the same point on every member, so that a plugin that runs too long fails the same way everywhere. Apply would get slower and the core bigger. It stays possible later, if the toolchain proves too costly for TypeScript developers, and would change RFC 0004, so it needs the owner's decision then.

## A type that is not registered

**The database opens anyway** (Carsten, 2026-11-12, NOC-232). When the lock names a plugin type whose type id and rule the registry does not hold:

- The fold parks the field's operations with the reason **`UnknownType`**, the reason RFC 0004 and RFC 0005 already name, which RFC 0009's `ParkReason` now holds as `ParkReason::UnknownType`. `space.status` counts them, as it counts every parked operation ([RFC 0009, "The Rust SDK"](0009-sdk-api.md#the-rust-sdk)).
- The field reads as unset.
- A write to the field is refused with the error **`unknownType`**, which names the type id and the rule, before anything is written ([RFC 0009, "Errors"](0009-sdk-api.md#errors)).
- Once the type is registered, the next open folds what was parked, through the view's migration of an additive change, as for a new field ([RFC 0009, "Open and close"](0009-sdk-api.md#open-and-close), step 4).

This keeps what a member does not understand, as principle 3 asks and as an unknown field is kept, and it keeps working every tool that opens the database without the plugin: the command line, a cloud function, or a TypeScript app in v1.

**A rule version that differs counts as not registered.** The registry's key holds the rule, so a plugin that implements `acme/counter` rule 1 does not fold a field the lock names with rule 2. Folding it by another rule would diverge from every member that runs the right one (principle 2).

**A changed `config` is refused by the build.** A plugin type's `config` is part of its merge rule: two members that fold one field with different options could reach different states. So `ncdb model build` and the check on the device refuse a change of a field's `config`, as they refuse a change of its type ([RFC 0005, "Refused"](0005-schema-and-model-lock.md#refused)). A new field with the new `config` can be added, and the old one deprecated.

**Not taken.** Refusing to open, with `modelRefused` naming the field: the lock and the binary ship together, so a missing plugin is a build error, as RFC 0005 treats an unknown lock format, but it would stop every app and tool that lacks the plugin, which is every TypeScript app in v1. Refusing only in debug builds and opening in release builds: the same lock would then behave differently by build, which a developer meets only once it ships.

## Checking a plugin: ncdb-harness

The convergence harness becomes a public crate, **`ncdb-harness`** (Carsten, 2026-11-12, NOC-232). It is taken out of `ncdb-core`'s test code, and the core's built-in types move onto it, so there is one harness for both. A plugin crate uses it as a dev-dependency, with its own generators and its broken fixtures as RFC 0004 requires them, and runs it natively and on wasm32 (C3) in its own CI. The harness checks C1 to C7 and that `decode_state` round-trips.

**The limit, stated plainly (principle 4).** The core cannot prove that a third party's plugin converges. It runs what the app registers. The contract is checked in the plugin's CI, by the plugin's author, not on the device; an app that registers a plugin trusts its author for convergence as it trusts any dependency for correctness.

**Not taken.** A command, `ncdb plugin check`, that builds a plugin crate and runs the harness on it: it can wrap the crate later, but the crate comes first. A short harness run on every registration, at open: it would slow every open and prove little, since a short run finds only gross mistakes.

## What a plugin depends on: ncdb-plugin

A plugin depends on a small public crate, **`ncdb-plugin`**, which `ncdb-core` depends on in turn (Carsten, 2026-11-12, NOC-232). It holds what a plugin needs and nothing else:

- the trait `FieldType`;
- `Data`;
- `Origin`, `OperationId`, `MemberId` and `Hlc`, which name an operation, its author and its stamp;
- the three park reasons a plugin may return: `UnknownOperation`, `UnknownArgument` and `MalformedArguments`;
- `Plugin`.

The view, SQLite, the keys and the bytes of a change stay private to `ncdb-core`, which stays free to change them.

**Not taken.** A public module inside `ncdb-core`: one crate fewer, but every plugin would depend on the whole core and on its version, and every change of the core would be a change for plugin authors.

## The view keeps no column for a plugin field

A plugin type cannot be indexed in v1 ([RFC 0005, "Indexes"](0005-schema-and-model-lock.md#indexes)), and RFC 0006 gives a field a column in `c_<collection>` only for a read value that can be sorted; a field without one is served from its merge state ([RFC 0006, "Where the view lives"](0006-local-view.md#where-the-view-lives)). So a plugin field has its merge state in `fs_<collection>` and no column. A read decodes the state and calls the plugin's `read`. An index on a plugin type needs an order on its read values, which `Data` does not define; it comes with a later RFC.

## What a plugin can reach

A plugin is code in the member's process. It sees the decrypted content of its fields, and it runs next to the keys, so it is trusted like any dependency of the app ([GENESIS.md, "What it must never do"](../../factory/GENESIS.md#what-it-must-never-do)): an app registers only plugins whose authors it trusts, as it adds only crates it trusts.

- **It never logs content.** `Debug` of a plugin's state, operations and read values prints `[redacted]`, as the SDK does for content ([RFC 0003, "Keys never reach logs, errors or telemetry"](0003-keys-and-cryptography.md#keys-never-reach-logs-errors-or-telemetry)), and `ncdb-plugin`'s types do the same.
- **It reaches no key through `ncdb-plugin`.** The crate gives it no key, no other field and no other space: the core hands it one field's state and operations. A plugin that is malicious is still Rust in the process and can reach anything the process can, which is why it is trusted like any dependency and not sandboxed in v1.
- **The relay is not affected.** A plugin's operations sit inside the change's encrypted payload, as every operation does ([RFC 0002](0002-change-and-log-format.md)), so the relay sees no more than for a built-in type (principle 1).

## What this does not do

- **It does not prove a plugin converges.** The harness checks it in the plugin's CI; the core runs what the app registers.
- **It does not sandbox a plugin.** A plugin is trusted code in the member's process, like any dependency.
- **It gives TypeScript no plugins in v1.** A plugin field reads as unset on a TypeScript member and cannot be written there, until `ncdb core build` lands.
- **It does not change a field's rule or `config`.** Both stay as the field was added, until migrations exist.
- **It does not index a plugin field.** There is no column and no order for its values yet.
- **It does not keep old rule versions for a plugin.** A plugin release that drops a rule version makes the fields that name it park.

## Agreement with GENESIS.md and the decided RFCs

| [GENESIS.md](../../factory/GENESIS.md#principles-in-priority-order) says | This RFC |
| --- | --- |
| Principle 1, "The relay never reads content." | A plugin's operations travel inside the encrypted payload; the relay sees nothing new. |
| Principle 2, "Every copy converges." | The registry keys a type by its rule; a differing rule or `config` is never folded by other code; `ncdb-harness` checks C1 to C7. |
| Principle 3, "Nothing is lost silently." | A type without code is parked as `UnknownType` and counted, never dropped, and folded once registered. |
| Principle 4, "Say plainly what it does not do." | [What this does not do](#what-this-does-not-do) and the limits of the registry and the harness. |
| "Never write content … into logs." | `Debug` of a plugin's state and values prints `[redacted]`. |
| "Never break an existing client or stored data." | A plugin keeps every rule version it shipped; `config` never changes under a field. |

**Conflict, accepted by the owner.** Principle 7 says behaviour never forks by platform. In v1 a plugin field is available on a Rust member and not on a TypeScript member. The merge result does not fork, since every member that folds the field folds it the same way, but what an app can show does; `ncdb core build` closes the gap.

### Changes to the decided RFCs

Each change is written into that text in this pull request, where it belongs, with the owner's decision named where it stands ([How a decided RFC changes](README.md#how-a-decided-rfc-changes)):

- **RFC 0004**, "The plugin contract": a plugin is Rust compiled into the app's core, as this RFC defines it, in place of "until a plugin RFC says otherwise".
- **RFC 0005**, "What this RFC does not cover" points to this RFC for writing and loading a plugin; "Refused" gains a change of a plugin field's `config`.
- **RFC 0009**: `ParkReason::UnknownType` joins the park reasons of the Rust SDK, in place of the sentence that no core reason produces it; the error `unknownType` joins "Errors"; and `OpenOptions::plugin` and `unknownType` join "Public names".

## History

- 2026-11-12 · First published, with the design the owner decided on NOC-232 (NOC-235)

## How this RFC is tested

`scripts/rfc-0011.test.mjs`, run by `repo:test` in `pnpm gates`, checks that the RFC index lists this RFC, that its status says the owner decided it and when, that it links GENESIS.md and the RFCs it builds on and every link resolves, that it has a section for every part NOC-235 names, that it reads as one design with no list of decisions or open questions, that each of the owner's seven decisions stands in its section with its source and the options not taken, that RFC 0004, RFC 0005 and RFC 0009 point to it and take its names, and that the decision log records it. Whether the design is right is the owner's review. The design itself is tested where it is built: `ncdb-plugin` and `ncdb-harness` with the core's built-in types moved onto them, and the Rust SDK's `OpenOptions::plugin`, each in its own ticket.
