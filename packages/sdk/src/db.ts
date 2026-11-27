// The database an app opens in the browser (RFC 0009, "Open and close",
// "Write", "Get by id", "Queries and indexes", "Subscribe", "Spaces";
// NOC-176 slices 1 and 2). The SDK runs the core,
// the Rust SDK compiled to WebAssembly, in a dedicated worker it starts
// itself, and speaks to it in the bridge's messages
// (crates/nocentraldb/src/bridge/mod.rs): canonical CBOR, one request and
// one answer at a time. `@nocentraldb/sdk` ships the worker script and the
// wasm module and finds them with `new URL(..., import.meta.url)` (Carsten,
// 2026-11-12, NOC-274).

import type { RecoveryCode } from "./account.ts";
import { connectWorker } from "./connect.ts";
import type { Durability, ViewStatus } from "./durability.ts";
import { SetElements } from "./elements.ts";
import type { Invitation, Members } from "./members.ts";
import type { Infer, model } from "./nc.ts";
import type { SpaceStatus } from "./status.ts";
import type { SyncStatus } from "./sync.ts";

/** A collection handle of the model, such as `model.tasks`. */
type AnyCollection = Parameters<typeof model>[0][string];

/** An object id or a space id: 16 bytes as 32 lowercase hex digits (RFC 0009, "Get by id"). */
type Id = string;

/** Elements to add to, or remove from, a set or many links: what `nc.add` and `nc.remove` give. */
interface Elements<E> {
  readonly "nc.elements": E;
}

/** What a write takes for a field that reads as `T`: the value, `null` to clear it, or for a set or many links `nc.add` / `nc.remove`. */
type Write<T> = [T] extends [readonly (infer E)[] | undefined]
  ? Elements<E>
  : Exclude<T, undefined> | null;

/** The values a create or an update writes, by field name; `undefined` writes nothing. */
type Values<C> = C extends AnyCollection
  ? { [K in keyof Infer<C>]?: Write<Infer<C>[K]> | undefined }
  : never;

/** An object as `space.get` reads it (RFC 0009, "Get by id"). */
type Read<C> = C extends AnyCollection
  ? Infer<C> & {
      readonly $id: Id;
      readonly $space: Id;
      readonly $deleted: boolean;
    }
  : never;

/** Where the page after another starts. It holds the page's last position on this device and prints as `[redacted]`; another database refuses it with `cursorOfAnotherModel` (RFC 0009, "Queries and indexes"). */
interface Cursor {
  toString(): string;
}

/**
 * A query of a declared index, whose objects are `T`: the handle
 * `model.<collection>.indexes.<index>`, with `.where` fixing its `where`
 * fields (`E`, their values in index order), `.take` the page size (50 by
 * default, at most 1000) and `.after` where the page starts (RFC 0009,
 * "Queries and indexes"; RFC 0018, R3). Each returns a new query.
 */
export interface Query<T, E extends readonly unknown[] = readonly []> {
  /** Fixes the index's `where` fields, one value each, in their order. */
  where(...values: E): Query<T>;
  /** The same call as `where`, for an index declared with `equal`. */
  equal(...values: E): Query<T>;
  /** Sets the page size: 50 by default, at most 1000. */
  take(n: number): Query<T, E>;
  /** Starts after the cursor of an earlier page. */
  after(cursor: Cursor): Query<T, E>;
  /** Brands the query with its objects' type. */
  readonly "nc.query"?: T;
}

/** One page of a query: its objects in index order, the cursor after the last one when more may follow, and whether more do. */
export interface Page<T> {
  readonly items: T[];
  readonly cursor: Cursor | undefined;
  readonly hasMore: boolean;
}

/** Stops a subscription. */
type Stop = () => void;

/** What one fold touched in one collection (RFC 0009, "Subscribe"): ids, never values, and whether the change was this member's own write. */
interface Change {
  readonly space: Id;
  readonly collection: string;
  readonly ids: readonly Id[];
  readonly origin: "local" | "remote";
}

/** What a write answers: its change id and the ids of the objects it created, in order. */
interface Written {
  /** The change's id; for a write split into several changes, the last part's, which depends on every other part. */
  readonly change: string;
  readonly ids: readonly Id[];
}

