// The database an app opens in the browser (RFC 0009, "Open and close",
// "Write", "Get by id", "Spaces"; NOC-176 slice 1). The SDK runs the core,
// the Rust SDK compiled to WebAssembly, in a dedicated worker it starts
// itself, and speaks to it in the bridge's messages
// (crates/nocentraldb/src/bridge/mod.rs): canonical CBOR, one request and
// one answer at a time. `@nocentraldb/sdk` ships the worker script and the
// wasm module and finds them with `new URL(..., import.meta.url)` (Carsten,
// 2026-11-12, NOC-274).

import { connectWorker } from "./connect.ts";
import { SetElements } from "./elements.ts";
import type { Infer, model } from "./nc.ts";

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
  /** Closes the database; every later call fails with `closed`. */
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
