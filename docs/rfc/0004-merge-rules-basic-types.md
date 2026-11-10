# RFC 0004: Merge rules for basic values, sets and links

REQUEST FOR COMMENTS · NOVEMBER 2026 · Ticket NOC-8

This RFC defines how the first field types of NoCentralDB merge concurrent changes, so that every copy of a space reaches the same state: basic values, sets and links. It also defines the contract every other field type, built in or plugin, must meet before the core accepts it.

It builds on [RFC 0001](0001-nocentraldb.md), which promises that "the same set of changes produces the same state on every device, in any order of arrival", and that each field merges by the rules of its type. RFC 0001 says what the shipped types do in one line each. This RFC makes those lines exact.

Merge rules are hard to change once a type ships: a changed rule makes old and new clients diverge. So each rule below says why it was chosen and which rules were not.

**Status.** Decided by the owner (Carsten, 2026-11-10).

**How to give feedback.** Reply in this RFC's discussion, [RFC 0004: Merge rules for basic values, sets and links](https://github.com/nocentraldb/nocentraldb/discussions/127), one topic per reply, or open a pull request against this file to change its text. [Giving feedback](README.md#giving-feedback) says how, and what happens to your point.

**Out of scope.** Rich text, streams and personal fields (later RFCs). Computed fields and views. The encoding of changes and of the ordering metadata ([RFC 0002](0002-change-and-log-format.md)); how this RFC's operations sit inside RFC 0002's operation map is [How field operations travel in a change](#how-field-operations-travel-in-a-change). Object deletion, except how a deletion travels ([Creating and deleting an object](#creating-and-deleting-an-object)); [Links](#links) says what this RFC assumes about it. Of object creation, this RFC defines only what two creates of one object id mean.

## Summary

| Type                                                      | State                                                         | Read value                                         | Concurrent writes                                                 | What stays visible to the app                                    |
| --------------------------------------------------------- | ------------------------------------------------------------- | -------------------------------------------------- | ----------------------------------------------------------------- | ---------------------------------------------------------------- |
| Basic value (string, number, boolean, choice from a list) | The writes no other write has replaced                        | The write with the highest stamp: last writer wins | One wins by stamp order; the others stay as conflicts             | Each losing write, with its value, author and stamp              |
| Set                                                       | The additions no removal has seen, and the removals           | The elements with at least one live addition       | An addition wins over a removal that did not see it (add wins)    | Each removal that lost to a concurrent addition                  |
| Link (one target)                                         | A basic value whose value is an object id                     | The winning target, with its resolution status     | As for basic values                                               | As for basic values; plus a target that is deleted or missing    |
| Links (many targets)                                      | A set of object ids                                           | The targets, each with its resolution status       | As for sets                                                       | As for sets; plus each target that is deleted or missing         |
| Reference (one target, in any space)                      | A basic value whose value is a space id and an object id      | The winning reference, with its resolution state   | As for basic values                                               | As for basic values; plus a target that is not accessible or missing |

In one sentence: every field's state is a pure function of the set of changes a copy has received, so the order in which they arrive cannot matter; and no change disappears without either a later writer having seen it or the app being able to read that it lost.

## Terms

- **Change.** One signed, encrypted entry in a space's log, as RFC 0002 defines it. A change carries one or more operations; this RFC looks only at the operations on one field of one object.
- **Operation.** What a change does to one field: for example "set the title to X" or "add the tag Y". The change around it belongs to RFC 0002; its meaning belongs to this RFC, and its encoding to [How field operations travel in a change](#how-field-operations-travel-in-a-change).
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

RFC 0002 confirms that causal delivery is not guaranteed, and this RFC does **not** need it: no field type may assume that a change arrives after the changes its author had seen. The relay hands out changes in log order, but members exchange changes in island mode, logs are compacted, and the convergence harness (NOC-9) delivers in random order with duplicates. Every rule below converges under any order of arrival. No field type may assume causal delivery: a guarantee that field types could rely on would give smaller state, but one relay or sync bug that reorders changes would then make copies diverge. The cost is that each field keeps the ids that were replaced (R and T below), which grow until compaction.

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

Stamp order: compare the stamps of RFC 0002 as unsigned numbers (O2), then the author member id as bytes, then the operation id as bytes. All three are in the change itself, so every member holds the same three values for every operation and orders them the same way. The author member id comes before the operation id so that, for two writes with equal stamps, who wins depends on who wrote, not on the hash of their content. Either order converges. The operation id alone would save one comparison, but the winner of a tie would then be effectively random.

### Late changes stay identifiable

A change that arrives late, for example from a laptop that was offline for a week, was made without seeing the writes others made in the meantime. Its write replaces only what its author had seen. When it arrives:

- If it comes last in stamp order, it wins, and the writes it did not see stay live as conflicts.
- If it does not come last (the usual case, because its stamp is older), it loses, and it stays live as a conflict. It does not disappear.

A conflict ends only when someone writes again having seen it: the new write replaces the winner and every conflict. That is a decision a person or an agent made with the conflict in front of them, so nothing is lost silently.

The app can read, per field (working names, see below):

- `value`: the winning write's value.
- `conflicts`: the losing live writes, each with its value, author and stamp. Conflicts whose value equals the winner's value are left out of this list, because nothing was lost; they stay in the state and are replaced by the next write.
- Per change, `outcome(change, field)`: one of **won** (it is the value), **lost** (it is a conflict) or **replaced** (a later write by an author who had seen it replaced it).

What "late" means here: the converged state cannot record **when** a change arrived, because arrival differs per member and recording it would make copies diverge. What every copy records the same way is that a write **lost to a write its author had not seen**. That is the late change of principle 3 in a form that converges. A member may additionally show, as local information that is not part of the converged state, that a change arrived after it had already shown a different value; see [Agreement with GENESIS.md and RFC 0001](#agreement-with-genesismd-and-rfc-0001).

`value`, `conflicts` and `outcome`, and below `lostRemovals` and the link statuses **ok**, **deleted**, **missing** and **wrong collection**, are working names: the SDK's API review gives the public names, because public names belong to the SDK, where an app meets them ([RFC 0009](0009-sdk-api.md)).

Keeping the losing writes is what keeps RFC 0001's promise and principle 3 together. The rules not taken:

| Rejected | Why |
| --- | --- |
| Plain last writer wins, losing writes kept only in the log and the archive | Simpler state, but the app cannot see the loss: a week-old offline edit disappears without a trace. Breaks principle 3 ("nothing is lost silently"). |
| A multi-value register as the read value (the app always gets a list of concurrent values) | Every app must handle several values for every field. RFC 0001 promises the last writer wins. The multi-value state stays underneath, and the read gives one winner. |
| Wall-clock time alone as the stamp | Not causal: a member with a slow clock can correct a value and still lose to the value it corrected. O3 rules it out. |
| First or last writer by arrival order | Arrival order differs per member, so copies diverge. This is the broken fixture the convergence harness uses. |
| Recording "arrived late" in the converged state | Arrival differs per member, so copies diverge. Lost-to-a-write-not-seen is the convergent form of "late". |

### Value rules per kind

The merge is the same for all four kinds. The kind only constrains the value:

- **String.** Any Unicode string, stored as written. The SDK normalises to NFC before writing, so that two members typing the same text produce the same bytes (this matters for sets, not for basic values).
- **Number.** An integer (64-bit signed) or a float (IEEE 754 binary64), as the schema declares. Concurrent writes do not add up: the last writer wins. A counter that adds concurrent increments is a different field type with different semantics, and not part of this RFC: RFC 0001 ships numbers as last writer wins, and a counter can come later as its own type. For floats, RFC 0002 gives NaN one canonical encoding and negative and positive zero one each (O5). Negative and positive zero stay different values, so a set may hold both.
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

Add wins because a lost removal is cheap to repeat and easy to show, while a lost addition is data that is gone. No addition is lost, and a lost removal stays readable. The rules not taken:

| Rejected | Why |
| --- | --- |
| Remove wins | A removal from a device that was offline for a week silently undoes additions others made meanwhile. A lost removal is cheaper to show and redo than a lost addition. |
| Two-phase set (a removed element can never come back) | A tag removed once can never be added again; users expect to re-add. |
| Last-writer-wins element set (add and remove compared by stamp) | Whether an element survives depends on clocks, and the losing addition is lost silently. |

### Late changes stay identifiable

An addition never loses: it can only be removed by a removal whose author had seen it. A removal can lose, to an addition of the same element its author had not seen. Such a removal is a **lost removal**: a removal of e, while e is still in the set, that no later addition of e has replaced. When someone adds e again having seen the removal (a deliberate re-add), that addition names the removal in its `replaces`, and the removal no longer counts as lost.

The app can read, per set field: the `elements`; per element, the `lostRemovals` with author and stamp; and per change, `outcome(change, field)` with **applied**, **lost** (a removal that lost to an addition it had not seen) or **replaced**. As for basic values, these are working names.

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

This RFC assumes only one thing about object deletion, which belongs to a later RFC: whether an object is deleted is itself a converged value, so every copy with the same changes agrees on it. [Creating and deleting an object](#creating-and-deleting-an-object) gives the rule for it.

### Two creates of one object id

RFC 0002 lets an object be created by a "create" operation (operation 0), and its object ids are 128 random bits, so two honest members never draw the same id. A member can still create one id twice on purpose, or a bug can. Every copy treats two or more creates of one id in one space the same way, consistent with deletion as a converged value ([Creating and deleting an object](#creating-and-deleting-an-object)):

- **One object.** The id names one object; it exists once any create of it has arrived. A create neither deletes nor restores: whether the object is deleted stays its own converged value.
- **Its collection.** A create puts the object into a collection, which is a basic value of the object: the create last in stamp order wins, and a create that names another collection is a conflict the app can read, as for any basic value. Creates replace nothing, so a conflict here stays until the object is deliberately moved, which a later RFC defines.
- **Its fields.** Writes to the object's fields merge by their field types, whichever create they followed. A field that the winning collection does not declare is kept in the state, as data the reader does not understand, and is not shown as a field of the object.

Every copy computes this from the set of creates and writes alone, so the rule converges by the argument for basic values, and no create or write is dropped.

### A link to an object the member has not received

The link stays as written, with status **missing**. When the target's changes arrive, the status becomes **ok** (or **deleted**, or **wrong collection**) without any change to the link. A link is never refused or dropped because its target is unknown: in island mode, a member may receive a task's `dependsOn` before the task it depends on, and dropping the link would lose data and make copies diverge. The link is never lost, and an app can show that data is still arriving. The options not taken:

| Rejected | Why |
| --- | --- |
| Hiding such links from reads until the target arrives | The same state, but an app cannot show "waiting for data". |
| Dropping links whose target is missing or deleted | The target may arrive later or be restored; dropping loses data and makes copies diverge during sync. |
| Refusing a link to an unknown target (referential integrity) | Needs one place that sees every write before it is accepted. NoCentralDB has none (RFC 0001, "What it is not"). |

A link cannot point into another space: the SDK refuses to write one. If a change still names an object of another space (a bug, or a hostile member), the target is never found in this space, so the link reads **missing** forever. A copy cannot tell "not arrived yet" from "will never arrive"; the status says what the copy knows, no more. Members of the other space do not learn anything from this, since the link is encrypted in this space. A reference into another space is a field type of its own, `nc.ref`, which [References](#references) defines.

## References

Rights come from many small spaces (GENESIS.md), so related data often lives in different spaces: customers in one space, the orders that name them in another. A link points only into its own space ([Links](#links)). A **reference** points into any space, and is written `nc.ref("collection")` in the schema ([RFC 0005, "References into another space"](0005-schema-and-model-lock.md#references-into-another-space)). It stores the target's space id and its object id, and merges like a basic value. On read, the SDK resolves it from the member's own copies to one of three states. It can be indexed. Computed fields and views never follow it, so every member of the space computes the same state (principle 2). It gives no access to the target's space ([What this does not do](#what-this-does-not-do)). Rights come from many small spaces, so related data often lives in different spaces, and the SDK should know of the relation instead of every app building it again. The options not taken:

| Rejected | Why |
| --- | --- |
| No new type: the app stores the target's id as a plain value | The SDK would know nothing of the relation, could not say whether the target is readable, and every app would build the same thing again. |
| Cross-space links that computed fields and views follow, for members of both spaces | Two members of the orders' space with different memberships would compute different values from the same changes: principle 2 broken by design. |
| Letting `nc.link` point into another space | It changes what a shipped type's value means, and computed fields follow links (RFC 0001's `nc.reach`), which a reference must never allow. |

### State and operations

A reference is a field type of its own, `ncdb/ref` with rule version 1. It has no merge rule of its own: its state is a basic value's, and its value is a pair, the target's space id and object id. So the one table of [operation numbers](#operation-numbers) stays as it is, the merge rule needs no new proof, and an old client that does not know `ncdb/ref` parks the field's operations as an unknown type ([RFC 0005, "Data from a newer model"](0005-schema-and-model-lock.md#data-from-a-newer-model)) instead of misreading them. Two forms were not taken: `ncdb/link` with a new rule version whose value may name a space, one type for both, because a link's meaning would change under its own name and every reader of links would have to handle both forms; and `ncdb/ref` with a new operation number from 5 to 63, "set a reference", because the type id already says what the value is, so a second number adds nothing.

| Type                                 | State                                                    | Read value                                       | Concurrent writes   | What stays visible to the app                                        |
| ------------------------------------ | -------------------------------------------------------- | ------------------------------------------------ | ------------------- | -------------------------------------------------------------------- |
| Reference (one target, in any space) | A basic value whose value is a space id and an object id | The winning reference, with its resolution state | As for basic values | As for basic values; plus a target that is not accessible or missing |

- **set** (1) writes it, with set's arguments `{1: value, 2: replaces}`, as for every basic value; `null` clears it. It is the type's only operation. Add, remove and delete are not defined on it, so a member parks them with the reason "unknown operation" ([Operations a member does not understand](#operations-a-member-does-not-understand)).
- **It converges by the argument for basic values** ([Why it converges](#why-it-converges)). State, apply, read and outcome are basic values', and the value enters them only where a basic value's does: as a whole, compared by its canonical bytes (O5).
- **One target per field.** A field holds a single reference. A field of many references is RFC 0005's ([References into another space](0005-schema-and-model-lock.md#references-into-another-space)).

### How a reference resolves

As for a link, the state is not part of the reference: the reference keeps the space id and the object id as written, whatever happens to the target. The SDK works out the state when the app reads the reference, from the member's own copies of the spaces it holds. A readable target is **ok** here, the working name links use ([Late changes stay identifiable](#late-changes-stay-identifiable)); RFC 0005 names the states for the app: `"ok"`, `"notAccessible"` and `"missing"` ([References into another space](0005-schema-and-model-lock.md#references-into-another-space)).

| State                | Meaning                                                                                                                                                                                                                                  |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **ok**               | The member holds a copy of the target's space, and that copy holds the target, not deleted, in a collection the schema names for the field.                                                                                             |
| **not accessible**   | The member holds no copy of the target's space: it never joined it, or it left and its rows of the space were deleted ([RFC 0006](0006-local-view.md)).                                                                 |
| **missing**          | The member holds a copy of the target's space, but the copy holds no object with that id, or holds it deleted, or in a collection the schema does not name for the field. An erased target reads missing, as a deleted one does. |

Unlike a link's status, the state differs from member to member. Two members of the orders' space hold the same changes and the same reference, but only one of them may be in the customers' space. So the state is never part of the space's shared state (principle 2): no computed field, view, index or other stored value depends on it, and a computed field or view never follows a reference into its target. The computed-fields RFC must refuse one that tries. A member shows the state as local information only, as principle 3 allows for what differs from member to member. Two members that hold the same copies of the same spaces compute the same state. Keeping the resolution state as part of the field's stored state was not taken for that reason: it differs from member to member, so it can never be shared state (principle 3).

Three states, not four. A target in a collection the schema does not name reads **missing**, and an old client that does not know a target collection added later reads **missing** until it upgrades. A fourth state, **wrong collection**, as links have, would be more precise, but every app would have to handle it.

**Not accessible** means exactly that the member's database holds no copy of the target's space. Resolution then reads what `space.get` on the target's space would read, so a reference never shows more or less than the app could read itself. A stricter rule, not accessible whenever the member is not a current member of the target's space by the control state of its copy, even while it holds rows of it, was not taken: the app can still read that copy through `space.get`, so the reference would hide what the app can read anyway.

The state changes without any change to the reference:

- **The target arrives** in the member's copy of its space: **missing** becomes **ok**.
- **The target is deleted:** **ok** becomes **missing**, and **ok** again if it is restored ([A link whose target was deleted](#a-link-whose-target-was-deleted)).
- **The member joins** the target's space: **not accessible** becomes **missing** until the target arrives, then **ok**. **It leaves**, and its rows of the space are deleted: the state becomes **not accessible**.

A copy cannot tell a target that has not arrived from one that never will, as for links ([A link to an object the member has not received](#a-link-to-an-object-the-member-has-not-received)). The state says what the member's copies hold, no more.

### The value and its encoding

The reference travels as a set (1) in RFC 0002's operation map, exactly as [How field operations travel in a change](#how-field-operations-travel-in-a-change) defines it for basic values. Only the value, argument 1, is new:

```text
arguments (CBOR map in key 5), as for every set
├── 1 value         the reference, or null to clear the field
└── 2 replaces      array of operation ids

reference (CBOR array of two)
├── 0 space id      16-byte byte string (RFC 0002, Space)
└── 1 object id     16-byte byte string (RFC 0002, Object ids)
```

- **Written `[space id, object id]`,** a CBOR array of two 16-byte byte strings, in that order. It has one encoding and takes 35 bytes: plain CBOR, the same form as an [operation id](#the-arguments-of-each-operation). One 32-byte byte string, the space id followed by the object id, would be two bytes smaller but a hand-written layout inside CBOR, which operation ids avoid too. A map, `{1: space id, 2: object id}`, would leave room for more keys at two bytes more, but a reference has nothing more to carry.
- **The field's type is not encoded.** A member finds `ncdb/ref` in its `model.lock.json`, from the collection and field names, as for every field ([Where field operations sit in a change](#where-field-operations-sit-in-a-change)).
- **Depth.** The array sits at level 7, where [the depth count](#the-arguments-of-each-operation) puts the value, and its byte strings at level 8, well inside the 26 levels a value may nest.
- **A value that does not fit,** such as a bare 16-byte object id or an array whose items are not 16-byte byte strings, written by a buggy or hostile member, is applied and kept as written, as every basic value is. It names no object, so it reads **missing**, and `space.inspect` returns it as written. This is what [The arguments of each operation](#the-arguments-of-each-operation) does for a link target that is not 16 bytes, so a link and a reference report the same mistake the same way. Reading it as a value outside the field's kind, `undefined`, with the field named in [RFC 0009](0009-sdk-api.md#get-by-id)'s `$raw`, would tell the app the value is wrong, but links and references would then report the same mistake in two ways.
- **The space id may be the referencing object's own space.** Such a reference resolves as any other, to **ok** or **missing**, because the member holds its own space. Computed fields and views do not follow it either. The schema cannot know where a target will live, and an app that keeps customers and orders in one space today and splits them later keeps its field. Having the SDK refuse it with `invalidValue`, so that the app uses `nc.link`, was not taken: a field could then not hold targets in its own space and in others.

### What the relay and the members see

**The relay sees nothing new.** The reference lives in the payload of a change of the referencing space, which is AEAD ciphertext under that space's key (RFC 0002, The payload). The relay cannot tell a reference from any other value, and cannot link the two spaces through it. A reference adds 35 bytes to the plaintext, as a value of that size does.

**Members of the referencing space** learn the target's space id and object id: that an order refers to some object in some other space, and which, by their random ids. They learn nothing of the target's content, and two references with the same space id tell them only that their targets share a space. Knowing a space id grants nothing: the relay checks membership on every request ([RFC 0007](0007-relay.md)), and a space's content is encrypted under keys only its members hold ([RFC 0003](0003-keys-and-cryptography.md)).

**Members of the target space** learn nothing from a reference, unless they are members of the referencing space too.

### What this does not do

Stated plainly (principle 4):

- **It gives no access.** A reference does not let a member of the orders' space read the customer. **not accessible** says only that this member cannot read the target; it says nothing about whether the target exists.
- **It does not keep the target alive.** Nothing in the target's space knows it is referenced. Deleting or erasing the customer does not touch the orders that name it; they read **missing**. There is no referential integrity across spaces, and no cascade.
- **Missing has several causes.** A target that has not arrived, was never created, was deleted or erased, or is in a collection the schema does not name all read **missing**. The state alone does not tell them apart.
- **It differs from member to member,** and changes without a change to the referencing object. Whether a subscription re-runs when it changes is RFC 0005's ([References into another space](0005-schema-and-model-lock.md#references-into-another-space)).
- **Nothing computed follows it.** "All open orders of customers in Berlin" is no query: the app reads the orders through the index on the reference ([RFC 0005](0005-schema-and-model-lock.md#references-into-another-space)), and the customers in their own space. That is the price of principle 2.
- **A hostile member can write any reference,** including one to a space id and object id it guessed or learned elsewhere. It reads **not accessible** or **missing** for members who cannot read that target, and gives nobody access.

## The plugin contract

RFC 0001 lets developers add field types as plugins, and the core runs them next to the built-in ones. A field type that breaks convergence breaks every promise built on it, so the core accepts a field type, built in or plugin, only when it meets this contract. The types above are the first to meet it. A plugin is Rust compiled into the core, until a plugin RFC says otherwise: plugins as code loaded at run time, in any language, were not taken, because every member must run the identical rule natively and in WebAssembly.

### What a field type defines

- A **state**, with a canonical encoding: equal states encode to equal bytes.
- Its **operations**, with a canonical encoding of their logical content (the change format around them is RFC 0002's).
- An **initial state** for a field nobody has written.
- **apply(state, operation) → state**, which the core calls for every received operation, in whatever order they arrive.
- **read(state) → value**, which is what the app, views and computed fields see.
- **outcome(state, operation id) → won | applied | lost | replaced**: where the operation went (the types above define which apply). Every received operation has exactly one outcome.
- A **type id and rule version**, recorded in `model.lock.json`. A merge rule never changes under the same type id and version: a new rule is a new version, and a field can move to it only through a migration (GENESIS.md: "Never break an existing client or stored data"). Migrations belong to a later RFC; until it exists, a field's rule version cannot change. Whatever that RFC defines, a migration must guarantee that every member switches rules at the same point in the log, a point fixed by the changes themselves and not by when a member upgrades, so that copies holding the same changes converge; a member that does not know the new version parks the changes made under it, as RFC 0002 does for every change it does not understand, until it upgrades.
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

## How field operations travel in a change

[RFC 0002](0002-change-and-log-format.md) defines the operation map of a change (keys 1 to 5) and leaves the operation number (key 4) and the value (key 5) to the field type. This section fills them in for the types of this RFC, and for an object's deletion, a boolean basic value of the object ([Creating and deleting an object](#creating-and-deleting-an-object)). It changes no merge rule: state, apply, read and outcome above stay as they are. Rich text, streams and personal fields are not covered; their RFCs take numbers from the same table.

### Where field operations sit in a change

A field operation is one entry of the payload's operation array (RFC 0002, payload key 4), so it is an RFC 0002 operation map. This section fills in key 4 and key 5 for the types of this RFC:

```text
operation (RFC 0002's map, keys unchanged)
├── 1 object id     16 bytes
├── 2 collection    text, the collection's name in model.lock.json
├── 3 field         text, the field's name in model.lock.json
├── 4 op            uint: the operation number (see Operation numbers)
└── 5 value         the operation's arguments: a CBOR map (see The arguments)

arguments (CBOR map in key 5)
├── 1 value         set: the value written; add and remove: the element;
│                   for links, the target's 16-byte object id
├── 2 replaces      set and add: array of operation ids
└── 3 observed      remove: array of operation ids

operation id (CBOR array of two)
├── 0 change id     32-byte byte string (RFC 0002, Change ids)
└── 1 position      uint, counted from 0 (RFC 0002, Operation ids)
```

- **Key 4** carries the operation's number.
- **Key 5** carries the operation's arguments. RFC 0002 calls key 5 the value and lets the field type define its shape. For the types of this RFC it holds the value together with `replaces` or `observed`, because the merge rules need both. Key 5 is present in every operation of this RFC.
- **The operation's own id is not encoded.** It is the change id plus the operation's position in the array (RFC 0002, Operation ids). The tag of a set addition is that id, so an add carries no tag of its own.
- **The field's type is not encoded either.** A member finds it in its `model.lock.json`, from the collection and field names (keys 2 and 3), and reads the operation number in the context of that type. An operation without a field (key 3 absent) is a create or a delete of the object.

The frame, the header and the rest of the payload do not change. Every byte of this section sits inside the payload, which is encrypted (see [What the relay sees](#what-the-relay-sees)).

The arguments sit in key 5 as a map with small integer keys, because RFC 0002's map then stays as it is, key 5 already belongs to the field type, and the map leaves room for arguments the way RFC 0002's maps do. The layouts not taken:

| Rejected | Why |
| --- | --- |
| New keys in RFC 0002's operation map for `replaces` and `observed` (keys 6 and 7) | Changes RFC 0002's map for every field type to carry two of this RFC's concepts. Key 5 already belongs to the field type, and a core that does not know the new keys would park them as unknown "must understand" keys. |
| The arguments as a positional array, `[value, replaces]` | One byte smaller per argument, but no room for a "may ignore" argument, and a new argument changes what every position means. |

### Operation numbers

| Number    | Operation                     | Field types                                                      | Arguments                             |
| --------- | ----------------------------- | ---------------------------------------------------------------- | ------------------------------------- |
| 0         | create the object (RFC 0002)  | none: it names a collection, not a field                         | none: keys 3 and 5 are absent         |
| 1         | **set**                       | basic value (string, number, boolean, choice), link (one target), reference | `{1: value, 2: replaces}`             |
| 2         | **add**                       | set, links (many targets)                                        | `{1: element, 2: replaces}`           |
| 3         | **remove**                    | set, links (many targets)                                        | `{1: element, 3: observed}`           |
| 4         | **delete**                    | none: it writes whether the object is deleted                    | `{1: true or false, 2: replaces}`; key 3 absent |
| 5 to 63   | reserved                      | later built-in types: rich text, streams, personal fields        | defined by their RFCs                 |
| 64 and up | reserved for plugin types     | each plugin type numbers its own operations                      | defined by the plugin                 |

The rules for the numbers:

- **One table for the built-in types.** A number means at most one operation across all built-in types: add is 2 on a set field and on a links field alike, and 2 is never set on another built-in type. A remove sent to a basic value is therefore an operation the type does not define, and is parked (see [Operations a member does not understand](#operations-a-member-does-not-understand)), instead of being misread as one of the type's own operations. A log or an error can name an operation without the field's type.
- **A number is never reused or redefined.** A retired operation keeps its number reserved.
- **Plugin types start at 64.** A plugin type numbers its operations from 64 up, within its own type id; two plugin types may use the same number, because the field's type in `model.lock.json` tells them apart: its type id and rule version, as [RFC 0005](0005-schema-and-model-lock.md) references them ([How a field type is referenced](0005-schema-and-model-lock.md#how-a-field-type-is-referenced)), such as `ncdb/set` for a built-in type or `acme/counter` for a plugin. A plugin is Rust compiled into the core ([The plugin contract](#the-plugin-contract)).
- **Numbers 0 and 4 belong to the object**, for every collection: 0 is RFC 0002's create, 4 is delete (see [Creating and deleting an object](#creating-and-deleting-an-object)). No field type defines them.

So an operation sent to the wrong type is parked, not misread, and a number names an operation without the field's type. The numberings not taken:

| Rejected | Why |
| --- | --- |
| Numbers per type, each starting at 1 (set 1 on basic values, add 1 on sets) | Fewer reserved numbers, but an operation sent to the wrong type is misread as that type's operation 1 instead of being parked, and a log cannot name an operation without the field's type. |
| Operation names as text (`"set"`, `"add"`) | RFC 0002 makes key 4 an unsigned integer, and text costs bytes in every operation. |

### The arguments of each operation

An argument key means the same in every operation that uses it:

| Key       | Name           | CBOR                                    | Meaning                                                                                                                    |
| --------- | -------------- | --------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| 1         | value, element | any value in RFC 0002's profile         | set: the value written, `null` to clear. add and remove: the element. For links: the target's object id, 16 bytes.        |
| 2         | replaces       | list of operation ids                   | set: the writes the author's copy held as the field's state, the winner and every conflict. add: the lost removals of this element in the author's copy. |
| 3         | observed       | list of operation ids                   | remove: the tags of the live additions of this element in the author's copy.                                              |
| 4 to 63   | reserved       | "must understand"                       | A member that does not know the key parks the operation.                                                                   |
| 64 and up | reserved       | "may ignore"                            | A member that does not know the key applies the operation as if the key were absent. Such a key must not change state, read or outcome. |

Per operation:

- **set** writes `{1: value, 2: replaces}`. Both keys are required. A write to a field nobody has written yet writes an empty `replaces`. Clearing an optional field writes `null` as the value: this is RFC 0004's "set with the empty value".
- **add** writes `{1: element, 2: replaces}`. Both keys are required. A fresh addition writes an empty `replaces`; a deliberate re-add names the lost removals it saw.
- **remove** writes `{1: element, 3: observed}`. Both keys are required. The SDK does not write a removal of an element its copy does not hold, so `observed` is not empty when the SDK writes it. A reader applies an empty `observed` as RFC 0004 says: it names no tag, so it removes nothing.

**An operation id** is a CBOR array of two items: the 32-byte change id as a byte string and the position as an unsigned integer, for example `[h'aaaa…aa', 0]`. It takes 36 bytes while the position is below 24.

**A list of operation ids** (`replaces`, `observed`) is a CBOR array, sorted by the canonical bytes of its items, with no item twice. A change id always has 32 bytes and RFC 0002's integers always take their shortest form, which sorts like the number. So this order is the change id as bytes, then the position as a number: the order RFC 0004's tie-break uses for operation ids. An empty list is written as the empty array (`80`), never left out, so each operation has exactly one encoding.

A reader treats such a list as a set: it accepts a list that is unsorted or names an id twice, and ignores the order and the repeats. The state is a function of the set of ids (see [Why it converges](#why-it-converges)), so convergence holds either way; only the writer is held to the sorted form.

This form is plain CBOR with one encoding per operation, and its sort order is this RFC's tie-break order. The forms not taken:

| Rejected | Why |
| --- | --- |
| An operation id as one byte string, change id followed by the position | Three bytes smaller per id, but a hand-written binary layout inside CBOR, with its own integer encoding: the risk [RFC 0002's encoding](0002-change-and-log-format.md#the-cbor-profile) avoids. |
| An operation id shortened to a prefix of the change id, or hashed | Two operations could share it, and a member could not check a reference by recomputing it. |
| Leaving an empty `replaces` out | Two bytes smaller per first write, but two encodings of the same operation if readers accept both, and a "required key" check that fails on the most common write. |
| Rejecting an unsorted or repeating list | A new failure mode that buys no convergence: the state depends on the set of ids only. |

**The value.** Any value of RFC 0002's profile, inside the depth limit below. Whether it fits the field's kind, such as a text in an integer field, an option the reader's schema does not list, or a link target that is not 16 bytes, is not a question of encoding. The operation is applied, and the field type decides how the read shows the value, as this RFC does for choices ("unknown option") and as NOC-16 does for a value of the wrong kind (kept on arrival). A link target that is not a 16-byte byte string names no object, so it resolves as **missing**.

**A personal value.** The personal fields RFC decides where the sealed personal value sits. RFC 0002 puts it in key 5; with arguments in key 5, the natural place is argument 1, which leaves `replaces` readable to members, not to the relay.

**An operation names only operations of other changes.** The change id is the hash of the signed header, which holds the encrypted payload, so it does not exist yet while the payload is written. An operation therefore cannot name an operation of its own change. The SDK writes the net effect of a transaction on each field instead: at most one set per field, and at most one add or one remove per element of a set field.

- A transaction that writes a field twice encodes only its last write.
- A transaction that adds an element and then removes it encodes only the removal of the additions its copy held before, or nothing.
- A transaction that removes an element and then adds it encodes both: the removal names only earlier tags, so the new addition survives it, as the app expects.

A reader still accepts a change with several operations on one field and applies each by this RFC's rules. Two sets on one field in one change replace nothing of each other; they have the same stamp and author, so the higher position wins the tie-break and the other one is a conflict. This needs no second form of id and nothing a reader must special-case. A bare position in `replaces` or `observed`, naming an operation of the same change, was not taken: the SDK could then write every step of a transaction, but every decoder would need two forms of id, for a case the SDK avoids by writing a transaction's net effect.

**Depth.** RFC 0002's decoder allows 32 levels counted from the frame: the frame at depth 1, the header map at 2, the payload map at 3 ([RFC 0002, "The CBOR profile"](0002-change-and-log-format.md#the-cbor-profile)). Below them sit the operation array (4), the operation map (5), the arguments map (6), the value and the lists of ids (7), an operation id (8), and its change id and position (9). A value therefore nests at most 26 levels, counting itself. A basic value, a set element and a link target are one level. The SDK refuses to write a deeper value. A payload that holds one does not decode after decryption, so nothing of its change is folded; which log state such a change takes is RFC 0002's: it is parked ([RFC 0002, "A payload that decrypts but does not decode"](0002-change-and-log-format.md#a-payload-that-decrypts-but-does-not-decode)).

### Creating and deleting an object

Whether an object is deleted is a basic boolean value of the object, with this RFC's rule for basic values. A deletion can be undone; edits to a deleted object are kept and become visible again on restore; links to it read **deleted**. Nothing is lost, and the rule needs no new proof. Deletion that is final and wins over everything was not taken: simpler for apps, but a late edit to a deleted object would be lost silently. [Two creates of one object id](#two-creates-of-one-object-id) makes the object's collection a basic value fed by its creates. Both travel as operations on the object, with no field (key 3 absent):

- **create** (number 0, RFC 0002) writes `{1: object id, 2: collection, 4: 0}`, without key 5. Its collection (key 2) is the value it writes; it replaces nothing, as RFC 0004 says, so it carries no arguments.
- **delete** (number 4) writes `{1: object id, 2: collection, 4: 4, 5: {1: deleted, 2: replaces}}`. The arguments are set's: `true` deletes the object, `false` restores it, and `replaces` names the deletes of this object, winner and conflicts, that the author's copy held. It merges as a basic value: the last writer wins, and a delete that lost stays readable as a conflict. Before any delete, an object is not deleted.

Rules for delete:

- **It belongs to the object, not to the collection it names.** Key 2 is required by RFC 0002 and holds the collection the author's copy showed for the object. The deletion state is the object's, whichever collection a delete names. A delete that names a collection the member's model does not know is parked like any operation on an unknown collection (RFC 0002, Versioning).
- **A value other than `true` reads as not deleted.** A value of the wrong kind is applied, as for every basic value ("The value" above), and reads as not deleted; this is what NOC-17's object model does.
- **A delete with key 3 present is malformed.** It is parked with the reason "malformed arguments", like a delete without key 5 or without `replaces`.
- **A create with key 5 present is malformed.** It is parked with the reason "malformed arguments", as a delete with key 3 present is. One create then has one encoding, and the rule depends only on the operation's bytes, so every member parks the same creates (principle 2). Ignoring key 5 on a create was not taken: one create would have two encodings, and a later version could change what an old client already folded. A later rule that gives creates arguments takes a new operation number, as every new operation does ([How a field type adds operations](#how-a-field-type-adds-operations)).
- **A create and a delete are independent.** A create neither deletes nor restores, and a delete does not need the create to have arrived: it is kept, and applies to the object once it exists, as a link to a missing target is kept (RFC 0004, Links).

Deletion travels as an operation of its own, with set's arguments: it is the boolean basic value above with no new rule, and a client that does not know deletion parks it as an unknown operation instead of misreading it. The forms not taken:

| Rejected | Why |
| --- | --- |
| Deletion as a set (number 1) on a reserved field name, such as `$deleted` | No new number, and [RFC 0005](0005-schema-and-model-lock.md) already reserves names that start with `$` for the core, so the name is free; but no `model.lock.json` declares the field, so every member would special-case one name in its field lookup, and a member without that case would park a deletion as a write to an unknown field. |
| Deletion as a set (number 1) without a field (key 3 absent) | No new number, but the meaning of set would depend on whether key 3 is there; a client that knows set but not deletion would misread it instead of parking it. |
| Separate delete and restore operations | Two numbers for one boolean value; a restore would need to name the deletes it undoes anyway, which is what `replaces` already does. |

Everything else about deletion, such as who may delete or restore, what an app sees of a deleted object, and how deletion meets compaction and erasure, belongs to a later RFC on deletion.

### Operations a member does not understand

RFC 0002 parks per operation, in step 7 of [What a member does with a change it receives](0002-change-and-log-format.md#what-a-member-does-with-a-change-it-receives). This section adds the cases of an operation on a field the member's model knows:

| The operation                                                                                                                       | The member                                                                       |
| ----------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| has a number the field's type does not define, for example 9 on a basic value, or 3 (remove) on a basic value                       | parks it, with the reason "unknown operation"; the rest of the change is folded  |
| has an argument key from 4 to 63 that the operation does not define                                                                 | parks it, with the reason "unknown argument"                                     |
| has an argument key from 64 up that the operation does not define                                                                   | applies it as if the key were absent                                             |
| has malformed arguments: key 5 missing or not a map, a required key missing, a list that is not an array, or an item of a list that is not `[32-byte byte string, unsigned integer]` | parks it, with the reason "malformed arguments" |
| has a value outside the field's kind                                                                                                | applies it (see "The value" above)                                               |
| names ids it never saw, or forms a cycle with another operation                                                                     | applies it, as [Why it converges](#why-it-converges) says                        |

Each case depends only on the operation's bytes and the member's `model.lock.json`, never on the order of arrival or the time, so two members with the same changes and the same model park the same operations (principle 2). The SDK reports parked operations with their reasons (RFC 0002, Versioning); these three reasons join [RFC 0005](0005-schema-and-model-lock.md)'s unknown collection, unknown field and unknown type ([Data from a newer model](0005-schema-and-model-lock.md#data-from-a-newer-model)), so none is ignored silently (principle 3). A parked operation is folded after an upgrade, in the total order, as if it had just arrived (RFC 0002). Parking is how the fold meets C5 of [the plugin contract](#the-plugin-contract) for an operation it cannot decode: deterministic, without a panic, the same on every copy.

Malformed arguments are parked because that needs no new state, is deterministic on every member, and applies nothing the reader had to guess. The options not taken:

| Rejected | Why |
| --- | --- |
| Reading malformed arguments leniently, for example a missing `replaces` as empty | Converges, but a write without `replaces` would silently turn into a conflict, and each version of the core could be lenient in other ways. |
| Rejecting the whole change for one malformed operation | Rejected is for RFC 0002's checks (signature, membership, key), which cover the whole change. One bad operation would hide its change's other operations from every member. |

**What this does not do.** A malformed operation stays parked on every version, because no upgrade can read it. And while an old client parks an operation that a newer client applies, the two show different values for that field: a set the old client parks also leaves the writes it replaced live as conflicts there. This is RFC 0002's reading of principle 2 for parked data ([Versioning](0002-change-and-log-format.md#versioning-old-clients-keep-what-they-do-not-understand)): every member that understands all the operations it holds ends in the same state.

### How a field type adds operations

- **A new operation is a new rule version.** A type's operations are part of its merge rule, which is fixed by its type id and rule version ([The plugin contract](#the-plugin-contract)). A new operation changes what apply does, so it never joins a type under the same version.
- **Until the migrations RFC exists, existing fields keep their operations.** [RFC 0005](0005-schema-and-model-lock.md) lets a new field use a new rule version at once, and refuses to move an existing field to one until migrations exist ([The additive-change rule](0005-schema-and-model-lock.md#the-additive-change-rule)). So a new operation reaches only fields declared with the new rule version, or with a new field type. For example, a counter whose increments add up is its own type, with its own operation number from the table above, and not an operation added to integers.
- **A new operation takes a number never used before.** An old client that meets it parks it ("unknown operation"), keeps it, forwards it, reports it and folds it after the upgrade. It never misreads it as an older operation, because no number is reused.
- **A new argument follows the same rule.** An argument that changes state, read or outcome takes a key from 4 to 63 and comes with a new rule version; an old client parks the operation. A key from 64 up is only for information that changes none of them, which an old client may drop without diverging.

Adding operations to a type under the same rule version, as long as existing ones keep their meaning, was not taken: it is faster to extend, but an old client would then park what a new client applies under the same version, and this RFC fixes a merge rule by its type id and version.

### Byte-exact examples

Each example is one operation map, as it sits in the payload's operation array, first in CBOR diagnostic notation and then as hex. The ids are fixed so that the bytes are easy to read:

- object `T1`: 16 bytes of `11`; link target `T2`: 16 bytes of `22`;
- change ids `A`: 32 bytes of `aa`, and `B`: 32 bytes of `bb`;
- change id `A` also names the delete in E14;
- the collection `tasks` with the fields `title` (string), `priority` (integer), `tags` (set of strings), `parent` (link) and `dependsOn` (links).

The bytes are written by [`vectors/0004/generate.py`](vectors/0004/generate.py) from the descriptions below, with Python's cbor2 6.2.1 in canonical mode, into `vectors/0004/E1.hex` to `E14.hex`. `scripts/rfc-0004-examples.test.mjs` checks, in `pnpm gates`, that each example's hex below decodes under RFC 0002's profile with a strict reader of its own, encodes again to the same bytes, and equals the committed vector. The bytes were also decoded and re-encoded to the same bytes by `ncdb_core::cbor`, outside the gate. NOC-16 (basic values and the link) and NOC-17 (sets and links) build their encoders and decoders from E1 to E11, and NOC-39 the fold of creates and deletions from E12 to E14; each commits them as test vectors.

**E1. set, the first write to a string.** `title` becomes "Ship v1"; nothing is replaced.

```text
{1: h'11111111111111111111111111111111', 2: "tasks", 3: "title", 4: 1, 5: {1: "Ship v1", 2: []}}
```

```text
a501501111111111111111111111111111111102657461736b7303657469746c
65040105a20167536869702076310280
```

**E2. set, an integer that replaces a winner and a conflict.** `priority` becomes 3, replacing operations `[A, 0]` and `[B, 1]`, sorted by their bytes.

```text
{1: h'1111…11', 2: "tasks", 3: "priority", 4: 1,
 5: {1: 3, 2: [[h'aaaa…aa', 0], [h'bbbb…bb', 1]]}}
```

```text
a501501111111111111111111111111111111102657461736b7303687072696f
72697479040105a201030282825820aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa00825820bbbbbbbbbbbbbbbbbbbbbbbbbb
bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb01
```

**E3. set, clearing a field.** `title` is cleared with `null`, replacing `[A, 0]`.

```text
{1: h'1111…11', 2: "tasks", 3: "title", 4: 1, 5: {1: null, 2: [[h'aaaa…aa', 0]]}}
```

```text
a501501111111111111111111111111111111102657461736b7303657469746c
65040105a201f60281825820aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
aaaaaaaaaaaaaaaaaaaaaaaa00
```

**E4. add, a fresh addition to a set.** "urgent" is added to `tags`; its tag is this operation's own id.

```text
{1: h'1111…11', 2: "tasks", 3: "tags", 4: 2, 5: {1: "urgent", 2: []}}
```

```text
a501501111111111111111111111111111111102657461736b73036474616773
040205a20166757267656e740280
```

**E5. remove, from a set.** "urgent" is removed; the author's copy held one live addition of it, tagged `[A, 2]`.

```text
{1: h'1111…11', 2: "tasks", 3: "tags", 4: 3, 5: {1: "urgent", 3: [[h'aaaa…aa', 2]]}}
```

```text
a501501111111111111111111111111111111102657461736b73036474616773
040305a20166757267656e740381825820aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa02
```

**E6. add, a deliberate re-add.** "urgent" is added again by an author who saw the lost removal `[B, 0]`, which no longer counts as lost.

```text
{1: h'1111…11', 2: "tasks", 3: "tags", 4: 2, 5: {1: "urgent", 2: [[h'bbbb…bb', 0]]}}
```

```text
a501501111111111111111111111111111111102657461736b73036474616773
040205a20166757267656e740281825820bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb00
```

**E7. set, a link to one target.** `parent` points to `T2`; nothing is replaced.

```text
{1: h'1111…11', 2: "tasks", 3: "parent", 4: 1, 5: {1: h'22222222222222222222222222222222', 2: []}}
```

```text
a501501111111111111111111111111111111102657461736b73036670617265
6e74040105a20150222222222222222222222222222222220280
```

**E8. add, a link to one more target.** `T2` is added to `dependsOn`.

```text
{1: h'1111…11', 2: "tasks", 3: "dependsOn", 4: 2, 5: {1: h'2222…22', 2: []}}
```

```text
a501501111111111111111111111111111111102657461736b73036964657065
6e64734f6e040205a20150222222222222222222222222222222220280
```

**E9. remove, a link target.** `T2` is removed from `dependsOn`; the author's copy held its addition `[A, 1]`.

```text
{1: h'1111…11', 2: "tasks", 3: "dependsOn", 4: 3, 5: {1: h'2222…22', 3: [[h'aaaa…aa', 1]]}}
```

```text
a501501111111111111111111111111111111102657461736b73036964657065
6e64734f6e040305a20150222222222222222222222222222222220381825820
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
01
```

**E10. An operation number the type does not define.** E1 with number 9. A member parks it with the reason "unknown operation" and folds the rest of its change.

```text
{1: h'1111…11', 2: "tasks", 3: "title", 4: 9, 5: {1: "Ship v1", 2: []}}
```

```text
a501501111111111111111111111111111111102657461736b7303657469746c
65040905a20167536869702076310280
```

**E11. A "may ignore" argument.** E1 with argument 64 set to "hint". A member that does not know key 64 applies it exactly as E1.

```text
{1: h'1111…11', 2: "tasks", 3: "title", 4: 1, 5: {1: "Ship v1", 2: [], 64: "hint"}}
```

```text
a501501111111111111111111111111111111102657461736b7303657469746c
65040105a3016753686970207631028018406468696e74
```

**E12. create, an object.** `T1` is created in `tasks`. No field, no arguments.

```text
{1: h'1111…11', 2: "tasks", 4: 0}
```

```text
a301501111111111111111111111111111111102657461736b730400
```

**E13. delete, an object.** `T1` is deleted; no delete of it was held before.

```text
{1: h'1111…11', 2: "tasks", 4: 4, 5: {1: true, 2: []}}
```

```text
a401501111111111111111111111111111111102657461736b73040405a201f5
0280
```

**E14. delete, a restore.** `T1` is restored with `false`, replacing the delete `[A, 3]`.

```text
{1: h'1111…11', 2: "tasks", 4: 4, 5: {1: false, 2: [[h'aaaa…aa', 3]]}}
```

```text
a401501111111111111111111111111111111102657461736b73040405a201f4
0281825820aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
aaaaaaaaaa03
```

**References.** E15 to E17 use the ids above, with three more: the space `S`, 16 bytes of `33`, and the customers `C1`, 16 bytes of `44`, and `C2`, 16 bytes of `55`, both in `S`. The object `T1` is an order in the collection `orders`, whose field `customer` is `nc.ref("customers")`.

The same generator, [`vectors/0004/generate.py`](vectors/0004/generate.py), writes them into `vectors/0004/E15.hex` to `E17.hex`, and `scripts/rfc-0004-examples.test.mjs` checks them in `pnpm gates` as it checks E1 to E14. They follow [References](#references): the type `ncdb/ref`, written by set, and its value `[space id, object id]`.

**E15. set, the first write to a reference.** `customer` points to `C1` in `S`; nothing is replaced.

```text
{1: h'1111…11', 2: "orders", 3: "customer", 4: 1,
 5: {1: [h'33333333333333333333333333333333', h'44444444444444444444444444444444'], 2: []}}
```

```text
a501501111111111111111111111111111111102666f72646572730368637573
746f6d6572040105a20182503333333333333333333333333333333350444444
444444444444444444444444440280
```

**E16. set, a reference that replaces a winner and a conflict.** `customer` points to `C2` in `S`, replacing `[A, 0]` and `[B, 1]`, sorted by their bytes.

```text
{1: h'1111…11', 2: "orders", 3: "customer", 4: 1,
 5: {1: [h'3333…33', h'5555…55'], 2: [[h'aaaa…aa', 0], [h'bbbb…bb', 1]]}}
```

```text
a501501111111111111111111111111111111102666f72646572730368637573
746f6d6572040105a20182503333333333333333333333333333333350555555
555555555555555555555555550282825820aaaaaaaaaaaaaaaaaaaaaaaaaaaa
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa00825820bbbbbbbbbbbbbbbbbbbb
bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb01
```

**E17. set, clearing a reference.** `customer` is cleared with `null`, replacing `[A, 4]`.

```text
{1: h'1111…11', 2: "orders", 3: "customer", 4: 1, 5: {1: null, 2: [[h'aaaa…aa', 4]]}}
```

```text
a501501111111111111111111111111111111102666f72646572730368637573
746f6d6572040105a201f60281825820aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa04
```

### What the relay sees

Nothing changes for the relay. Field operations, their numbers, values, `replaces` and `observed` live only inside the payload, which is AEAD ciphertext (RFC 0002, The payload). The header, the frame and control changes are untouched, and the relay still sees exactly what [RFC 0002 lists](0002-change-and-log-format.md#what-the-relay-can-see-and-what-it-cannot).

One effect is stated plainly (principle 4): `replaces` and `observed` make the plaintext larger, by about 37 bytes per named operation, so a write that resolves many conflicts produces a larger change. The relay sees the size of the ciphertext, as it does for every change; padding to a multiple of 256 bytes ([RFC 0002, "Padding"](0002-change-and-log-format.md#padding)) blurs it but does not hide it. Inside the space, the lists show members which writes an author had seen, which they already learn from its dependencies.

## Agreement with GENESIS.md and RFC 0001

This RFC agrees with `factory/GENESIS.md` and [RFC 0001](0001-nocentraldb.md) on these points:

- **Principle 2, every copy converges.** Every field's state is a function of the set of changes; the contract makes the harness check it for every type, built in or plugin.
- **Principle 3, nothing is lost silently.** Losing writes and lost removals stay in the state and readable; deleted or missing link targets keep their links.
- **Principle 7, one core.** C3 requires identical results natively and in WebAssembly.
- **RFC 0001, field types.** Basic values: the last writer wins. Sets: additions and removals from every member merge. Links: point to another object in the same space. The FAQ's answer "both changes are kept" holds: the losing change stays readable.
- **"A client keeps data it does not understand yet."** A choice outside the reader's list is kept as written.
- **RFC 0002.** It meets O1 to O5, confirms that causal delivery is not guaranteed, and leaves concurrent creates of one object id to this RFC, which [Two creates of one object id](#two-creates-of-one-object-id) answers.
- **"Late" in principle 3.** GENESIS.md says a change that loses to a change its author had not seen stays identifiable as such. Arrival time cannot be part of the converged state without breaking principle 2, which ranks higher, so this RFC makes identifiable what converges: that a change lost to a change its author had not seen. Arrival time can only be local, per-member information.

These rules have limits, stated plainly (principle 4):

1. **"Last writer" is the highest stamp, not the last in real time.** With the wall-clock component of O4, a member whose clock runs ahead wins concurrent writes it should lose, until the others' clocks pass it. No member can correct this on its own without diverging. Stamps far in the future are not refused, neither by the writing member, nor by a receiving member, nor by the relay, because refusing them on some members and not on others would break convergence ([RFC 0002, "A late change stays identifiable"](0002-change-and-log-format.md#a-late-change-stays-identifiable)). Members only warn when a received clock is more than a minute ahead of their own.
2. **Concurrent increments do not add up.** RFC 0001's `priority: nc.int()` is last writer wins. Two agents that raise a priority by one at the same time raise it by one, not two, and one write shows as a conflict. RFC 0001 does not promise otherwise, but readers may assume it.
3. **A missing link target is ambiguous.** A copy cannot tell a target that has not arrived from one that never will.
4. **State grows.** The replaced ids (R and T) grow with every overwrite and removal. Compaction (a later RFC) must decide when they may be dropped without letting an old, replaced change come back.

## How this RFC is tested

`scripts/rfc-0004.test.mjs` checks that this file exists, links to RFC 0001 and RFC 0002, has the sections its ticket requires, and that its relative links resolve. `scripts/rfc-0004-complete.test.mjs` checks the rules of this text where they stand, with the reason and the options not taken for each, and `scripts/rfc-0004-examples.test.mjs` checks the bytes of every example, see [Byte-exact examples](#byte-exact-examples). The rules themselves are tested when they are built: NOC-16 (basic values) and NOC-17 (sets and links) implement them test first against the convergence harness of NOC-9, with the contract above.

## History

- 2026-11-10 · First published (RFC 0001 to RFC 0010 rewritten as fresh texts, NOC-165)