/** A transaction: the writes one `space.write` records and turns into one change (RFC 0009, "One write, one change"). */
export interface Tx {
  /** Records the creation of an object and returns its new id, drawn on the device. */
  create<C extends AnyCollection>(collection: C, values: Values<C>): Id;
  /** Records a write of `values` to the object `id`. */
  update<C extends AnyCollection>(
    collection: C,
    id: Id,
    values: Values<C>,
  ): void;
  /** Records the deletion of the object `id`. */
  delete(collection: AnyCollection, id: Id): void;
  /** Records the restore of the deleted object `id`. */
  restore(collection: AnyCollection, id: Id): void;
}

/** A space: its objects are read and written through it (RFC 0009, "Spaces"). */
export interface Space {
  /** The space id, 32 lowercase hex digits. */
  readonly id: Id;
  /** The members of this space: add, list, re-role and remove (RFC 0009, "Invitations and members"). */
  readonly members: Members;
  /** In an ad-hoc space this database created: the space and id of the object `copy` was copied from, or `undefined` on any other database; the mapping never syncs (RFC 0009, "Sub-agents and ad-hoc spaces"; Carsten, 2026-11-16, NOC-351). */
  sourceOf(
    copy: Id,
  ): Promise<{ readonly space: Id; readonly id: Id } | undefined>;
  /** In an ad-hoc space this database created: the copy of the object `source` of `sourceSpace`, or `undefined` when this database copied no such object into it. */
  copyOf(sourceSpace: Id, source: Id): Promise<Id | undefined>;
  /** Leaves the space: this member writes its own removal and keeps its copy; the last admin is refused with `lastAdmin` (RFC 0009, "Spaces"). */
  leave(): Promise<{ readonly notice: import("./members.ts").Notice }>;
  /** The audit of this space's erasures; every member may read it, a reader too (RFC 0009, "Erasures"). */
  readonly erasures: {
    /** Each erasure, with who deleted the person key, who only saw it and who never answered. */
    list(): Promise<ErasureEntry[]>;
    /** Takes the provisional erasure `shred` back within its 7 days, and gives `takenBack`: only an admin of the performer's account may, others are refused with `notAllowed`; a shred that is final, decided or not of this space is refused with `invalidValue` (RFC 0009, "Erasing a person"; Carsten, 2026-11-15, NOC-326). */
    takeBack(shred: string): Promise<import("./members.ts").Notice>;
  };
  /** What this space holds that a read does not show, as a report (RFC 0009, "Sync and status"). */
  readonly status: {
    /** The report as it is now. */
    current(): SpaceStatus;
    /** Calls `listener` with the report at once, and again whenever it changes. */
    subscribe(listener: (status: SpaceStatus) => void): Stop;
  };
  /**
   * Records the writes of `record` and turns them into one change of this space.
   *
   * A write whose change would exceed the relay's limit of 16 MiB is split, between objects, into several changes, each within the limit (NOC-431). On this device it stays atomic: every part is folded in one transaction. **On other devices it is not atomic**: the parts arrive and fold one by one, so another member can see part of the write before the rest arrives, and a part the relay refuses leaves the others standing. One operation, or the operations of one object, that do not fit into one change are refused with `tooLarge`, before anything is written. A write well below the limit, such as a few hundred objects, is always one change.
   */
  write(record: (tx: Tx) => void): Promise<Written>;
  /** A write with one create. */
  create<C extends AnyCollection>(
    collection: C,
    values: Values<C>,
  ): Promise<Written>;
  /** A write with one update. */
  update<C extends AnyCollection>(
    collection: C,
    id: Id,
    values: Values<C>,
  ): Promise<Written>;
  /** A write with one delete. */
  delete(collection: AnyCollection, id: Id): Promise<Written>;
  /** A write with one restore. */
  restore(collection: AnyCollection, id: Id): Promise<Written>;
  /** The object `id` of `collection`, deleted or not, or `undefined` when the copy holds none. */
  get<C extends AnyCollection>(
    collection: C,
    id: Id,
  ): Promise<Read<C> | undefined>;
  /** A page of a query of an index with `scope: "space"`, in this space. */
  find<T>(query: Query<T, readonly unknown[]>): Promise<Page<T>>;
  /** Follows a query in this space: its page at once, and again after every fold in this space that touched its collection. */
  subscribe<T>(
    query: Query<T, readonly unknown[]>,
    listener: (page: Page<T>) => void,
  ): Stop;
  /** Follows the object `id` of `collection`: as `get` reads it, at once and after every fold in this space that touched the collection. */
  subscribe<C extends AnyCollection>(
    collection: C,
    id: Id,
    listener: (object: Read<C> | undefined) => void,
  ): Stop;
}

