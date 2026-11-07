# RFC 0004: Merge rules for basic values, sets and links

REQUEST FOR COMMENTS · DRAFT 0.1 · NOVEMBER 2026 · Ticket NOC-8

This RFC defines how the first field types of NoCentralDB merge concurrent changes, so that every copy of a space reaches the same state: basic values, sets and links. It also defines the contract every other field type, built in or plugin, must meet before the core accepts it.

It builds on [RFC 0001](0001-nocentraldb.md), which promises that "the same set of changes produces the same state on every device, in any order of arrival", and that each field merges by the rules of its type. RFC 0001 says what the shipped types do in one line each. This RFC makes those lines exact.

**Status.** Draft for the owner's sign-off. Merge rules are hard to change once a type ships (a changed rule makes old and new clients diverge), so nothing here is built until this RFC is merged. The implementation tickets are NOC-16 (basic values) and NOC-17 (sets and links). The convergence harness that checks the rules is NOC-9.

**Out of scope.** Rich text, streams and personal fields (later RFCs). Computed fields and views. The encoding of changes and of the ordering metadata (RFC 0002, NOC-6). Object deletion (see [Links](#links) for what this RFC assumes about it). Of object creation, this RFC defines only what two creates of one object id mean.

## Summary

| Type                                                      | State                                                         | Read value                                         | Concurrent writes                                                 | What stays visible to the app                                    |
| --------------------------------------------------------- | ------------------------------------------------------------- | -------------------------------------------------- | ----------------------------------------------------------------- | ---------------------------------------------------------------- |
| Basic value (string, number, boolean, choice from a list) | The writes no other write has replaced                        | The write with the highest stamp: last writer wins | One wins by stamp order; the others stay as conflicts             | Each losing write, with its value, author and stamp              |
| Set                                                       | The additions no removal has seen, and the removals           | The elements with at least one live addition       | An addition wins over a removal that did not see it (add wins)    | Each removal that lost to a concurrent addition                  |
| Link (one target)                                         | A basic value whose value is an object id                     | The winning target, with its resolution status     | As for basic values                                               | As for basic values; plus a target that is deleted or missing    |
| Links (many targets)                                      | A set of object ids                                           | The targets, each with its resolution status       | As for sets                                                       | As for sets; plus each target that is deleted or missing         |

In one sentence: every field's state is a pure function of the set of changes a copy has received, so the order in which they arrive cannot matter; and no change disappears without either a later writer having seen it or the app being able to read that it lost.

## Terms

- **Change.** One signed, encrypted entry in a space's log, as RFC 0002 defines it. A change carries one or more operations; this RFC looks only at the operations on one field of one object.
- **Operation.** What a change does to one field: for example "set the title to X" or "add the tag Y". Its encoding belongs to RFC 0002; its meaning belongs to this RFC.
- **Change id.** The id RFC 0002 gives each change. An operation is identified by its change id plus its position in the change; this RFC writes "the operation's id" for that pair.
- **Stamp.** The ordering metadata of RFC 0002, attached to every change: its 64-bit hybrid logical clock, with 48 bits of milliseconds since the Unix epoch and a 16-bit counter, compared as an unsigned number. The rules below rely only on the properties listed in [What this RFC needs from RFC 0002](#what-this-rfc-needs-from-rfc-0002).
- **Author member id.** RFC 0003's id of the member that signed a change, a hash of its public keys. RFC 0002 carries it in every change's header.
- **Stamp order.** The total order on operations that every member computes the same way: by stamp first, then by author member id, then by operation id. It is the only tie-break in this RFC.
- **Seen.** A member has seen an operation when it applied it to its copy before it made a new change. A later operation can only replace what its author had seen.
- **Concurrent.** Two operations are concurrent when neither author had seen the other's operation.
- **Copy.** One member's folded state of a space.

## What this RFC needs from RFC 0002

RFC 0002 defines the clock: a 64-bit hybrid logical clock in every change's payload, 48 bits of milliseconds and a 16-bit counter. The rules in this RFC depend on that clock only through the properties below, so that a later clock that keeps them needs no change here. RFC 0002 meets each of them; its section "What RFC 0003 and RFC 0004 need from this RFC" says how.

| #   | Property                                                                                                                                                                         | Why this RFC needs it                                                                           | RFC 0002 |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- | -------- |
| O1  | Every change has an id that no two changes share, and every member computes the same id for the same change.                                                                     | Duplicates are recognised by id (idempotence). Set additions use it as their tag.               | Met: the change id is a hash of the signed header; an operation id is the change id plus its position. |
| O2  | The stamp is computed from the change alone, and comparing two stamps gives the same answer on every member and platform, native and WebAssembly.                                | Stamp order must be the same everywhere, or two copies pick different winners.                 | Met: the clock is an unsigned 64-bit integer; the author member id and the operation id are in the same change. |
| O3  | If the author of change b had seen change a before making b, then a's stamp is lower than b's.                                                                                   | A write must win over the writes its author replaced. Without O3 a correction can lose to the value it corrected. | Met: a member's clock is always higher than every clock it has seen. |
| O4  | The stamp carries the author's wall-clock time (as a hybrid logical clock does), so that among concurrent writes the one made later in real time usually wins. Wanted, not required for convergence. | "Last writer wins" should mean roughly what a user expects. A pure counter would let the busiest member win instead. | Met: the upper 48 bits are milliseconds since the Unix epoch. |
| O5  | Every value has exactly one canonical encoding: equal values encode to equal bytes, different values to different bytes.                                                         | Set elements are compared by their canonical bytes.                                              | Met: one encoding per value; NaN has one encoding, and negative and positive zero have one each. |

RFC 0002 confirms that causal delivery is not guaranteed, and this RFC does **not** need it: no field type may assume that a change arrives after the changes its author had seen. The relay hands out changes in log order, but members exchange changes in island mode, logs are compacted, and the convergence harness (NOC-9) delivers in random order with duplicates. Every rule below converges under any order of arrival.

## Basic values

Basic values are strings, numbers (integers and floats), booleans and a choice from a fixed list. All four merge the same way. RFC 0001 promises that "the last writer wins". This RFC keeps that promise for the value an app reads, and adds that the writes which lost stay readable instead of disappearing.

### State and operations

There is one operation:

- **set(value, replaces)**: write `value` to the field. `replaces` is the set of operation ids of the writes the author's copy held as the field's state when it made the change: the current winner and every current conflict. A write to a field nobody has written yet replaces nothing. Clearing an optional field is a set with the empty value.

For a set C of received set operations on one field, the state is:

- `R(C)`, the **replaced ids**: the union of `replaces` over all operations in C.
- `L(C)`, the **live writes**: the operations in C whose id is not in R(C).

The **value** the app reads is the live write that comes last in stamp order. The other live writes are the field's **conflicts**. If no write is live (only possible with malformed operations, see below), the field reads as unset.

Applying one operation w to a state (L, R), as the core does when a change arrives:

1. If w's id is already known (in L or in R, or applied before), stop: the operation is a duplicate.
2. R' = R ∪ w.replaces.
3. L' = (L ∪ {w}) minus every write whose id is in R'.

A write whose id is already in R arrives after the write that replaced it. Step 3 drops it at once, so it never shows up as a conflict. This is why R is kept: without causal delivery, R is how a copy remembers what was replaced before it arrived.

### Why it converges

- **A function of the set.** L(C) and R(C) are defined from the set C alone, by union and filtering. A set has no order and no duplicates, so any order of arrival and any number of repeats of the same operation give the same C, hence the same state.
- **The incremental rule computes that function.** After applying w to (L(C), R(C)), R' is R(C ∪ {w}) by definition of the union, and an operation is in L' exactly when it is in C ∪ {w} and its id is not in R'. That is L(C ∪ {w}). So applying operations one at a time, in any order, lands on the same state as computing it from the set: commutative (any two operations applied in either order give the same state), associative (grouping does not matter, since the result only depends on the union) and idempotent (step 1, and union with an element already present changes nothing).
- **The read is deterministic.** Stamp order is a total order (O1 makes the operation id a final tie-break that is never equal for two different operations), and it is the same on every member (O2). The maximum of the same set under the same total order is the same element.
- **Malformed operations cannot break convergence.** A buggy or hostile member may name ids in `replaces` it never saw, or build a cycle where two writes replace each other. The state is still a function of the set, so every copy agrees; in the worst case (a cycle) no write is live and every copy reads the field as unset. Signatures and membership (RFC 0003) decide who may write at all; a member that may write may already overwrite the field, so a forged `replaces` gives it no new power.
- **Honest operations always leave a live write.** With O3, a write's stamp is higher than every write it replaces. The write with the highest stamp in C is therefore replaced by nothing in C, so L(C) is never empty when C is not.

### Tie-break

Stamp order: compare the stamps of RFC 0002 as unsigned numbers (O2), then the author member id as bytes, then the operation id as bytes. All three are in the change itself, so every member holds the same three values for every operation and orders them the same way. The author member id comes before the operation id so that, for two writes with equal stamps, which one wins does not depend on the hash of their content. Either order converges; see [owner decision 2](#decisions-for-the-owner).

### Late changes stay identifiable

A change that arrives late, for example from a laptop that was offline for a week, was made without seeing the writes others made in the meantime. Its write replaces only what its author had seen. When it arrives:

- If it comes last in stamp order, it wins, and the writes it did not see stay live as conflicts.
- If it does not come last (the usual case, because its stamp is older), it loses, and it stays live as a conflict. It does not disappear.

A conflict ends only when someone writes again having seen it: the new write replaces the winner and every conflict. That is a decision a person or an agent made with the conflict in front of them, so nothing is lost silently.

The app can read, per field (the names are placeholders; the SDK's public names belong to the owner):

- `value`: the winning write's value.
- `conflicts`: the losing live writes, each with its value, author and stamp. Conflicts whose value equals the winner's value are left out of this list, because nothing was lost; they stay in the state and are replaced by the next write.
- Per change, `outcome(change, field)`: one of **won** (it is the value), **lost** (it is a conflict) or **replaced** (a later write by an author who had seen it replaced it).

What "late" means here: the converged state cannot record **when** a change arrived, because arrival differs per member and recording it would make copies diverge. What every copy records the same way is that a write **lost to a write its author had not seen**. That is the late change of principle 3 in a form that converges. A member may additionally show, as local information that is not part of the converged state, that a change arrived after it had already shown a different value; see [Agreement with GENESIS.md and RFC 0001](#agreement-with-genesismd-and-rfc-0001).

### Value rules per kind

The merge is the same for all four kinds. The kind only constrains the value:

- **String.** Any Unicode string, stored as written. The SDK normalises to NFC before writing, so that two members typing the same text produce the same bytes (this matters for sets, not for basic values).
- **Number.** An integer (64-bit signed) or a float (IEEE 754 binary64), as the schema declares. Concurrent writes do not add up: the last writer wins. A counter that adds concurrent increments is a different field type and not part of this RFC. For floats, RFC 0002 gives NaN one canonical encoding and negative and positive zero one each (O5). Negative and positive zero stay different values, so a set may hold both.
- **Boolean.** True or false.
- **Choice from a list.** One of the options the schema lists. A value that the reader's schema does not list (because a newer schema added the option) is kept as written, not dropped and not replaced. The app reads it as "unknown option", with the raw value. This keeps the promise that old clients keep data they do not understand yet.

## Sets

A set field holds an unordered collection of distinct basic values: strings, numbers, booleans or options from a list. RFC 0001 promises that "a set merges additions and removals from every member". This RFC chooses **add wins**: when an addition and a removal of the same element are concurrent, the element stays.

### State and operations

There are two operations:

- **add(e, replaces)**: add element e. The operation's id is the tag of this addition. `replaces` is the set of ids of removals of e that the author's copy held (see "lost removals" below); for a fresh addition it is usually empty.
- **remove(e, observed)**: remove element e. `observed` is the set of tags of the live additions of e in the author's copy. A removal can only remove additions its author had seen.

Elements are compared by the canonical encoding of their value (O5). For a set C of received set operations on one field:

- `T(C)`, the **removed tags**: the union of `observed` over all removals in C.
- `A(C)`, the **live additions**: the additions in C whose tag is not in T(C).
- The **elements** the app reads: the values that have at least one live addition, in ascending order of their canonical bytes, so every copy lists them in the same order.

Applying one operation to a state works as for basic values: skip known ids; a removal adds its `observed` tags to T and drops those additions from A; an addition joins A unless its tag is already in T.

### Why it converges

The argument is the one for basic values. T(C) and A(C) are defined from the set C by union and filtering, so they do not depend on order or repeats, and the incremental rule computes them: an addition is in A after the step exactly when it is in C ∪ {op} and its tag is not in the new T. The element list is sorted by a total order on bytes, so it is the same on every copy. No tie-break by stamp is needed: an addition survives unless a removal names its tag, and a removal can only name tags its author had seen.

### Concurrent addition and removal

Alice removes "urgent" from a task's tags. At the same time, offline, Bob adds "urgent" to the same task. Alice's removal names the tags of the additions she had seen; Bob's new addition has a tag she never saw. Every copy ends with "urgent" in the set. Bob's addition is not lost; Alice's removal did not take effect.

Add wins because a lost removal is cheap to repeat and easy to show, while a lost addition is data that is gone. See [Rejected options](#rejected-options) for remove wins.

### Late changes stay identifiable

An addition never loses: it can only be removed by a removal whose author had seen it. A removal can lose, to an addition of the same element its author had not seen. Such a removal is a **lost removal**: a removal of e, while e is still in the set, that no later addition of e has replaced. When someone adds e again having seen the removal (a deliberate re-add), that addition names the removal in its `replaces`, and the removal no longer counts as lost.

The app can read, per set field: the `elements`; per element, the `lostRemovals` with author and stamp; and per change, `outcome(change, field)` with **applied**, **lost** (a removal that lost to an addition it had not seen) or **replaced**. As for basic values, these names are placeholders.

## Links

A link points to another object in the same space, by its object id (RFC 0002). RFC 0001's example, `dependsOn: nc.links("tasks")`, is a field of many links to objects of the collection `tasks`.

### State and operations

There are two link field types, and neither has merge rules of its own:

- **Link (one target)** is a basic value whose value is an object id. It merges exactly as [Basic values](#basic-values) do: the last writer wins, losing writes stay as conflicts.
- **Links (many targets)** is a set of object ids. It merges exactly as [Sets](#sets) do: add wins, lost removals stay readable.

The convergence argument is therefore the one above. What is new is how a link **resolves**: whether the object it names can be read.

### Resolution

Resolution is not part of the link's state. The link keeps the object id as written, whatever happens to the target. The core works out the status when the app reads the link, from the same copy it reads everything else from:

| Status                | Meaning                                                                                                                                       |
| --------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| **ok**                | The copy holds the target, it is not deleted, and it is in the collection the schema names.                                                  |
| **deleted**           | The copy holds the target and it is deleted.                                                                                                  |
| **missing**           | The copy holds no object with this id. The target may not have arrived yet, may never have been created, or may live in another space (see below). |
| **wrong collection**  | The copy holds the target, but in a collection the schema does not allow for this field.                                                      |

Because the status is a function of the copy, two copies that hold the same set of changes compute the same status. Before they hold the same set, they may differ, as any other value does during sync. Computed fields and views that follow links (RFC 0001's `nc.reach`) will read these statuses; how they treat each status belongs to the computed-fields RFC.

### A link whose target was deleted

The link stays as written, with status **deleted**. It is not removed, because nothing in the copy may be dropped silently, and because a deletion may be undone. If the target is restored, the link reads **ok** again with no change to the link.

This RFC assumes only one thing about object deletion, which belongs to a later RFC: whether an object is deleted is itself a converged value, so every copy with the same changes agrees on it. [Owner decision 4](#decisions-for-the-owner) recommends a rule for it.

### Two creates of one object id

RFC 0002 lets an object be created by a "create" operation (operation 0), and its object ids are 128 random bits, so two honest members never draw the same id. A member can still create one id twice on purpose, or a bug can. Every copy treats two or more creates of one id in one space the same way, consistent with deletion as a converged value (owner decision 4):

- **One object.** The id names one object; it exists once any create of it has arrived. A create neither deletes nor restores: whether the object is deleted stays its own converged value.
- **Its collection.** A create puts the object into a collection, which is a basic value of the object: the create last in stamp order wins, and a create that names another collection is a conflict the app can read, as for any basic value. Creates replace nothing, so a conflict here stays until the object is deliberately moved, which a later RFC defines.
- **Its fields.** Writes to the object's fields merge by their field types, whichever create they followed. A field that the winning collection does not declare is kept in the state, as data the reader does not understand, and is not shown as a field of the object.

Every copy computes this from the set of creates and writes alone, so the rule converges by the argument for basic values, and no create or write is dropped.

### A link to an object the member has not received

The link stays as written, with status **missing**. When the target's changes arrive, the status becomes **ok** (or **deleted**, or **wrong collection**) without any change to the link. A link is never refused or dropped because its target is unknown: in island mode, a member may receive a task's `dependsOn` before the task it depends on, and dropping the link would lose data and make copies diverge.

A link cannot point into another space: the SDK refuses to write one. If a change still names an object of another space (a bug, or a hostile member), the target is never found in this space, so the link reads **missing** forever. A copy cannot tell "not arrived yet" from "will never arrive"; the status says what the copy knows, no more. Members of the other space do not learn anything from this, since the link is encrypted in this space.

## The plugin contract

RFC 0001 lets developers add field types as plugins, and the core runs them next to the built-in ones. A field type that breaks convergence breaks every promise built on it, so the core accepts a field type, built in or plugin, only when it meets this contract. The types above are the first to meet it.

### What a field type defines

- A **state**, with a canonical encoding: equal states encode to equal bytes.
- Its **operations**, with a canonical encoding of their logical content (the change format around them is RFC 0002's).
- An **initial state** for a field nobody has written.
- **apply(state, operation) → state**, which the core calls for every received operation, in whatever order they arrive.
- **read(state) → value**, which is what the app, views and computed fields see.
- **outcome(state, operation id) → won | applied | lost | replaced**: where the operation went (the types above define which apply). Every received operation has exactly one outcome.
- A **type id and rule version**, recorded in `model.lock.json`. A merge rule never changes under the same type id and version: a new rule is a new version, which every member must run before the schema may use it, with a migration (GENESIS.md: "Never break an existing client or stored data").
- **Generators** for the convergence harness: random operations from several simulated members, where each member only replaces what its own simulated copy had seen.
- At least one **broken fixture**: a deliberately wrong variant of the type that the harness must reject, to prove the checks fire for this type (GENESIS.md: "fixtures that prove detectors fire").

### What it must prove, and how the harness checks it

| #   | Property                                                                                                                                                                                                                                                                          | How the convergence harness (NOC-9) checks it                                                                                                              |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| C1  | **Converges in any order.** For any set of operations, every order of arrival yields equal states. This is commutativity and associativity of apply over arbitrary orders, not only over concurrent operations: no type may assume causal delivery.                                 | Generates operations from several members, delivers them to at least three copies in different random orders, and compares the copies' canonical state bytes. |
| C2  | **Idempotent.** Applying an operation that the state already holds changes nothing.                                                                                                                                                                                                | Adds random duplicates to every delivery, and applies the whole set twice to one copy.                                                                      |
| C3  | **Deterministic everywhere.** apply, read and outcome are pure functions: no wall clock, no randomness, no arrival time, no iteration order of a hash map, no platform-dependent float behaviour. Native and WebAssembly builds give identical bytes.                               | Runs the same seed natively and as wasm32 (NOC-10) and compares the state bytes.                                                                            |
| C4  | **Nothing lost silently.** Every operation's outcome is one of: visible in the read value, replaced by an operation whose author had seen it, or lost and readable as lost. All copies give the same outcome for the same operation.                                                | Checks every generated operation's outcome on every copy. The generator knows what each member had seen, so it checks that **replaced** only names operations whose author had seen the replaced one. |
| C5  | **Robust.** Any operation, including malformed ones (unknown ids, cycles, values outside the schema), is applied without a panic, and the state still converges.                                                                                                                   | Mixes arbitrary operations into the generated sets.                                                                                                         |
| C6  | **The checks fire.** The type's broken fixtures fail C1 to C5.                                                                                                                                                                                                                     | Runs every broken fixture and expects a failure; a fixture that passes fails the gate.                                                                      |
| C7  | **Tested reads.** read and outcome are the only way the app sees the state, so they are checked as part of the state: two copies with equal state bytes give equal reads.                                                                                                        | Compares read values and outcomes, not only state bytes.                                                                                                     |

The harness runs in `pnpm gates` and is deterministic for a seed, which it prints on failure. A field type that does not pass it does not ship in the core and is not accepted as a plugin.

What the contract does not check: whether the merge does what users expect, how large the state grows, and whether the type is fast. Those are reviewed by people. The contract only guarantees that every copy agrees and that every change is accounted for.

## Rejected options

| Option                                                                                     | Why not                                                                                                                                                                                 |
| ------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Plain last writer wins, losing writes dropped                                              | A week-old offline edit disappears without a trace. Breaks principle 3 ("nothing is lost silently").                                                                                    |
| A multi-value register as the read value (the app always gets a list of concurrent values) | Every app must handle several values for every field. RFC 0001 promises the last writer wins. We keep the multi-value state underneath and read one winner.                            |
| Wall-clock time alone as the stamp                                                         | Not causal: a member with a slow clock can correct a value and still lose to the value it corrected. O3 rules it out.                                                                    |
| First or last writer by arrival order                                                      | Arrival order differs per member, so copies diverge. This is the broken fixture NOC-9 uses.                                                                                             |
| Recording "arrived late" in the converged state                                            | Arrival differs per member, so copies diverge. Lost-to-a-write-not-seen is the convergent form of "late".                                                                               |
| Requiring causal delivery                                                                  | Members exchange changes in island mode and after compaction in any order, and the harness delivers in random order. A rule that needs causal delivery diverges when it is violated.  |
| Remove wins for sets                                                                        | A removal from a device that was offline for a week silently undoes additions others made meanwhile. A lost removal is cheaper to show and redo than a lost addition.                    |
| Two-phase set (a removed element can never come back)                                       | A tag removed once can never be added again; users expect to re-add.                                                                                                                    |
| Last-writer-wins element set (add and remove compared by stamp)                             | Whether an element survives depends on clocks, and the losing addition is lost silently.                                                                                                |
| Counters for numbers (concurrent increments add up)                                        | A different field type with different semantics; RFC 0001 ships numbers as last writer wins. A counter can come later as its own type.                                                 |
| Dropping links whose target is missing or deleted                                          | The target may arrive later or be restored; dropping loses data and makes copies diverge during sync.                                                                                   |
| Refusing a link to an unknown target (referential integrity)                               | Needs one place that sees every write before it is accepted. NoCentralDB has none (RFC 0001, "What it is not").                                                                        |
| Plugins as code loaded at run time, in any language                                        | Every member must run the identical rule natively and in WebAssembly. Until a plugin RFC says otherwise, a plugin is Rust compiled into the core.                                      |

## Decisions for the owner

GENESIS.md reserves merge rules and public API names for the owner. Each decision below has options and a recommendation. Merging this RFC accepts the recommendations unless the PR records otherwise.

1. **What basic values keep when writes conflict.**
   - A: Last writer wins on read, losing writes stay as conflicts until someone writes having seen them (this RFC). Recommended: keeps RFC 0001's promise and principle 3.
   - B: Plain last writer wins; losing writes stay only in the log and the archive. Simpler state, but the app cannot see the loss.
   - C: Expose every concurrent value (multi-value register). Every app must handle lists.
2. **Tie-break order after the stamp.**
   - A: Author member id, then operation id (this RFC). Recommended: who wins a tie depends on who wrote, not on a hash of the content.
   - B: Operation id only. One comparison fewer; the winner of a tie is effectively random.
3. **Sets: add wins or remove wins.**
   - A: Add wins (this RFC). Recommended: no addition is lost; a lost removal stays readable.
   - B: Remove wins. A late removal undoes additions it never saw.
4. **Object deletion (assumed here, defined in a later RFC).**
   - A: Whether an object is deleted is a basic boolean value with this RFC's rule. A deletion can be undone; edits to a deleted object are kept and become visible again on restore; links read **deleted**. Recommended: nothing is lost, and the rule needs no new proof.
   - B: Deletion is final and wins over everything. Simpler for apps, but a late edit to a deleted object is lost silently.
5. **Links to targets the copy does not hold.**
   - A: Keep the link, read as **missing**, resolve when the target arrives (this RFC). Recommended.
   - B: Hide such links from reads until the target arrives. Same state, but an app cannot show "waiting for data".
6. **No field type may assume causal delivery.**
   - A: Yes (this RFC). Recommended: matches the harness and island mode. Cost: each field keeps the ids that were replaced (R and T above), which grow until compaction.
   - B: RFC 0002 guarantees causal delivery, and field types may rely on it. Smaller state, but one relay or sync bug that reorders changes makes copies diverge.
7. **Placeholder names.** `value`, `conflicts`, `lostRemovals`, `outcome`, and the statuses **ok**, **deleted**, **missing** and **wrong collection** are working names. Recommended: keep them in this RFC and decide the public names in the SDK's API review, not here.

## Agreement with GENESIS.md and RFC 0001

This RFC agrees with `factory/GENESIS.md` and [RFC 0001](0001-nocentraldb.md) on these points:

- **Principle 2, every copy converges.** Every field's state is a function of the set of changes; the contract makes the harness check it for every type, built in or plugin.
- **Principle 3, nothing is lost silently.** Losing writes and lost removals stay in the state and readable; deleted or missing link targets keep their links.
- **Principle 7, one core.** C3 requires identical results natively and in WebAssembly.
- **RFC 0001, field types.** Basic values: the last writer wins. Sets: additions and removals from every member merge. Links: point to another object in the same space. The FAQ's answer "both changes are kept" holds: the losing change stays readable.
- **"A client keeps data it does not understand yet."** A choice outside the reader's list is kept as written.
- **RFC 0002.** It meets O1 to O5, confirms that causal delivery is not guaranteed, and leaves concurrent creates of one object id to this RFC (its conflict point 3), which [Two creates of one object id](#two-creates-of-one-object-id) answers.

It names these conflicts and limits for the owner and pm:

1. **"Late" in principle 3.** GENESIS.md says a change that "arrives late … and loses to a newer change stays identifiable as such". Arrival time cannot be part of the converged state without breaking principle 2, which ranks higher. This RFC makes identifiable what converges: that a change lost to a change its author had not seen. If the owner wants arrival time as well, it can only be local, per-member information, and GENESIS.md should say so.
2. **"Last writer" is the highest stamp, not the last in real time.** With the wall-clock component of O4, a member whose clock runs ahead wins concurrent writes it should lose, until the others' clocks pass it. No member can correct this on its own without diverging. RFC 0002 decides that stamps far in the future are not refused, neither by the writing member, nor by a receiving member, nor by the relay, because refusing them on some members and not on others would break convergence. Members only warn when a received clock is more than a minute ahead of their own. This RFC states the limit (principle 4).
3. **Concurrent increments do not add up.** RFC 0001's `priority: nc.int()` is last writer wins. Two agents that raise a priority by one at the same time raise it by one, not two, and one write shows as a conflict. RFC 0001 does not promise otherwise, but readers may assume it.
4. **A missing link target is ambiguous.** A copy cannot tell a target that has not arrived from one that never will.
5. **State grows.** The replaced ids (R and T) grow with every overwrite and removal. Compaction (a later RFC) must decide when they may be dropped without letting an old, replaced change come back.

## How this RFC is tested

`scripts/rfc-0004.test.mjs` checks that this file exists, links to RFC 0001, has the sections its ticket requires, and that its relative links resolve. The rules themselves are tested when they are built: NOC-16 and NOC-17 implement them test first against the convergence harness of NOC-9, with the contract above.
