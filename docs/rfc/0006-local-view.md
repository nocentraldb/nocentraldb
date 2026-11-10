# RFC 0006: The local view on SQLite, natively and in the browser

REQUEST FOR COMMENTS · NOVEMBER 2026 · Ticket NOC-12

**Changelog.**

- **2026-11-08** · NOC-44 · [Amendment 1: following RFC 0008](#amendment-1-following-rfc-0008) brings this RFC in line with the owner's decisions in [RFC 0008](0008-membership.md): the control table holds RFC 0008's control state, control changes are recomputed through `resolve`, and a shred destroys the key only once it is stable. It decides nothing new. The sentences it changes elsewhere are marked *(Amendment 1)*.
- **2026-11-09** · NOC-34 · [Amendment 2: durability of a browser copy](#amendment-2-durability-of-a-browser-copy) follows what [NOC-33 measured](../browser-storage.md) about how long a browser keeps a web app's copy, and proposes how the SDK, the developer page [Browser apps and durability](../browser-durability.md) and the text for end users tell developers and users how reliable a browser copy is. The owner decided its decisions 10 to 15 as recommended (Carsten, 2026-11-09). The sentences it changes elsewhere are marked *(Amendment 2)*.
- **2026-11-09** · NOC-35 · [Amendment 3: the device checks as findings rows](#amendment-3-the-device-checks-as-findings-rows) follows the owner's answer that the first Firefox private window was closed: what it held is gone after the window closes, and whether it lasts within one open session is unknown (NOC-58). The device checks are now cited by their rows in [NOC-33's findings](../browser-storage.md#findings). It decides nothing. The sentences it changes are marked *(Amendment 3)*.
- **2026-11-09** · NOC-58 · [Amendment 4: a reload in a private window](#amendment-4-a-reload-in-a-private-window) follows NOC-58's measurement: IndexedDB, and the member key in it, survives a reload in a private window in Playwright's Chromium, Firefox and WebKit. It updates the facts in the private-window finding and in decision 12's recommendation, and records that the owner keeps decision 12 at A. It decides nothing. The sentences it changes are marked *(Amendment 4)*.
- **2026-11-09** · NOC-72 · [Amendment 5: the cursor and the plan check](#amendment-5-the-cursor-and-the-plan-check) follows the view that #81 (NOC-69) built: a page after a cursor is written key by key, and the check on open accepts a `SEARCH` or `SCAN` on the declared index. The owner decided it by merging #81 (Carsten, 2026-11-09). The sentences it changes are marked *(Amendment 5)*.
- **2026-11-09** · NOC-75 · [Amendment 6: following RFC 0009](#amendment-6-following-rfc-0009) follows [RFC 0009](0009-sdk-api.md), which the owner decided: an index declared with `scope: "space"` puts the space id first, and no declared index holds a deleted object. It decides nothing. The sentences it changes are marked *(Amendment 6)*.
- **2026-11-10** · NOC-92 · [Amendment 7: following RFC 0005's references](#amendment-7-following-rfc-0005s-references) follows [RFC 0005's D19](0005-schema-and-model-lock.md#decisions-for-the-owner-d18-to-d22), which the owner decided: a reference indexes as one 32-byte `BLOB` column, the space id followed by the object id, compared by its bytes, and an unset reference or a value that does not fit indexes as unset. It decides nothing. The sentences it changes are marked *(Amendment 7)*.

This RFC defines how the core turns a space's log into the local view an app reads: how changes are folded into SQLite tables as they arrive, how the view is rebuilt from the log, how a query runs only through declared indexes and pages with a cursor, and how the same Rust code does all of this natively and in a web browser with the data stored in the browser.

It builds on [RFC 0001](0001-nocentraldb.md), which promises that every member "folds [the log] locally into an SQLite view" and that "reads never leave the device". [RFC 0002](0002-change-and-log-format.md) defines the log and keeps it in the same SQLite database as the view. [RFC 0003](0003-keys-and-cryptography.md) defines the keys that decrypt a change before it is folded. RFC 0004 (NOC-8) defines how a field merges. RFC 0005 (NOC-11) defines the schema and `model.lock.json`, which declares the collections, fields and indexes this RFC turns into tables; where and how the lock declares an index is RFC 0005's to define. This RFC uses those documents only through the properties it names.

**How to give feedback.** This RFC has no discussion of its own yet: start one in the category [RFC feedback](https://github.com/nocentraldb/nocentraldb/discussions/categories/rfc-feedback). [Giving feedback](README.md#giving-feedback) says how, and what happens to your feedback.

**Status.** Decided. The layout of the view on disk is an on-disk format, which GENESIS.md reserves for the owner. The owner accepted the recommendation of every decision in [Decisions for the owner](#decisions-for-the-owner), and this RFC was merged with the owner's approval in #36 (Carsten, 2026-11-08). The design below describes the decided options. The view on SQLite is not built yet. The owner decided Amendment 2, decisions 10 to 15, as recommended (Carsten, 2026-11-09).

**Out of scope.** Computed fields and views (a later RFC); this RFC only names where they plug into the fold. Encryption at rest on the device, beyond naming the question for the owner ([decision 6](#decisions-for-the-owner)). Compaction and snapshots (a later RFC). The sync protocol with the relay (RFC 0007, NOC-13).

## Summary

| Question                                   | Answer                                                                                                                                                                      |
| ------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Where does the view live?                  | In one SQLite database per member on a device, next to that member's log (RFC 0002). The log is the source of truth; the view is a cache that can always be rebuilt.         |
| How do changes get in?                     | The fold: append the frame, decrypt it in memory, apply each operation through its field type, write the read value into a table row, all in one SQLite transaction.        |
| How is the view rebuilt?                   | From the frames in the log, into fresh tables, swapped in by one transaction. Triggered by a model change (a new rule version included), a new view format, or a damaged view. |
| How does an app query?                     | Only through an index the model declares. The core generates the SQL; there is no raw SQL for apps. Every generated query is checked to use its index.                       |
| How does an app page?                      | Keyset pagination: the cursor is the index key of the last row, and the next page is "the rows after this key", in index order.                                             |
| Which SQLite natively?                     | `rusqlite` with SQLite compiled from source (`bundled`), pinned, on desktop and mobile.                                                                                    |
| Which SQLite in the browser?               | The same `rusqlite`, backed by `sqlite-wasm-rs` (SQLite compiled to `wasm32-unknown-unknown`) and the `opfs-sahpool` VFS of `sqlite-wasm-vfs`, in a dedicated Web Worker.   |
| Does it work in a browser?                 | Yes in Chromium, shown by [the spike](#the-spike). The official build's `opfs-sahpool`, which `sqlite-wasm-vfs` ports to Rust, also works in Firefox and in Safari on iOS and macOS ([Amendment 2](#amendment-2-durability-of-a-browser-copy)); the Rust port has run only in Chromium. *(Amendment 2.)* |
| What is in plaintext on the device?        | Every decrypted field value of every space the member belongs to, its merge state and its indexes. Keys are not in the database. See [what that means for a lost device](#what-the-view-stores-in-plaintext). |
| Can a read wait for the network?           | No. The read path has no network code in it.                                                                                                                                |

## Terms

- **Log.** A member's append-only record of the changes of its spaces, as exact frames, in the table `log` (RFC 0002, "The log on disk").
- **Frame.** The exact bytes of one change: header, ciphertext, signature (RFC 0002).
- **Fold.** Applying changes from the log to the view. The fold is the only writer of the view.
- **View.** Everything in the database except the log: one table per collection, the field types' merge state, indexes, and the bookkeeping this RFC defines. The app reads only the view (GENESIS.md, principle 6).
- **Read value.** What a field type says an app reads for a field, given its merge state (RFC 0004: for a basic value, the winning write).
- **Merge state.** What a field type keeps to compute the read value and stay convergent (RFC 0004: for a basic value, the live writes and the replaced ids).
- **Control change.** A change of kind 1 in RFC 0002, with a body [RFC 0008](0008-membership.md#the-control-bodies) defines: the genesis, a membership entry, a key envelope, a key chain entry, a person-key entry, a shred entry or an acknowledgement. It carries no content. *(Amendment 1.)*
- **Rejected.** A change kept in the log but not folded because it failed a check: its signature, its author's membership at its key generation, or its decryption. The log state and its reasons are defined in [RFC 0002, Amendment 1](0002-change-and-log-format.md#rejected-changes), decision D11. A rejected change **waits** for a control change of its space that may let it pass.
- **Declared index.** An index declared in `model.lock.json`. RFC 0005 defines the declaration. This RFC needs only three things from it: the collection, the ordered list of fields with a direction each, and which leading fields a query must fix with equality.
- **Cursor.** In this RFC, a **page cursor**: the position after the last row of a page in a declared index. Not to be confused with the relay cursor of RFC 0002, which this RFC calls the **relay position**.
- **Target.** A platform the core runs on: native desktop (macOS, Linux, Windows), native mobile (iOS, Android), or a browser (Chromium-based, Firefox, Safari).

## Where the view lives

Each member on a device has one SQLite database. It holds the log of every space the member belongs to (RFC 0002) and the view folded from it. A device with two members, for example a person and an agent that runs as its own member, has two databases: the agent's database holds only the agent's spaces, so the agent never sees more than its spaces (GENESIS.md, "Never give an agent more than its spaces").

One database per member, rather than one per space, lets a declared index span all of a member's spaces: "my next 50 tasks across every project" is one range lookup, not one per space. Every row carries its space id, so leaving a space deletes the rows of that space. This is [decision 1](#decisions-for-the-owner).

The database contains these kinds of tables. The names are illustrative; the exact layout is not a stable format (see [decision 2](#decisions-for-the-owner)).

```text
log                 RFC 0002: the frames, the source of truth
meta                view format version, model hash, per space the relay position
c_<collection>      one row per object: space id, object id, one column per field's read value
fs_<collection>     merge state per object and field, as the field type encodes it (opaque bytes)
ix_<collection>_*   one SQLite index per declared index on c_<collection>
control             per space: RFC 0008's control state, and which key generations are held
parked, gaps        RFC 0002: changes stored but not folded, and dependencies not yet received
```

- **`c_<collection>`** has the primary key `(space id, object id)`. A field's column holds its read value in an SQLite type the field type names: `INTEGER`, `REAL`, `TEXT` (UTF-8) or `BLOB`. A field whose type has no single value that can be sorted (rich text, a set) has a column only if its type defines a sortable projection, such as the number of elements; otherwise the read value is served from `fs_<collection>`.
- **`fs_<collection>`** holds, per `(space id, object id, field)`, the field type's merge state, encoded by the field type itself. The core does not interpret it; this is what lets a plugin field type keep any state it needs (RFC 0004, "The plugin contract"). Losing writes that the app can read (RFC 0004's conflicts, RFC 0002's concurrent losers) live in this state, so they survive a rebuild like everything else.
- **`meta`** lets the core decide on open whether the view still matches the code and the model ([Rebuilding the view](#rebuilding-the-view)).

## Folding

The fold turns a frame in the log into rows of the view. A member folds every change the moment it is appended, whether it came from the relay, from a peer in island mode, or from the member's own write. The same code path handles all three.

### One change

Every change goes through the seven steps of [RFC 0002, Amendment 1, "What a member does with a change it receives"](0002-change-and-log-format.md#what-a-member-does-with-a-change-it-receives), in that order and in one SQLite transaction. That section defines the steps and the state each one leaves a change in; this RFC says what the view does at each:

1. **Decode and append** (Amendment step 1). A frame that is not well-formed or not canonical is not appended. Otherwise insert the frame into `log` ([RFC 0002](0002-change-and-log-format.md#the-log-on-disk)). If the change id is already there, stop: a duplicate changes nothing.
2. **Check and decrypt** (Amendment steps 2 to 6). Understand the header, verify the signature, check that the author is a member at the change's key generation, decrypt the payload in memory, understand the payload. The checks use the control state this member holds now ([Control changes and changes that wait for them](#control-changes-and-changes-that-wait-for-them)). A change that stops here changes nothing in the view:
   - An unknown suite, kind, header format or payload format leaves it **parked**, as before.
   - An unknown author, a bad signature, an author who is not a member, a missing key envelope ("no key") or a payload that does not decrypt leaves it **rejected**, with that reason (D11). A missing key envelope is "rejected, no key", not parked.
   - A control change stops after the membership check: it has no payload to decrypt, and it folds as the next section says.
3. **Fold, operation by operation** (Amendment step 7, D10). For each operation, in the order of the change:
   1. Look up the collection and field in the member's `model.lock.json`. If either is unknown, or the operation carries an unknown "must understand" key, park that operation alone and go on with the next.
   2. Load the field's merge state from `fs_<collection>` (empty for a new object).
   3. Call the field type's apply function with the state and the operation; it returns the new state (RFC 0004, "The plugin contract").
   4. Write the new state back, and write the field type's read value into the field's column of `c_<collection>`.
4. **Derive.** Update computed fields and views that depend on the rows this change touched. This is the plug-in point for the later RFC on computed fields and views; until it exists, the step is empty. It runs inside the same transaction, so a reader never sees a row without its computed fields.
5. **Mark** the log entry as folded and commit.
6. **Notify.** After the commit, hand the set of touched `(collection, space id, object id)` to the subscriptions, which re-run the queries that read those rows. Notifications carry ids, never values.

The plaintext from step 2 exists only in memory and in the columns and state written in step 3. It is never written to a log, an error or telemetry ([RFC 0003, "Keys never reach logs, errors or telemetry"](0003-keys-and-cryptography.md#keys-never-reach-logs-errors-or-telemetry)).

Because appending and folding are one transaction, the log and the view agree after a crash: a change is in both or in neither (RFC 0002).

### Control changes and changes that wait for them

Some changes carry no content: the control changes of RFC 0002 (kind 1), whose bodies RFC 0003 defines. They decide whether other changes can be checked and decrypted, so the fold handles them first-class, and the order in which they arrive must not change the result.

**The control state.** The view keeps a `control` table set per space that holds RFC 0008's control state: the genesis; each member's public keys, certificate and role; its accepted grants with their from generations and its accepted revocations with their cutoffs; the generation graph, with each generation's previous generations and envelopes; the person-key entries, with each person key's commitment and newest record; and the shred entries and their acknowledgements. It also records which key generations this member holds an envelope for ([RFC 0008, "The function"](0008-membership.md#the-function)). *(Amendment 1.)* Key material itself is never in the database; the SDK keeps it in the platform keystore ([RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys)). Like every field's merge state, the control state is a function of the set of control changes the member holds: it is RFC 0008's `resolve` of that set, and it does not depend on their order. *(Amendment 1.)*

**How each control change folds,** in the same transaction as any change (steps 1, 2, 5 and 6 above; steps 3 and 4 are replaced by this):

| Control change          | What the fold does                                                                                                                                                                              |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Genesis, membership entry, key chain entry, person-key entry | Adds it to the set of control changes. The control state is then recomputed through `resolve` from every control change the member holds, so a control change that arrives late can change the verdict of one accepted before. *(Amendment 1.)* |
| Key envelope            | If it is for this member, the SDK unwraps it into the keystore, and the control state records that this generation's key is held. Envelopes for other members are kept in the log and not opened. |
| Shred entry             | Records the person key as shredded. The fold erases that person's personal fields from the view at once, in every object of the space, and the member writes a "seen" acknowledgement. Once the shred is stable, the member deletes its copy of the key and writes a "deleted" acknowledgement ([RFC 0008, "Effects that wait until a change is stable"](0008-membership.md#effects-that-wait-until-a-change-is-stable); [RFC 0003, "Shredding a person's key"](0003-keys-and-cryptography.md#shredding-a-persons-key)). *(Amendment 1.)* |
| Acknowledgement         | Records which member saw which shred, and which deleted its copy of the key, for the audit. *(Amendment 1.)*                                                                                     |

**Re-checking.** Whenever the member appends a control change of a space, it checks the rejected changes of that space again from Amendment step 3, in the same transaction, as D11 defines. A change that now passes is decrypted and folded (steps 2 to 5 above) as if it had just arrived. A change that still fails stays rejected. The core may skip changes that can never pass, such as a bad signature or a payload that does not decrypt under a key the member holds.

**Un-folding.** A later control change can make a check fail that a change has already passed, for example a removal whose cutoff lies below changes the member already folded. RFC 0008 lists every way this happens ([What a new change can change](0008-membership.md#what-a-new-change-can-change), decision M4). *(Amendment 1.)* A sealed change then moves from folded to rejected in the transaction that appends the control change, and the fold refolds every field the change touched from the operations of the changes still folded. A field's state is a function of its set of operations (RFC 0004), so the view then equals the view of a member that never folded the change. A bookkeeping table from object and field to the operations that touched them, rebuildable from the log, makes this a lookup instead of a scan. Only sealed changes are un-folded. A control change that loses its verdict is recomputed through `resolve`: its reversible effects follow at once, and its irreversible effects never happened, because they wait until it is stable ([RFC 0008, "Effects that wait until a change is stable"](0008-membership.md#effects-that-wait-until-a-change-is-stable)). *(Amendment 1.)*

**A change the relay refuses after the member folded it.** It stays folded, as D12 defines. The member records the relay's refusal on the log entry, and the SDK reports it to the app. The view changes only if the member's own check, run again with the control changes it holds, now rejects the change; then it is un-folded as above.

**Why the end state depends only on the set.** A change is folded exactly when it passes its checks against the control state computed from all control changes the member holds. Re-checking and un-folding keep that true after every control change, whatever the order of arrival. Two members that hold the same set of changes therefore hold the same control state, fold the same set of changes, and, by RFC 0004, reach the same view (principle 2). The convergence harness (NOC-9) checks this with control changes in the generated sets, delivered in any order.

### Many changes

When the relay or a peer hands over a page of changes, the core folds the page in one transaction, which is much faster in SQLite than one transaction per change. The page is atomic as a whole: after a crash, either all of it or none of it is in the log and the view, and the relay position in `meta` moves with it. A change that stops at step 2 does not fail the page; it is kept as rejected or parked, as above.

A large first sync is cut into pages of a bounded number of changes, so that readers are never blocked for long ([One writer, many readers](#one-writer-many-readers)).

### Why the order of arrival does not matter

The fold applies changes in the order they arrive, which differs between members. That is safe because every field type's state is a function of the set of operations it has received (RFC 0004), and step 4 computes derived values from the folded rows, not from the order in which they were folded. The convergence harness (NOC-9) checks this for the fold as a whole: folding a set of changes in any order, with duplicates, gives byte-identical `c_*` and `fs_*` tables, and the same result as a [rebuild](#rebuilding-the-view) from the log.

### The member's own writes

A write by the app becomes a change: the core encrypts and signs it, then appends and folds it like any other change, in one transaction, before it is sent anywhere (RFC 0002: "It is in the member's view before it leaves the device"). The app reads its own write from the view at once, and keeps working without a relay (island mode).

### One writer, many readers

The fold is the only writer. Natively, the database runs in WAL mode: the fold writes on one connection, readers use their own connections and read the last committed state without waiting for the writer. In the browser, the database has one connection inside a dedicated worker ([SQLite on each target](#sqlite-on-each-target)); reads and folds take turns on it, and the fold commits page by page so a read waits at most for one page.

## Rebuilding the view

The view is a cache. Everything in it can be computed again from the frames in `log`, and the core does so when the view no longer matches what the code and the model expect.

### When

On open, the core compares `meta` with what it is running:

| Mismatch                                                                                | What the core does                                                                                             |
| --------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| The model adds a declared index                                                         | `CREATE INDEX` on the existing table. No refold.                                                               |
| The model adds a field or a collection (an additive change, RFC 0005)                   | Add the column or table, and fold the parked operations that are now known (RFC 0002). No full rebuild.        |
| The view format version changed (a new core lays out the view differently)              | Full rebuild.                                                                                                  |
| A field's rule version changed in `model.lock.json` (RFC 0004, RFC 0005)                 | Refold that field in every object of the collection from the log. A different merge rule is always a new rule version, never a new build of the same one: a merge rule never changes under the same type id and rule version (RFC 0004), so two members on different builds of a field type cannot fold differently. |
| `PRAGMA integrity_check` fails on a view table, or a view table is missing, while `log` is intact | Full rebuild.                                                                                          |
| `log` itself is damaged or missing                                                      | Not a rebuild: the member restores its log from the relay or a peer, from relay position 0, and folds it.       |

Changes the member wrote but the relay has not yet confirmed exist only in this member's log. Losing the log loses them; this is why the log is kept with full durability ([decision 7](#decisions-for-the-owner)).

### How

A full rebuild folds every frame in `log` into new, empty view tables beside the old ones, in pages, each page in its own transaction. Meanwhile the app keeps reading the old view and new changes keep arriving: each new change is folded into the old view and appended to `log`, so the rebuild picks it up too. When the rebuild has reached the end of `log`, one transaction drops the old tables, renames the new ones and updates `meta`. Readers then see the new view at once and completely, never half of it.

The rebuild folds the frames in local-position order. Any other order would give the same result ([Why the order of arrival does not matter](#why-the-order-of-arrival-does-not-matter)); local-position order is chosen only because it reads the table sequentially.

The rebuild decrypts every frame again, so it needs the space keys. A person key that was shredded ([RFC 0003, "Shredding a person's key"](0003-keys-and-cryptography.md#shredding-a-persons-key)) is gone, and the fold treats that person's personal fields as erased, exactly as it did before the rebuild. A rebuild therefore never brings erased data back.

### What a rebuild costs

A rebuild reads and decrypts the member's whole log. Its time grows with the number of changes, not the number of objects. Compaction (a later RFC) bounds it by replacing old history with a signed snapshot. Until compaction exists, the SDK reports a rebuild's progress, and the app keeps reading the old view while it runs. For a model change that is only additive (the common case), there is no full rebuild at all.

## Declared indexes and ordered pages

GENESIS.md, principle 6: "Queries run only through declared indexes. [...] Every index and view keeps its order, so results come sorted and paginated by default."

### Declared indexes

`model.lock.json` declares each index, as RFC 0005 defines. From the declaration this RFC uses the collection, the fields the index orders by with the direction of each, and which leading fields a query must fix with equality. The core turns each one into an SQLite index:

```sql
CREATE INDEX ix_tasks_by_status_priority
  ON c_tasks (status, priority, space_id, object_id);
```

Every index ends with `(space_id, object_id)`. Together they are unique, so the index order is total: no two rows tie, and a page boundary is always exact.

An index declared with `scope: "space"` ([RFC 0005, "Indexes"](0005-schema-and-model-lock.md#indexes)) puts the space id first instead, `(space_id, status, priority, object_id)` for the index above, and still ends with the object id, so its order is total too. Its equality prefix starts with the space id, which `space.find` fixes, so a query on it reads one space's rows ([RFC 0009, "Queries and indexes"](0009-sdk-api.md#queries-and-indexes), decision S8). An index without the option spans every space of the member, as [decision 1](#decisions-for-the-owner) chose. *(Amendment 6.)*

No declared index holds a deleted object, so a query never returns one, while reading the object by its id still finds it, to show or restore it ([RFC 0009, "Get by id"](0009-sdk-api.md#get-by-id), decision S9). The implementation keeps the rows out, for example through a partial index on the rows whose object is not deleted, and a restore puts them back. *(Amendment 6.)*

A reference, `nc.ref` ([RFC 0005, Amendment 5](0005-schema-and-model-lock.md#amendment-5-a-reference-into-another-space)), indexes as one column of 32 bytes, an SQLite `BLOB`: the target's space id followed by the object id, each 16 bytes, as the reference is stored and whatever its state, so the index is the same on every member of the space. "All orders of customer X" is then one equality lookup. An unset reference, and a value that does not fit ([RFC 0004, decision 17](0004-merge-rules-basic-types.md#decisions-for-the-owner-15-to-20)), index as unset (`NULL`). *(Amendment 7.)*

A query is always "the rows of this index, with these values for its equality prefix, after this cursor, at most N". The SDK offers no other query and no raw SQL. The core builds the SQL from the model, and it has one shape. For an index on `status` (equality) and `priority` (ascending), the page after a cursor whose last row has priority 3 reads:

```sql
SELECT status, priority, space_id, object_id FROM c_tasks
WHERE status IS ?
  AND priority >= 3                             -- the bound on the first key: SQLite seeks to the cursor
  AND (priority > 3 OR (priority = 3 AND        -- the cursor, key by key
       (space_id > ? OR (space_id = ? AND
        (object_id > ? OR (object_id = ? AND 0))))))
ORDER BY status ASC, priority ASC, space_id ASC, object_id ASC
LIMIT ?;
```

The first page has neither the bound nor the cursor condition. *(Amendment 5.)* The cursor condition goes key by key through the keys after the equality prefix, then `space_id` and `object_id`, which are ascending. For a key `k` with the cursor's value at `?`, it is one of four conditions, where `rest` is the condition for the next key and `0` (false) follows the last one:

| Key `k` | The cursor's value is set | The cursor's value is unset (`NULL`) |
| --- | --- | --- |
| Ascending | `(k > ? OR (k = ? AND rest))` | `(k IS NOT NULL OR (k IS NULL AND rest))` |
| Descending | `(k < ? OR k IS NULL OR (k = ? AND rest))` | `(k IS NULL AND rest)` |

This follows the order of [The order of values](#the-order-of-values): an unset value sorts first ascending and last descending. On the first key after the equality prefix, the query adds a bound so that SQLite seeks instead of walking the prefix from its start: `k >= ?` when the key is ascending and the cursor's value is set, `k IS NULL` when it is descending and unset, and none otherwise, so a descending first key scans within the equality prefix. *(Amendment 5.)* The equality prefix is written `k IS ?`, so a lookup can fix a key to unset.

**The check.** When the core opens a view, it runs `EXPLAIN QUERY PLAN` for the two query shapes of every declared index, its first page and a page after a cursor. It accepts a plan when every step that reads the table, a `SEARCH` or a `SCAN`, goes `USING INDEX` or `USING COVERING INDEX` through the declared index. It refuses to open when a step reads the table without it or through another index, or when the plan sorts in a temporary B-tree (`USE TEMP B-TREE`). An index without an equality prefix plans its first page as `SCAN ... USING COVERING INDEX`, which walks the index in order and stops after the page, so a `SCAN` on the declared index is accepted. *(Amendment 5.)* A test in the implementation ticket does the same for a model that uses every field type. The spike shows the plan SQLite chooses for this shape: `SEARCH tasks USING COVERING INDEX tasks_by_priority ((priority,id)>(?,?))`.

Filters on a field that is not in the index are not offered. An app that needs one declares an index, or a computed field and a view (the later RFC). This is the price of "reads cost only their result" (RFC 0001), and RFC 0001's [What it is not](0001-nocentraldb.md#what-it-is-not) names it.

### The order of values

The order of an index must be the same on every member and every target, or two members show the same page differently. So:

- **Text sorts by its UTF-8 bytes** (SQLite's `BINARY` collation), never by a locale's rules. Locale-aware collation depends on the platform's ICU or OS version, which would fork behaviour by platform (principle 7). An app that wants "Ä" next to "A" declares a computed sort key (the later RFC). This is [decision 5](#decisions-for-the-owner).
- **Numbers** sort numerically; integers and floats as SQLite compares them. The canonical encoding of RFC 0002 gives every value one representation, so equal values sort as equal.
- **An unset field** sorts first in ascending order and last in descending order (SQLite's `NULL` order).
- **A reference sorts by its 32 bytes** (SQLite's `BLOB` comparison, `memcmp`): first by the target's space id, then by its object id. *(Amendment 7.)*
- **The same SQLite version on every target** ([SQLite on each target](#sqlite-on-each-target)), so the comparison code itself is the same.

### Ordered pages

A page is at most N rows, in index order, and a cursor that points after its last row. The cursor holds the last row's index key: the values of the indexed fields, the space id and the object id, plus the index's name and the model hash. The next page is the rows whose key comes after the cursor's in the index's order, written key by key as [Declared indexes](#declared-indexes) shows, which is one walk of the index from the cursor, whatever page it is. *(Amendment 5.)*

- **A page is cheap at any depth.** Unlike `OFFSET`, a keyset cursor does not read the rows before it.
- **Concurrent folds do not break paging.** A row that did not change between two pages is never skipped and never repeated. A row that changed its indexed values between two pages may appear twice or not at all, because it moved across the boundary. The SDK says so in the cursor's documentation; a consistent snapshot over many pages is not offered (it would hold a read transaction open and block the WAL from shrinking).
- **A cursor is local and holds content.** Its key values are plaintext field values. It never leaves the device in the SDK's design, is never logged, and its `Debug` output is redacted ([RFC 0003, "Secret types do not print"](0003-keys-and-cryptography.md#keys-never-reach-logs-errors-or-telemetry)). An app that puts it in a URL makes a copy the core cannot erase.
- **A cursor from another model is refused.** If the model hash in the cursor differs from the view's, the query fails with a plain error and the app starts from the first page.
- **The default page size** is 50, and the maximum is a fixed number the implementation ticket sets, so a single read cannot hold the database for long.

## SQLite on each target

Principle 7: "The same core runs natively and in a web browser. Behaviour never forks by platform." The fold, the SQL text, the pragmas and the queries are one body of Rust code that talks to SQLite through `rusqlite`. Only two things differ by target, both chosen when the database is opened: the **VFS** (how SQLite reaches storage) and the **location** of the file. The spike runs the same function natively and in a browser and gets the same rows and the same query plan.

| Target                         | SQLite                                                                                       | Storage                                                         | Concurrency                                     |
| ------------------------------ | -------------------------------------------------------------------------------------------- | --------------------------------------------------------------- | ----------------------------------------------- |
| Desktop: macOS, Linux, Windows | `rusqlite` with `bundled`: SQLite compiled from source by `libsqlite3-sys`                   | A file in the app's data directory, WAL mode                    | One writer connection, reader connections       |
| Mobile: iOS, Android           | The same, built for the mobile target                                                        | A file in the app's sandbox, WAL mode                           | As desktop                                      |
| Browser: Chromium, Firefox, Safari | `rusqlite` with `ffi-sqlite-wasm-rs`: `sqlite-wasm-rs`, SQLite compiled to `wasm32-unknown-unknown` | The Origin Private File System (OPFS), through the `opfs-sahpool` VFS of `sqlite-wasm-vfs` | One connection, in a dedicated Web Worker |

### Desktop and mobile

- **Library.** `rusqlite`, pinned to an exact version, with the `bundled` feature, so the SQLite in the binary is the one we tested, compiled from source. The system's SQLite is never used: its version differs between macOS releases, Linux distributions and Android versions, and so would query plans and edge cases.
- **Durability.** WAL mode with `synchronous = FULL`, so a committed transaction survives a power loss. The log holds the member's unconfirmed writes, which exist nowhere else ([decision 7](#decisions-for-the-owner)).
- **iOS.** The file uses the data-protection class "complete until first user authentication", so the core can fold in the background after the device has been unlocked once since boot. The stricter class "complete" would make the file unreadable while the device is locked, and background sync would fail. This is part of [decision 6](#decisions-for-the-owner).
- **Limits.** The OS may kill the app at any point; the transaction guarantees the database is consistent afterwards. A full disk makes the fold fail; the change stays with the relay or peer and is folded later, and the SDK reports the error.

### The browser

- **Library.** The same `rusqlite`, with the `ffi-sqlite-wasm-rs` feature. `sqlite-wasm-rs` compiles SQLite's C source to `wasm32-unknown-unknown` with clang and links it into the core's own WebAssembly module, so the core calls SQLite directly, without a round-trip through JavaScript. `sqlite-wasm-vfs` provides the `opfs-sahpool` VFS, a port of the official SQLite build's VFS of the same name.
- **Where it runs.** In a dedicated Web Worker. `opfs-sahpool` uses OPFS synchronous access handles, which browsers offer only in dedicated workers. On the main thread it fails (the spike: `NotSupported`). The SDK starts the worker; the app talks to it through the SDK's asynchronous API, which is the same API it uses natively.
- **Browser support.** OPFS synchronous access handles exist in Chromium 108+, Firefox 111+ and Safari 16.4+. The spike ran only in Chromium. The official build's `opfs-sahpool` works in Playwright's Firefox and WebKit ([NOC-33](../browser-storage.md#findings)) and in branded Safari on iOS and macOS ([NOC-33's findings](../browser-storage.md#findings), measured: Safari (iOS) · SQLite on OPFS (`opfs-sahpool`); measured: Safari (macOS) · SQLite on OPFS (`opfs-sahpool`)) *(Amendment 3)*; the Rust port must still run in Firefox and Safari before the implementation ships (see [The spike](#the-spike)). *(Amendment 2.)*
- **No special headers.** `opfs-sahpool` needs no `SharedArrayBuffer`, so the app's site does not need the COOP and COEP headers.
- **One tab owns the database.** A pool of access handles is exclusive: while one tab's worker holds it, another tab of the same origin cannot open it. The SDK elects one tab as the owner with the Web Locks API; the other tabs send their reads and writes to it with a `BroadcastChannel`. When the owner closes, the next tab takes the lock and opens the database. This is [decision 4](#decisions-for-the-owner).
- **Size.** The spike's module, SQLite included, is 1.8 MB, or 0.69 MB compressed with gzip.

**Limits of storage in the browser.** These are limits of the platform, and the SDK states them to the developer:

- **The browser can evict the data.** Under storage pressure, a browser may delete an origin's storage unless the site has persistent storage (`navigator.storage.persist()`). Safari deletes all script-writable storage of a site the user has not interacted with for seven days, unless it is a web app added to the home screen. A browser evicts all of an origin's data at once, so the view, the log and the member key in IndexedDB ([RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys)) go together. The browser is then no longer that member: the person sets it up again as a new device, and the changes the member wrote that the relay has not confirmed are lost. *(Amendment 2.)* The SDK asks for persistent storage on open, reports whether it was granted, and reports how many unconfirmed changes the member holds ([decision 8](#decisions-for-the-owner)). [Browser apps and durability](../browser-durability.md) says, per browser, what is measured, documented and unknown.
- **Private windows.** Firefox and WebKit have no OPFS in a private window, while IndexedDB works; Chromium has OPFS there and deletes it when the window closes. *(Amendment 2.)* Where OPFS is missing, the SDK reports that the copy is not persistent, runs on SQLite's in-memory VFS and syncs from the relay on every open.
- **Quota.** The origin's storage quota limits the size of the copy. The SDK reports the quota error instead of failing silently.

### The same fold code on every target

```rust
// The only target-specific code: which VFS and which file.
#[cfg(not(target_arch = "wasm32"))]
fn open(path: &Path) -> rusqlite::Result<Connection> { Connection::open(path) }

#[cfg(target_arch = "wasm32")]
async fn open(name: &str) -> Result<Connection, Error> {
    sqlite_wasm_vfs::sahpool::install::<WasmOsCallback>(&OpfsSAHPoolCfg::default(), false).await?;
    Ok(Connection::open(format!("file:/{name}?vfs=opfs-sahpool"))?)
}

// Everything else, the fold included, takes a &Connection and is the same on every target.
pub fn fold_page(conn: &Connection, frames: &[Frame]) -> Result<Folded, Error> { ... }
```

The browser's one asynchronous step is installing the VFS, once per worker. After that every SQLite call is synchronous inside the worker, as it is on a native thread. The convergence harness and the wasm32 gate (NOC-9, NOC-10) run the same fold tests on both targets.

**The SQLite version must be the same on every target.** The spike found `rusqlite` 0.41.1 with `bundled` builds SQLite 3.54.1, while `sqlite-wasm-rs` 0.6.2 builds SQLite 3.53.4. A difference in SQLite version is a difference in behaviour (query planner, edge cases of comparison), so the implementation ticket for the view pins both to the same SQLite release and adds a test that compares `sqlite_version()` on both targets. Where the crates cannot be aligned, `libsqlite3-sys` can be built natively from the same amalgamation `sqlite-wasm-rs` uses.

## What the view stores in plaintext

The log stores ciphertext (RFC 0002). The view does not: it exists so that reads are fast, and a read of an encrypted value would need a decryption per row and could not use an index. So the database on a device holds, in plaintext:

- **Every field's read value** of every object in every space the member belongs to.
- **The merge state** of every field, including the writes that lost (RFC 0004's conflicts). A value someone overwrote is still readable on the device until the field type drops it.
- **The indexes**, which repeat the indexed values.
- **Personal fields**, decrypted, for as long as the person key exists. After a shred (RFC 0003), the fold erases them from the view.
- **Metadata**: space ids, object ids, member ids, relay positions.
- **Leftovers SQLite keeps**: the WAL file, and free pages that still hold deleted rows. With `secure_delete` on, SQLite overwrites deleted content with zeros, and after a shred the core checkpoints the WAL with `TRUNCATE` ([decision 9](#decisions-for-the-owner)).

The database holds **no keys**. Private keys live in the platform's keystore, and in the browser as a non-extractable WebCrypto key that wraps the core's keys in IndexedDB ([RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys)).

**What that means for a lost device** (principle 4: say plainly what it does not do):

- **Anyone who can read the device's storage can read every space the member on it belongs to.** NoCentralDB protects the data from the relay, not from someone holding an unlocked device or its disk. Whether a stolen disk is readable depends on the device's own encryption: FileVault, BitLocker, LUKS, and the file-based encryption of iOS and Android protect a device that is off or locked. A browser's OPFS files lie in the browser profile, protected only by the operating system's disk encryption.
- **Removing the lost device's member does not delete its copy.** It stops the member from receiving or writing new changes ([RFC 0003, "Removing a member and rotating the space key"](0003-keys-and-cryptography.md#removing-a-member-and-rotating-the-space-key)). What it already holds, it keeps.
- **Shredding does not reach a lost device.** Its acknowledgement never arrives, and the audit shows that (RFC 0003).
- **Overwriting is not erasing on flash storage.** `secure_delete` overwrites the bytes SQLite can reach. An SSD or a phone's flash memory may keep old copies of a block where SQLite cannot reach them. Only the device's own disk encryption covers those.
- **A process running as the user can read the database.** As RFC 0001 says in [What it is not](0001-nocentraldb.md#what-it-is-not), the device is not a security boundary against the user's own processes, agents included.

Encrypting the database itself, for example with SQLCipher, is the owner's question and out of scope here: see [decision 6](#decisions-for-the-owner).

## Reads never touch the network

A read is a query against the view on the device. It returns what the view holds now, and never asks the relay or a peer for anything, not even when the view is empty or behind:

- **No fetch on a miss.** A read of an object the member has not received yet returns "not found", not a request to the relay. The SDK exposes how far each space's copy is behind (the relay position the member has folded versus the last one it has heard of) and whether the initial sync is complete, so an app can show "syncing" instead of guessing.
- **Separate code paths.** In the core, the module that runs queries has no dependency on the module that talks to the relay. The implementation ticket enforces it in a test: the query module compiles and its tests pass with the sync module replaced by one that fails on every call.
- **The same in the browser.** Reads are served by the worker that owns the database, from SQLite in OPFS. A read sent from another tab to the owning tab is a message between tabs of the same browser, not a network request.
- **Subscriptions** re-run a query after a fold. They react to what arrived; they never cause a network request themselves.

Syncing is the only thing that uses the network, and it writes to the log, never to a read result directly.

## The spike

The ticket asks for a spike that opens SQLite from Rust compiled to `wasm32-unknown-unknown` in a headless browser. It was run on 2026-11-07 and is not merged. Its code is on no branch: it is in the description of the pull request that introduces this RFC (#36), under "Spike code (not merged)".

**Result: works in Chromium**, with `rusqlite` 0.41.1 (`ffi-sqlite-wasm-rs`), `sqlite-wasm-rs` 0.6.2, `sqlite-wasm-vfs` 0.3.0 (`opfs-sahpool`), `wasm-bindgen` 0.2.130 and Rust 1.99.1, in headless Chromium 153 driven by Playwright.

What it did: one Rust function, `fold_and_page`, creates a table and a declared index, writes 1,000 rows in one transaction, reads a page of three rows after a keyset cursor, and reports the query plan and the SQLite version. The same function ran natively (`rusqlite` with `bundled`, in memory) and in the browser (in a dedicated worker, on OPFS).

| Check                                          | Native                                       | Chromium 153, dedicated worker, OPFS          |
| ---------------------------------------------- | -------------------------------------------- | --------------------------------------------- |
| Builds and opens SQLite                        | Yes                                          | Yes                                           |
| Same page after the cursor                     | `t0790:10, t0469:11, t0148:12`               | `t0790:10, t0469:11, t0148:12`                |
| Query plan                                     | `SEARCH ... USING COVERING INDEX`            | `SEARCH ... USING COVERING INDEX`             |
| Data survives closing and reopening the page   | n/a (in memory)                              | Yes: the second page load found 1,000 rows    |
| SQLite version                                 | 3.54.1                                       | 3.53.4                                        |
| On the main thread instead of a worker         | n/a                                          | Fails: `NotSupported`                         |
| Size of the WebAssembly module                 | n/a                                          | 1.8 MB, 0.69 MB with gzip                     |

What it did not show, and must be shown before the implementation ships:

- **Firefox and Safari.** Only Chromium was available to the spike. Both browsers support the API `opfs-sahpool` needs, and the official build's `opfs-sahpool` works in both ([Amendment 2](#amendment-2-durability-of-a-browser-copy)), but the Rust spike has not run in them. *(Amendment 2.)* The wasm32 gate (NOC-10) or the implementation ticket must run the same check in Firefox and WebKit.
- **More than one tab** ([decision 4](#decisions-for-the-owner)).
- **Speed** at the size of a real space. The spike measured nothing.
- **The library's maturity.** `sqlite-wasm-rs` and `sqlite-wasm-vfs` are young crates with one main maintainer. They are small and wrap the official SQLite source and a port of its official VFS, so forking them is feasible if they stop being maintained.

## Rejected options

| Option                                                                                         | Why not                                                                                                                                                                                                                                          |
| ---------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| The official SQLite WebAssembly build (`sqlite3.wasm` with its JavaScript API) in the browser   | SQLite would live in a second WebAssembly module, driven from JavaScript. The Rust fold would call it through JavaScript for every statement, and the browser would run a different SQLite binding than native: behaviour would fork (principle 7). |
| wa-sqlite in the browser                                                                       | The same problem: a separate module behind a JavaScript API. Its `OPFSCoopSyncVFS` handles many tabs better than `opfs-sahpool`; if decision 4's leader tab proves fragile, its approach can be ported to the Rust VFS.                          |
| A VFS on IndexedDB                                                                             | Works on the main thread and in older browsers, but writes each page through an asynchronous store and is much slower. OPFS exists in every browser we target.                                                                               |
| The `opfs` VFS of the official build                                                           | Needs `SharedArrayBuffer`, so every app's site must send COOP and COEP headers, which break many third-party embeds.                                                                                                                             |
| SQLite on the browser's main thread                                                            | OPFS synchronous access handles are not available there (the spike: `NotSupported`), and a long fold would freeze the page.                                                                                                                     |
| The system's SQLite natively                                                                   | Its version differs by OS release, so query plans and edge cases would fork by platform.                                                                                                                                                       |
| One database per space                                                                         | An index could not span spaces: "my next 50 tasks across every project" would be one query per space and a merge in memory. See decision 1.                                                                                                  |
| Storing read values encrypted in the view                                                      | An index cannot order ciphertext, and every read would decrypt every row it scans. Protecting data at rest belongs to the disk or the database file (decision 6).                                                                              |
| Raw SQL for apps                                                                               | An app could run a full table scan, which breaks principle 6, and could read the merge state and the log directly.                                                                                                                              |
| `OFFSET` pagination                                                                            | Page N reads all rows before it, and rows shift when a fold inserts one before the page boundary.                                                                                                                                              |
| Locale-aware collation in indexes                                                              | Depends on ICU or OS versions, so two members could order the same page differently (principle 7).                                                                                                                                            |
| A stable, versioned view layout that every client must keep reading                            | The view can always be rebuilt from the log. A promise about its layout would freeze the implementation without protecting any data. See decision 2.                                                                                         |

## Decisions for the owner

GENESIS.md reserves the on-disk format and anything a user sees first for the owner. Each decision has options and a recommendation.

1. **How many databases per member.**
   - A. One database per member on a device, holding all its spaces, with a space id in every row. *(recommended)*
   - B. One database per space.
   Recommendation: A. Indexes span spaces, so cross-space pages are one range lookup; leaving a space deletes its rows.

   Decided (Carsten, 2026-11-08): the recommendation, (A).

2. **Is the view's layout a stable format?**
   - A. No. Only the log (RFC 0002) is a stable on-disk format. The view's tables are internal to the core, versioned in `meta`, and a new core may rebuild them. *(recommended)*
   - B. Yes, versioned and migrated like the log.
   Recommendation: A. The view can always be rebuilt from the log; a stable layout would only slow down the core's development. Apps never see the tables (no raw SQL).

   Decided (Carsten, 2026-11-08): the recommendation, (A).

3. **The SQLite stack.**
   - A. `rusqlite`, pinned, with `bundled` natively and `ffi-sqlite-wasm-rs` (`sqlite-wasm-rs`, `opfs-sahpool` from `sqlite-wasm-vfs`) in the browser, in a dedicated worker. *(recommended)*
   - B. `rusqlite` natively, and the official SQLite WebAssembly build or wa-sqlite in the browser, called through JavaScript.
   Recommendation: A. One SQLite binding and one body of fold code on every target (principle 7), and the spike shows it works. The risk is two young crates; the fallback is to fork them.

   Decided (Carsten, 2026-11-08): the recommendation, (A).

4. **Several tabs of the same app in one browser.**
   - A. One tab owns the database, elected with Web Locks; other tabs reach it through a `BroadcastChannel`. *(recommended)*
   - B. Only one tab may run the app; the others show a notice.
   Recommendation: A. Users open apps in several tabs; B would surprise them. A needs its own test, as the spike did not cover it.

   Decided (Carsten, 2026-11-08): the recommendation, (A).

5. **The order of text in indexes.**
   - A. UTF-8 byte order everywhere; locale-aware order only through a computed sort key. *(recommended)*
   - B. Locale-aware collation in the index.
   Recommendation: A. B makes the order depend on the platform's ICU version, so two members could order the same page differently.

   Decided (Carsten, 2026-11-08): the recommendation, (A).

6. **Encryption at rest on the device.** Out of scope for this RFC to design; named here as the question.
   - A. Rely on the device: the OS's disk encryption, the platform keystore for keys, and on iOS the data-protection class "complete until first user authentication". The SDK documents what that does not cover. *(recommended for v1)*
   - B. Encrypt the database file with SQLCipher or SQLite3 Multiple Ciphers, under a key in the platform keystore. `sqlite-wasm-rs` has features for both, so the same choice works natively and in the browser. Costs speed, and adds a key the SDK must manage.
   - C. Decide later, after v1.
   Recommendation: A for v1, with B as a later RFC if a user's review asks for it. A key held on the same unlocked device protects little more than disk encryption does.

   Decided (Carsten, 2026-11-08): the recommendation, (A).

7. **Durability of a commit.**
   - A. WAL with `synchronous = FULL`: a committed change survives a power loss. *(recommended)*
   - B. WAL with `synchronous = NORMAL`: faster; the last commits before a power loss may be lost, the database stays consistent.
   Recommendation: A. The log holds the member's writes the relay has not confirmed, which exist nowhere else (principle 3: nothing is lost silently).

   Decided (Carsten, 2026-11-08): the recommendation, (A).

8. **Persistent storage in the browser.**
   - A. The SDK asks for persistent storage on open and tells the app whether it was granted and how many unconfirmed changes it holds. *(recommended)*
   - B. Leave it to the app.
   Recommendation: A. Eviction loses the member's unconfirmed writes; the developer must learn that from the SDK, not from a user.

   Decided (Carsten, 2026-11-08): the recommendation, (A).

9. **Deleted content in the database file.**
   - A. `PRAGMA secure_delete = ON`, and a WAL checkpoint with `TRUNCATE` after a shred or after leaving a space. *(recommended)*
   - B. SQLite's defaults; free pages keep old content until reused.
   Recommendation: A. Erasure should remove what the database can reach on a device that comes back online, at a small cost in write speed. The limits on flash storage stay, and the SDK says so.

   Decided (Carsten, 2026-11-08): the recommendation, (A).

## Agreement with GENESIS.md and RFC 0001

| Promise                                                                                     | How this RFC keeps it                                                                                                                         |
| ------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| "A full copy per member [...] folds it locally into an SQLite view." (GENESIS.md, RFC 0001) | The fold, one database per member.                                                                                                             |
| "Reads never leave the device." / "Never put the network on the read path."                 | [Reads never touch the network](#reads-never-touch-the-network): no fetch on a miss, and the query module cannot reach the sync module.       |
| Principle 2: every copy converges, computed fields included.                                | The fold applies field types whose state is a function of the set; derived values are computed in the same transaction; the harness checks that an incremental fold equals a rebuild. |
| Principle 3: nothing is lost silently.                                                      | Parked changes and operations stay in the log and are folded after an upgrade; losers stay in the merge state; full durability for the log.    |
| Principle 4: say plainly what it does not do.                                               | [What the view stores in plaintext](#what-the-view-stores-in-plaintext), the browser's storage limits, and what the spike did not show.        |
| Principle 6: queries run only through declared indexes; results sorted and paginated.        | No raw SQL; a plan check on open; keyset cursors with a total order.                                                                           |
| Principle 7: the same core natively and in the browser; behaviour never forks.              | One `rusqlite` binding and one fold; only the VFS and the file location differ; the SQLite version must match.                                 |
| "Never write content, keys or personal fields into logs [...]."                             | Plaintext exists only in memory and in the view; notifications carry ids; cursors are redacted.                                                |
| "Runs on any device [...] web apps in a browser (Chromium-based, Firefox, and Safari)."      | The design uses only APIs those browsers offer. The Rust spike ran in Chromium only; the official build's `opfs-sahpool` works in Firefox and Safari (Amendment 2). The Rust port in Firefox and Safari is an open check, not a claim. *(Amendment 2.)* |

**Conflicts.** None with GENESIS.md. One point a reader could take further than the design delivers: RFC 0001 says that "a phone can store a team's complete working set". In a browser, the copy can be evicted (Safari's seven-day rule above), so the "full copy" in a browser is a cache the browser may drop, unlike on a native device. This RFC states that limit; NOC-20 adds it to RFC 0001, and NOC-34 adds that the member key goes with the copy *(Amendment 2)*.

## Amendment 1: following RFC 0008

Added on 2026-11-08 for NOC-44. This RFC left two things to the membership RFC: what the control state holds, and whether a control change that arrives late can un-fold a change. [RFC 0008](0008-membership.md) answers both, and the owner accepted every one of its decisions, M1 to M26 (Carsten, 2026-11-08). Its section [Conflicts with GENESIS.md and the other RFCs](0008-membership.md#conflicts-with-genesismd-and-the-other-rfcs), point 13, names where this RFC said something else. This amendment changes those sentences, marked *(Amendment 1)* above. The owner has decided every point, so it adds no decision. The `control` table's layout is not a stable format ([decision 2](#decisions-for-the-owner)), so nothing on disk is promised that would need a migration.

- **The control table** (conflict 13). It holds RFC 0008's control state: the genesis, the members with their roles, the grants with their from generations and the revocations with their cutoffs, the generation graph, the person-key entries, the shreds and their acknowledgements, and which generations this member holds ([RFC 0008, "The function"](0008-membership.md#the-function)).
- **Control changes are recomputed, sealed changes un-folded** (conflict 13, decision M4). A late control change can change the verdict of a change accepted before. A sealed change whose verdict flips is un-folded as [Un-folding](#control-changes-and-changes-that-wait-for-them) describes. A control change is never un-folded: the core recomputes `resolve` over every control change it holds ([RFC 0008, "What a new change can change"](0008-membership.md#what-a-new-change-can-change)).
- **The shred row waits until stable** (decision M20). The fold erases the person's fields at once and writes a "seen" acknowledgement. Deleting the member's copy of the key and the "deleted" acknowledgement wait until the shred is stable, as RFC 0008 defines it, because a destroyed key cannot come back if a concurrent revocation rejects the shred later ([RFC 0008, "Effects that wait until a change is stable"](0008-membership.md#effects-that-wait-until-a-change-is-stable)).

## Amendment 2: durability of a browser copy

Added on 2026-11-09 for NOC-34. This RFC says that a browser may evict a member's copy and that the SDK reports it ([decision 8](#decisions-for-the-owner)). NOC-33 measured what each browser does with the copy, in Playwright's builds of Chromium, Firefox and WebKit, and collected what the vendors document; [How long a browser keeps a web app's copy](../browser-storage.md#findings) holds every finding with its source: measured, documented or unknown. This amendment states what that changes in this RFC and proposes how developers and their users learn it. The owner asked for a clean way to communicate it (Carsten, 2026-11-08). Decisions 10 to 15 below name public API and text for users, which GENESIS.md reserves for the owner; the owner decided each as recommended (Carsten, 2026-11-09).

**What the findings change in this RFC.** The sentences are marked *(Amendment 2)* above.

- **Eviction takes the member key too.** A browser evicts all of an origin's data at once (documented, MDN), and the member key lives in IndexedDB of the same origin ([RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys)). Safari's seven-day deletion names IndexedDB (documented, WebKit); whether it reaches OPFS is unknown. So after an eviction, the browser cannot fetch its spaces again as the same member: the person pairs it again as a new device ([RFC 0003, "The account key"](0003-keys-and-cryptography.md#the-account-key)). [RFC 0001, "What it is not"](0001-nocentraldb.md#what-it-is-not) says so too.
- **Safari runs `opfs-sahpool`.** The official build's `opfs-sahpool`, which `sqlite-wasm-vfs` ports to Rust, wrote and read back in branded Safari on iOS and macOS, as it did in Playwright's Firefox and WebKit ([NOC-33's findings](../browser-storage.md#findings), measured: Safari (iOS) · SQLite on OPFS (`opfs-sahpool`); measured: Safari (macOS) · SQLite on OPFS (`opfs-sahpool`)). *(Amendment 3.)* The Rust port itself has run only in Chromium.
- **Private windows.** Firefox and WebKit have no OPFS in a private window (`getDirectory()` fails), while IndexedDB works; Chromium has OPFS there and deletes it, and IndexedDB, when the window closes (measured). Branded Firefox on macOS has no OPFS there either; IndexedDB wrote, and once the private window was closed, a new private window read nothing, so what it held, the member key included, is gone after the window closes ([NOC-33's findings](../browser-storage.md#findings), measured: Firefox · Private window). *(Amendment 3.)* Within one open private session, IndexedDB, and the member key in it, survives a reload of the page in Playwright's Chromium, Firefox and WebKit (NOC-58; [NOC-33's findings](../browser-storage.md#findings), measured: Chromium (Playwright) · Private window: survives a reload (off-the-record context); measured: Firefox (Playwright) · Private window: survives a reload (private browsing mode); measured: WebKit (Playwright) · Private window: survives a reload (ephemeral context)); in the branded browsers nobody has checked a reload by hand. *(Amendment 4.)* What branded Safari, Chrome and Edge do in a private window is unknown.
- **`persist()` asks the user in Firefox.** Firefox shows a popup and the answer waits for the user (measured: no answer within 5 s; documented, MDN). Chrome and Edge decide silently by heuristics such as installation (documented); Playwright's Chromium and WebKit denied it (measured). Branded Safari denied it on iOS, in the browser and in a Home Screen app, and granted it to a Dock app on macOS; Firefox on macOS showed its prompt ([NOC-33's findings](../browser-storage.md#findings), measured: Safari (iOS) · `persist()`; measured: Safari (iOS) · `persist()` in a home-screen app; measured: Safari (macOS) · `persist()` in a Dock app; measured: Firefox (macOS) · `persist()`). *(Amendment 3.)* So installing an app to the iOS Home Screen does not make its copy persistent. Most browser copies therefore start as best effort, and asking on open, as decision 8 says, shows Firefox users a popup at first open.

**The four layers.** A developer learns how reliable a browser copy is from the SDK (decisions 10 to 12), from one page of documentation (decision 13), and passes it on with text for end users (decision 14). Decision 15 says where the developer meets it first. The page [Browser apps and durability](../browser-durability.md) is drafted in this amendment's pull request, so the owner decides on real text; every row in it names the finding it rests on, and the gate (`scripts/browser-durability.test.mjs`) fails when a row claims more than its finding.

10. **How the SDK reports durability.**
    - A. One namespace, `db.durability`, the same on every target. `current()` returns `{ storage, unconfirmed, oldestUnconfirmedAt, atRisk }`. `subscribe(listener)` calls the listener whenever one of them changes, at most once per fold or push, and returns a function that stops it. `requestPersistent()` asks the browser for persistent storage and resolves when the browser answers. *(recommended)*
    - B. Two places: `db.storage.persistent` (a boolean) and `db.sync.pending` (a count), each with its own event.
    - C. A boolean `db.isPersistent` only; apps count unsent changes themselves.
    Recommendation: A. One object answers the question a developer has: what would this user lose if the browser deleted the copy now. Only the core knows which changes the relay has confirmed, so C would leave apps guessing; B splits one answer in two. The values:
    - `storage: "persistent"`: a native app, or a browser whose `navigator.storage.persisted()` is true. It does not promise that the copy survives the user clearing site data, a lost device, or Safari's seven-day deletion, which persistent storage may or may not stop (unknown).
    - `storage: "best-effort"`: the copy is on OPFS and the browser has not granted persistent storage. The browser may delete it when storage runs low, and Safari after seven days without interaction. A Chromium private window reports `best-effort`, because the SDK cannot tell it from a normal window; its copy is deleted when the window closes (measured).
    - `storage: "temporary"`: OPFS could not be opened, as in a private window of Firefox or WebKit, and the copy is in memory (decision 12). It lives at most until the private session ends.
    - `unconfirmed`: the number of changes this member wrote that have no relay position yet ([RFC 0007, "Island mode"](0007-relay.md#island-mode)). They exist only on this device. Changes received from a peer are not counted: their author holds them. `oldestUnconfirmedAt` is the local time of the oldest of them, or `null`.
    - `atRisk`: true while `unconfirmed` is above zero and `storage` is not `persistent`.
    The SDK reads `persisted()` on open, after `requestPersistent()`, and whenever the page becomes visible again, because a user can grant or revoke it in the browser's settings.

    Decided (Carsten, 2026-11-09): the recommendation, (A).

11. **When the SDK asks for persistent storage.**
    - A. On open, as decision 8 says, without waiting: `nc.open` resolves before the browser answers, and `subscribe` reports the answer. An app that wants to explain first opens with `nc.open({ model, persistentStorage: "on-request" })` and calls `requestPersistent()` after its own onboarding text. *(recommended)*
    - B. On open, and `nc.open` waits for the answer.
    - C. Only when the app calls `requestPersistent()`, never on open.
    Recommendation: A. It keeps decision 8 for an app that does nothing, and refines it rather than reversing it. B would leave `nc.open` hanging in Firefox until the user answers the popup. C reverses decision 8 and leaves every app that forgets the call with a best-effort copy.

    Decided (Carsten, 2026-11-09): the recommendation, (A).

12. **The fallback in a private window, where OPFS is missing.**
    - A. SQLite's in-memory VFS, as [The browser](#the-browser) says, with `storage: "temporary"`. A reload of the page loses the copy and its unconfirmed changes, and the SDK fetches the spaces again from the relay. *(recommended)*
    - B. A VFS on IndexedDB for the length of the private session.
    Recommendation: A for v1. B helps only between a reload and the end of the private session. NOC-58 measured that IndexedDB survives a reload in a private window in Playwright's Chromium, Firefox and WebKit ([NOC-33's findings](../browser-storage.md#findings)), so B's copy would last that long there; the branded browsers have not been checked. *(Amendment 4.)* B would add a second storage path to build and test in three engines, with writes through an asynchronous store, which [Rejected options](#rejected-options) found much slower, and a durability no spike has checked against [decision 7](#decisions-for-the-owner). Either way the copy is temporary and the user is told the same. The member key in IndexedDB survives a reload as well (NOC-58), so a reload in a private window is not a new device; what a private window held, the member key included, is gone once it closes (measured: Firefox · Private window). *(Amendment 4.)* If apps report lost changes, B can be revisited with those rows.

    Decided (Carsten, 2026-11-09): the recommendation, (A).

    Confirmed (Carsten, 2026-11-09) after NOC-58's measurement: A stays. *(Amendment 4.)*

13. **The developer documentation.**
    - A. One page, [Browser apps and durability](../browser-durability.md), as drafted: NOC-33's findings in plain words per browser, what is lost when, how to reduce the risk (keep syncing, ask for persistent storage, install the app, offer a native app), private windows, the SDK's report and the text for users. Every row names its finding, and the gate checks it. *(recommended)*
    - B. The same facts in the SDK's API reference only.
    Recommendation: A. A developer has to know this before choosing a browser app for their users (principle 4), which is before reading an API reference. The page marks the SDK interface as decided and not yet built until it ships.

    Decided (Carsten, 2026-11-09): the recommendation, (A).

14. **Text for end users.**
    - A. The page carries short English texts that the developer copies into the app, with the placeholders `{appName}` and `{count}`: one for the onboarding of a browser app, one for unsaved changes (for one change and for more), and one for a private window. The SDK ships no text for users. *(recommended)*
    - B. The SDK exports the texts, for example `nc.text.durability.unsaved({ appName, count })`, so they change with the SDK.
    Recommendation: A for v1. The app owns its interface and its languages; exported texts would be more public API to keep stable and translate. The gate checks that each text names the app, stays under 60 words, and uses no word that promises more than the findings; no user has read them yet, which the page says.

    Decided (Carsten, 2026-11-09): the recommendation, (A).

15. **Where the developer meets it, and the warning in development.**
    - A. The README and RFC 0001 link the page. In development, the SDK logs one `console.warn` per page load when `storage` is not `persistent`. "Development" is `process.env.NODE_ENV !== "production"`, which bundlers replace at build time; where nothing replaces it, the warning stays on. `nc.open({ model, warnings: false })` turns it off. The warning names the status, the number of unconfirmed changes and the page, and nothing else: no content and no ids ("Never write content, keys or personal fields into logs", GENESIS.md). *(recommended)*
    - B. The same warning in every build, production included.
    - C. No warning; the page and `db.durability` only.
    Recommendation: A. A developer trying a browser app sees the limit on the first day, without reading anything, and the end user's console stays quiet. The proposed text: `NoCentralDB: this browser copy is best-effort. The browser may delete it, together with 3 changes the relay has not confirmed and this browser's member key. See https://github.com/nocentraldb/nocentraldb/blob/main/docs/browser-durability.md`

    Decided (Carsten, 2026-11-09): the recommendation, (A).

**Conflicts.** None with GENESIS.md. One with RFC 0001, which said a browser member fetches its log again after an eviction; RFC 0001's Amendment 3 corrects it in this pull request. This amendment adds no format, so nothing on disk or on the wire changes.

## Amendment 3: the device checks as findings rows

Added on 2026-11-09 for NOC-35. Amendment 2 cited the owner's device checks of 2026-11-09 by the headings of #74's manual check log, because #74's rows were not on `main` yet. #74 (NOC-60) is merged, and [NOC-33's findings](../browser-storage.md#findings) now hold those checks as rows, so the sentences marked *(Amendment 3)* above cite the rows, as [Browser apps and durability](../browser-durability.md) does. It decides nothing.

- **A Firefox private window, after it closes.** Amendment 2 said the log did not say whether the first private window was closed before a new one read nothing. The owner has answered that it was closed. So what a Firefox private window held, the member key included, is gone once the window closes (measured: Firefox · Private window), as in Playwright's Firefox. Whether IndexedDB lasts within one open private session is still unknown; NOC-58 measures it. Its decision 12 stays as decided: it rests on that open question, not on this one.

## Amendment 4: a reload in a private window

Added on 2026-11-09 for NOC-58. Amendment 2 and decision 12 left open whether IndexedDB keeps its data across a reload within one open private session, and with it the member key. NOC-58 measured it with NOC-33's probe: the probe writes in a private window, reloads the page in the same window and reads back. IndexedDB kept the data in Playwright's Chromium, Firefox and WebKit ([NOC-33's findings](../browser-storage.md#findings), measured: Chromium (Playwright) · Private window: survives a reload (off-the-record context); measured: Firefox (Playwright) · Private window: survives a reload (private browsing mode); measured: WebKit (Playwright) · Private window: survives a reload (ephemeral context). This amendment decides nothing; the sentences marked *(Amendment 4)* above state the measured facts.

- **The private-window finding** now says that IndexedDB, and the member key in it, survives a reload in all three engines, and that the branded browsers have not been checked by hand.
- **Decision 12's recommendation** now rests on the measured facts. A reload in a private window is not a new device. Under A, the copy itself is still in memory and is lost with a reload. The sentence on Firefox's device check, which the owner answered in Amendment 3, is gone. The recommendation's conclusion is unchanged.
- **The owner's confirmation.** Confirmed (Carsten, 2026-11-09) after NOC-58's measurement: A stays. The decision log records it in [`2026-11-09-NOC-58-decision-12-stays-a-after-the-private-window-reload-was-measured.md`](../../factory/decisions/2026-11-09-NOC-58-decision-12-stays-a-after-the-private-window-reload-was-measured.md).

## Amendment 5: the cursor and the plan check

Added on 2026-11-09 for NOC-72. #81 (NOC-69) built the view natively and found that two sentences of [Declared indexes](#declared-indexes) do not hold. pm chose option A on the coding agent's question, and the owner decided it by approving and merging #81 (Carsten, 2026-11-09). The decision log records why, in [`2026-11-09-NOC-69-a-page-after-a-cursor-is-written-key-by-key.md`](../../factory/decisions/2026-11-09-NOC-69-a-page-after-a-cursor-is-written-key-by-key.md). The sentences marked *(Amendment 5)* above follow the code in `crates/ncdb-core/src/view/query.rs`.

- **The cursor.** This RFC wrote the page after a cursor as one row-value comparison, `(priority, space_id, object_id) > (?, ?, ?)`. That gives a wrong page in two cases:
  - **An unset value in the last row.** When the cursor's last row has a `NULL` in an indexed field, a row-value comparison with `NULL` is `NULL`, not true, so the next page is empty although rows follow.
  - **A descending key.** A row value compares every key in one direction, and every index ends with ascending `(space_id, object_id)`, so an index with a descending key cannot be paged with it.

  The cursor condition is now written key by key, `NULL`-aware and per direction, nested so that SQLite walks one index, with a bound on the first key so that an ascending page seeks to the cursor.
- **The plan check.** This RFC accepted only a `SEARCH ... USING [COVERING] INDEX`. An index without an equality prefix plans its first page as `SCAN ... USING COVERING INDEX`, which reads only the page. The check now accepts a `SEARCH` or a `SCAN` on the declared index, and refuses any other index and any temporary B-tree.
- **Unchanged.** The order of values, the total order through `(space_id, object_id)`, what a cursor holds, and that the SDK offers no other query. The spike's plan in [The spike](#the-spike) was measured on the row-value shape and stays as it was recorded.

Options not taken: B, keeping the row values with ascending indexes only, which leaves a cursor on an unset value broken; C, row values where they are correct and the key-by-key condition otherwise, which means two code paths.

## Amendment 6: following RFC 0009

Added on 2026-11-09 for NOC-75. [RFC 0009](0009-sdk-api.md) defines the SDK's API, and the owner accepted every one of its decisions, S1 to S18 (Carsten, 2026-11-09). Its section [Conflicts with GENESIS.md and the other RFCs](0009-sdk-api.md#conflicts-with-genesismd-and-the-other-rfcs) names two points for this RFC. This amendment adds the sentences they need to [Declared indexes](#declared-indexes), marked *(Amendment 6)* above, and decides nothing.

- **A query in one space** (RFC 0009's [point 1](0009-sdk-api.md#conflicts-with-genesismd-and-the-other-rfcs), decision S8). Every index ended with the space id and the object id, so no query could fix a space. An index declared with `scope: "space"`, the option [RFC 0005, Amendment 3](0005-schema-and-model-lock.md#amendment-3-following-rfc-0009) adds, puts the space id first and keeps the object id last. Its equality prefix starts with the space id. The cursor condition, the bound on the first key and the plan check of [Amendment 5](#amendment-5-the-cursor-and-the-plan-check) apply to it unchanged, since they work on any key order after the equality prefix.
- **Deleted objects in indexes** (RFC 0009's [point 2](0009-sdk-api.md#conflicts-with-genesismd-and-the-other-rfcs), decision S9). This RFC did not say whether a deleted object's row stays in an index. It does not: queries skip deleted objects, and a read by id still finds them. How the rows are kept out, for example through a partial index, is the implementation's, as the layout of the tables is ([decision 2](#decisions-for-the-owner)).

## Amendment 7: following RFC 0005's references

Added on 2026-11-10 for NOC-92. [RFC 0005, Amendment 5](0005-schema-and-model-lock.md#amendment-5-a-reference-into-another-space) adds `nc.ref`, a reference into another space, and lets an index use it. Its [D19](0005-schema-and-model-lock.md#decisions-for-the-owner-d18-to-d22), which the owner decided as recommended (Carsten, 2026-11-10), chose how a reference is indexed and left the column to an amendment of this RFC. This amendment adds the sentences it needs to [Declared indexes](#declared-indexes) and [The order of values](#the-order-of-values), marked *(Amendment 7)* above, and decides nothing.

- **One column.** A reference is one column of 32 bytes, an SQLite `BLOB`: the target's space id, then its object id, each the 16 bytes RFC 0002 defines. It holds the reference as stored, whatever its state, which differs from member to member ([RFC 0004, Amendment 4](0004-merge-rules-basic-types.md#amendment-4-a-reference-into-another-space)), so every member of the space builds the same index (principle 2). An index on a reference ends with `(space_id, object_id)` of the referencing object, as every index does.
- **Its order.** The column compares by its bytes, so references sort by the target's space id and then by its object id, the same on every target.
- **Unset.** An unset reference, and a stored value that does not fit the type ([RFC 0004, decision 17](0004-merge-rules-basic-types.md#decisions-for-the-owner-15-to-20)), index as `NULL`, which sorts first ascending and last descending, as every unset field does.
- **Unchanged.** The cursor condition, the bound on the first key and the plan check of [Amendment 5](#amendment-5-the-cursor-and-the-plan-check) apply to the column as to any other: a cursor holds its 32 bytes as it holds any indexed value, and never leaves the device.