/** One erasure of a space, as `space.erasures.list()` gives it (RFC 0009, "Erasures"; Carsten, 2026-11-12, NOC-246). */
interface ErasureEntry {
  /** The change id of the erasure's shred entry. */
  readonly shred: string;
  /** `"final"` for a shred, or a provisional one a decision finalized; `"provisional"` until a decision ends it; `"takenBack"` when its account took it back; `"waiting"` while this copy lacks part of the shred's causal past, with no members until the gap fills (Carsten, 2026-11-20, NOC-392). */
  readonly state: "final" | "provisional" | "takenBack" | "waiting";
  /** The member that performed it, and its account's fingerprint. */
  readonly performer: { readonly id: string; readonly account: string };
  /** The fingerprint of the account that signed it off. */
  readonly signOff: string;
  /** Every member that could have held the person key; none while the erasure is waiting. */
  readonly members: readonly {
    readonly id: string;
    readonly account: string;
    /** Whether it is still a member. */
    readonly current: boolean;
    /** Whether it confirmed it deleted the key, only saw the shred, or never answered. */
    readonly acknowledged: "deleted" | "seen" | "missing";
  }[];
}

/** A space as `db.spaces.list` gives it. */
interface SpaceEntry {
  readonly id: Id;
  readonly role: string | null;
  /** Whether the space is of this database's model, or of another (RFC 0009, "Spaces of another model"). */
  readonly model: "ours" | "another";
}

