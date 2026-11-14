// The database an app opens in the browser (RFC 0009, "Open and close",
// "Write", "Get by id", "Queries and indexes", "Subscribe", "Spaces";
// NOC-176 slices 1 and 2). The SDK runs the core,
// the Rust SDK compiled to WebAssembly, in a dedicated worker it starts
// itself, and speaks to it in the bridge's messages
// (crates/nocentraldb/src/bridge/mod.rs): canonical CBOR, one request and
// one answer at a time. `@nocentraldb/sdk` ships the worker script and the
// wasm module and finds them with `new URL(..., import.meta.url)` (Carsten,
// 2026-11-12, NOC-274).

import { connectWorker } from "./connect.ts";
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
 * `model.<collection>.indexes.<index>`, with `.equal` fixing its equality
 * prefix (`E`, its values in index order), `.take` the page size (50 by
 * default, at most 1000) and `.after` where the page starts (RFC 0009,
 * "Queries and indexes"). Each returns a new query.
 */
export interface Query<T, E extends readonly unknown[] = readonly []> {
  /** Fixes the index's leading fields, as many as its `equal` declares. */
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
  /** The audit of this space's erasures; every member may read it, a reader too (RFC 0009, "Erasures"). */
  readonly erasures: {
    /** Each erasure, with who deleted the person key, who only saw it and who never answered. */
    list(): Promise<ErasureEntry[]>;
  };
  /** What this space holds that a read does not show, as a report (RFC 0009, "Sync and status"). */
  readonly status: {
    /** The report as it is now. */
    current(): SpaceStatus;
    /** Calls `listener` with the report at once, and again whenever it changes. */
    subscribe(listener: (status: SpaceStatus) => void): Stop;
  };
  /** Records the writes of `record` and turns them into one change of this space. */
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
  /** `"final"` for a shred, or a provisional one a decision finalized; `"provisional"` while it waits; `"takenBack"` when its account took it back. */
  readonly state: "final" | "provisional" | "takenBack";
  /** The member that performed it, and its account's fingerprint. */
  readonly performer: { readonly id: string; readonly account: string };
  /** The fingerprint of the account that signed it off. */
  readonly signOff: string;
  /** Every member that could have held the person key. */
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
  /** The relay's URL; without it, the database stays local. */
  relay?: string;
  /** Which database: 1 to 64 of a–z, 0–9, `-` and `_`; `"default"` when left out. */
  name?: string;
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