/** This member's database (RFC 0009, "Open and close"). */
export interface Db {
  readonly spaces: {
    /** Creates a space, offline if need be; this member is its first admin. */
    create(): Promise<Space>;
    /** The space `id`; `notFound` when the copy holds none. */
    get(id: Id): Promise<Space>;
    /** Every space of this member, with its role. */
    list(): Promise<SpaceEntry[]>;
    /** Calls `listener` with the list, then with each new list when a space is added, left or removed, or its role changes (RFC 0009, "Spaces"). */
    subscribe(listener: (spaces: SpaceEntry[]) => void): Stop;
    /**
     * Creates an ad-hoc space for a sub-agent: a space with a fresh key, which syncs as any other, holding a copy of each object of `copy` as a new object with a fresh id (RFC 0009, "Sub-agents and ad-hoc spaces"; Carsten, 2026-11-16, NOC-351).
     * `copy` takes objects this database read, as `space.get`, `find` and `subscribe` give them; any other object, a spread of one too, is refused with `invalidValue`.
     * A copied link points to the copy of its target, or is cleared; personal and plugin fields are not copied. `personal` and `local` are refused with `notAllowed` for now.
     * Gives the space, its `adHocSpaceCreated` notice, each source with its copy in the order of `copy`, and how many links were cleared and plugin fields not copied.
     */
    createAdHoc(options: {
      copy: readonly object[];
      personal?: boolean;
      local?: boolean;
    }): Promise<{
      readonly space: Space;
      readonly notice: import("./members.ts").Notice;
      readonly copies: readonly { readonly source: Id; readonly copy: Id }[];
      readonly clearedLinks: number;
      readonly pluginFieldsNotCopied: number;
    }>;
  };
  /** A page of a query of an index across every space of the member. */
  find<T>(query: Query<T, readonly unknown[]>): Promise<Page<T>>;
  /** Follows a query: its page at once, and again after every fold that touched its collection, whole pages, never diffs (RFC 0009, "Subscribe"). */
  subscribe<T>(
    query: Query<T, readonly unknown[]>,
    listener: (page: Page<T>) => void,
  ): Stop;
  /** The change event: after each fold, its space, collection and the ids it touched, and its origin (RFC 0009, "Subscribe"). */
  readonly changes: {
    /** Calls `listener` after each fold, once per collection it touched. */
    subscribe(listener: (change: Change) => void): Stop;
  };
  /** This member's invitation, with its account's fingerprint, for an admin to add it to a space (RFC 0009, "Invitations and members"). */
  invitation(): Promise<Invitation>;
  /** The relay connection, as a report (RFC 0009, "Sync and status"). */
  readonly sync: {
    /** The report as it is now. */
    current(): SyncStatus;
    /** Calls `listener` with the report at once, and again whenever it changes. */
    subscribe(listener: (status: SyncStatus) => void): Stop;
    /** Stops syncing, for an app that wants to stay offline; the connection reads `"paused"` until `resume`. */
    pause(): Promise<void>;
    /** Syncs again after `pause`. */
    resume(): Promise<void>;
    /** Whether the relay holds the change `change`, as `write` answered its id. */
    isConfirmed(change: string): Promise<boolean>;
    /** Resolves when the relay holds the change `change`. It may never resolve: the relay can refuse the change, or never be reached. */
    confirmed(change: string): Promise<void>;
  };
  /** What this copy would lose if the browser deleted it now, as a report (RFC 0006, "What the SDK reports"). */
  readonly durability: {
    /** The report as it is now. */
    current(): Durability;
    /** Calls `listener` with the report at once, and again whenever it changes. */
    subscribe(listener: (durability: Durability) => void): Stop;
    /** Asks the browser for persistent storage and answers its answer; the new storage is also reported through `subscribe`. */
    requestPersistent(): Promise<boolean>;
  };
  /** The view's report: whether a rebuild runs, and how far it is (RFC 0009, "Sync and status"). */
  readonly view: {
    /** The report as it is now. */
    current(): ViewStatus;
    /** Calls `listener` with the report at once, and again whenever it changes. */
    subscribe(listener: (view: ViewStatus) => void): Stop;
  };
  /** This member's account: its recovery code, and whether this open recovered it (RFC 0009, "Accounts and devices"). */
  readonly account: {
    /** The account's identity, its recovery key in hexadecimal: public, the same on every device and agent of the account and after a recovery, and what the app's backend signs an admission for (RFC 0015, "In the SDK"). `null` while the database waits to be paired. */
    readonly identity: string | null;
    /** The recovery code while the person has not confirmed they stored it, else `null`; shown again on every open until then. */
    recoveryCode(): Promise<RecoveryCode | null>;
    /** `accountRecovered` on the open that recovered the account with `recover`, else `null`. */
    recovered(): Promise<import("./members.ts").Notice | null>;
    /** On a device of the account, after a person confirmed the new device: the answer to show it, the spaces the new device was added to, each with its notice, and the spaces whose admins the app asks (RFC 0010). `kind` is `"device"` or `"agent"`. */
    pair(request: {
      kind: "device" | "agent";
      invitation: Invitation;
    }): Promise<{
      readonly answer: import("./account.ts").PairingAnswer;
      readonly added: readonly {
        readonly space: Id;
        readonly notice: import("./members.ts").Notice;
      }[];
      readonly askAdmins: readonly Id[];
    }>;
    /** On the new database, opened with `pairing: "request"`: takes the answer and makes it a device or an agent of the account; gives the notices of the pairing. */
    completePairing(
      answer: import("./account.ts").PairingAnswer,
    ): Promise<{ readonly notices: readonly import("./members.ts").Notice[] }>;
    /** On a device of the account, with the recovery code the person typed: removes a lost device or an agent of the account from every space this device administers, and renews the account key so that the lost one's is no longer accepted (RFC 0010, "Losing a device"). Gives the spaces it was removed from, each with its notice, the spaces whose admins the app asks, and `accountKeyRenewed`. The words are never logged. */
    removeMember(
      member: string,
      options: { recover: string },
    ): Promise<{
      readonly removed: readonly {
        readonly space: Id;
        readonly notice: import("./members.ts").Notice;
      }[];
      readonly askAdmins: readonly Id[];
      readonly notice: import("./members.ts").Notice;
    }>;
    /** On a device of another account, or of the performer's own where every member of a space belongs to it: signs off `request` with the account key in each space where the account may, checked against this copy, and names the others in `notSigned`. A database that holds no account key is refused with `notAllowed` (RFC 0009, "Erasing a person"). */
    signOff(request: import("./erase.ts").ErasureRequest): Promise<{
      readonly signOffs: import("./erase.ts").SignOffs;
      readonly notSigned: readonly Id[];
    }>;
  };
  /** The request to erase the person of `object`, one of the person's objects as this database read it: every space where this member may write and the person has a key. An object of no person is refused with `noPerson` (RFC 0009, "Erasing a person"; Carsten, 2026-11-15, NOC-326). */
  erasureRequest(object: object): Promise<import("./erase.ts").ErasureRequest>;
  /** Erases the person of `request` in every space `signOffs` signs off: one shred per space, each sign-off checked first; one that would not hold refuses the call with `invalidValue`, and nothing is written. Erasure does not reach devices that never come back online, copies made outside the database or unmarked free text, as its notice `erased` says. */
  erase(
    request: import("./erase.ts").ErasureRequest,
    signOffs: import("./erase.ts").SignOffs,
  ): Promise<import("./erase.ts").Erased>;
  /** Closes the database; every later call fails with `closed`, and every subscription stops. */
  close(): Promise<void>;
}

/** The options of `nc.open` in a browser (RFC 0009, "Open and close"). */
interface OpenOptions {
  /** The TypeScript model, `nc.model(...)`: types and handles. */
  // biome-ignore lint/suspicious/noExplicitAny: any model
  model: Record<string, any>;
  /** `model.lock.json`: the rules the core folds by. */
  lock: unknown;
  /** The relay's URL, or a function that gives it; without it, the database stays local. A token for the relay belongs in the query string, never in the path, since the sync status shows the path (NOC-495). The SDK calls a function before each connect, so a short-lived token is renewed when it reconnects (NOC-496): while the function throws or gives no `ws://` or `wss://` URL, `db.sync.current().error` is `relay_url_failed` and the SDK calls it again after a backoff. */
  relay?: string | (() => Promise<string>);
  /** Which database: 1 to 64 of a–z, 0–9, `-` and `_`; `"default"` when left out. */
  name?: string;
  /** `"on-open"`, the default: ask the browser for persistent storage on open, without waiting; `"on-request"`: only when the app calls `db.durability.requestPersistent()` (RFC 0006). */
  persistentStorage?: "on-open" | "on-request";
  /** What `nc.open` does when this browser had the database open before and the browser has since dropped its files, as Chromium does with a private window's OPFS after ten minutes with no database open: `"refuse"`, the default, fails with `storageLost`; `"open-empty"` opens an empty copy under the same member keys (RFC 0006, NOC-445). */
  ifStorageLost?: "refuse" | "open-empty";
  /** `"request"`: open a new database to be paired to an account; its invitation carries no certificate yet (RFC 0010). */
  pairing?: "request";
  /** The twelve words of a recovery code, as the person typed them: recovers the account on a new database (RFC 0010). Never logged. */
  recover?: string;
  /** Fetches an admission from the app's backend for the account's identity, `db.account.identity`, and answers its text, `ncdb-admit-1:...` (RFC 0015, "In the SDK"). The SDK calls it when the relay refuses a new space, presents the answer and pushes the space again, once; a function that throws leaves the space refused. The app handles a string, never a key. */
  admission?: (identity: string) => Promise<string>;
}

/** Opens this member's database in the browser. */
export const open = (options: OpenOptions): Promise<Db> =>
  connectWorker(options);

/** Adds `elements` to a set or to many links (RFC 0009, "Field operations per field type"). */
export const add = <E>(...elements: E[]): Elements<E> =>
  new SetElements(true, elements) as unknown as Elements<E>;

/** Removes `elements` from a set or from many links: the additions of each that the copy holds. */
export const remove = <E>(...elements: E[]): Elements<E> =>
  new SetElements(false, elements) as unknown as Elements<E>;
