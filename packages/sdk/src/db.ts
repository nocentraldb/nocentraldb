// The database an app opens in the browser (RFC 0009, "Open and close",
// "Write", "Get by id", "Spaces"; NOC-176 slice 1). The SDK runs the core,
// the Rust SDK compiled to WebAssembly, in a dedicated worker it starts
// itself, and speaks to it in the bridge's messages
// (crates/nocentraldb/src/bridge/mod.rs): canonical CBOR, one request and
// one answer at a time. `@nocentraldb/sdk` ships the worker script and the
// wasm module and finds them with `new URL(..., import.meta.url)` (Carsten,
// 2026-11-12, NOC-274).

import { type Cbor, decode, encode } from "./cbor.ts";
import { NcError } from "./error.ts";
import type { Infer, model } from "./nc.ts";

/** The core, as the SDK reaches it: one CBOR request, one CBOR answer. Internal; a test passes a fake. */
export interface Core {
  call(request: Uint8Array): Promise<Uint8Array>;
  terminate(): void;
}

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

/** The rule RFC 0006 sets for a browser database's name ("The browser"). */
const NAME = /^[a-z0-9_-]{1,64}$/;

const DESCRIBE = Symbol.for("nocentraldb.model.describe");

/** The worker that runs the core, shipped next to this module. */
const startWorker = (): Core => {
  const worker = new Worker(new URL("./worker.js", import.meta.url), {
    type: "module",
    name: "nocentraldb",
  });
  const waiting: ((answer: Uint8Array) => void)[] = [];
  let failed: Error | undefined;
  const failing: ((error: Error) => void)[] = [];
  worker.onmessage = (event: MessageEvent<Uint8Array>) => {
    failing.shift();
    waiting.shift()?.(event.data);
  };
  worker.onerror = (event) => {
    failed = new NcError(
      "storageFailed",
      `the worker failed: ${event.message}`,
    );
    for (const fail of failing.splice(0)) fail(failed);
    waiting.length = 0;
  };
  return {
    call: (request) =>
      new Promise((resolve, reject) => {
        if (failed) return reject(failed);
        waiting.push(resolve);
        failing.push(reject);
        worker.postMessage(request, [request.buffer]);
      }),
    terminate: () => worker.terminate(),
  };
};

/** Opens this member's database in the browser. */
export const open = async (options: OpenOptions): Promise<Db> => {
  checkName(options.name);
  return connect(startWorker(), options);
};

const checkName = (name: string | undefined) => {
  if (name !== undefined && !NAME.test(name)) {
    throw new NcError(
      "invalidValue",
      "a database name is 1 to 64 characters of a-z, 0-9, - and _",
    );
  }
};

/** What the lock says of the model's collections: each field's type. */
type Described = Record<
  string,
  { fields: Record<string, { type: string; of?: { type: string } }> }
>;

/** Opens the database through `core`. Internal: `open` passes the worker, a test a fake. */
export const connect = async (
  core: Core,
  options: OpenOptions,
): Promise<Db> => {
  try {
    checkName(options.name);
  } catch (error) {
    core.terminate();
    throw error;
  }
  const names = new Map<object, string>();
  for (const [name, collection] of Object.entries(options.model)) {
    names.set(collection, name);
  }
  const describe = (
    options.model as { [DESCRIBE]?: () => { collections: Described } }
  )[DESCRIBE];
  const described: Described = describe ? describe().collections : {};

  let next = 0;
  let closed = false;
  const ask = async (
    call: string,
    args: Record<string, unknown>,
  ): Promise<Cbor> => {
    if (closed) throw new NcError("closed", "the database is closed");
    next += 1;
    const answer = decode(
      await core.call(encode({ id: next, call, args })),
    ) as {
      ok?: Cbor;
      error?: {
        code: string;
        message: string;
        ids?: string[];
        fingerprint?: string;
      };
    };
    if (answer.error) {
      const { code, message, ids, fingerprint } = answer.error;
      throw new NcError(code, message, ids ?? [], fingerprint);
    }
    return answer.ok ?? null;
  };

  try {
    await ask("open", {
      lock: JSON.stringify(options.lock),
      name: options.name ?? "default",
      relay: options.relay,
    });
  } catch (error) {
    core.terminate();
    throw error;
  }

  const nameOf = (collection: object): string => {
    const name = names.get(collection);
    if (name === undefined) {
      throw new NcError(
        "notFound",
        "the model has no such collection; pass a handle such as model.tasks",
      );
    }
    return name;
  };

  const value = (
    collection: string,
    field: string,
    written: unknown,
  ): unknown => {
    if (written === null) return null;
    if (written instanceof SetElements) {
      const elements = written.elements.map((e) =>
        element(collection, field, e, true),
      );
      return written.add ? { $add: elements } : { $remove: elements };
    }
    return element(collection, field, written, false);
  };

  const element = (
    collection: string,
    field: string,
    written: unknown,
    inSet: boolean,
  ): unknown => {
    const type = described[collection]?.fields[field];
    const kind = inSet ? (type?.of?.type ?? type?.type) : type?.type;
    if (
      kind === "ncdb/int" &&
      !(typeof written === "number" && Number.isSafeInteger(written))
    ) {
      throw new NcError(
        "invalidValue",
        `${collection}.${field} takes a safe integer, between -(2^53 - 1) and 2^53 - 1`,
      );
    }
    if (typeof written === "string") return written.normalize("NFC");
    return written;
  };

  const values = (collection: string, written: Record<string, unknown>) => {
    const out: Record<string, unknown> = {};
    for (const [field, v] of Object.entries(written)) {
      if (v !== undefined) out[field] = value(collection, field, v);
    }
    return out;
  };

  const space = (id: Id): Space => {
    const write = async (record: (tx: Tx) => void): Promise<Written> => {
      if (closed) throw new NcError("closed", "the database is closed");
      const ops: Record<string, unknown>[] = [];
      const tx: Tx = {
        create: (collection, written) => {
          const name = nameOf(collection);
          const created = randomId();
          ops.push({
            op: "create",
            collection: name,
            id: created,
            values: values(name, written),
          });
          return created;
        },
        update: (collection, object, written) => {
          const name = nameOf(collection);
          ops.push({
            op: "update",
            collection: name,
            id: object,
            values: values(name, written),
          });
        },
        delete: (collection, object) => {
          ops.push({
            op: "delete",
            collection: nameOf(collection),
            id: object,
          });
        },
        restore: (collection, object) => {
          ops.push({
            op: "restore",
            collection: nameOf(collection),
            id: object,
          });
        },
      };
      record(tx);
      return (await ask("write", { space: id, ops })) as unknown as Written;
    };
    return {
      id,
      write,
      create: (collection, written) =>
        write((tx) => void tx.create(collection, written)),
      update: (collection, object, written) =>
        write((tx) => tx.update(collection, object, written)),
      delete: (collection, object) =>
        write((tx) => tx.delete(collection, object)),
      restore: (collection, object) =>
        write((tx) => tx.restore(collection, object)),
      get: async (collection, object) => {
        if (closed) throw new NcError("closed", "the database is closed");
        const answer = (await ask("get", {
          space: id,
          collection: nameOf(collection),
          id: object,
        })) as {
          fields: Record<string, unknown>;
        } | null;
        if (answer === null) return undefined;
        const { fields, ...rest } = answer;
        return { ...rest, ...fields } as never;
      },
    };
  };

  return {
    spaces: {
      create: async () => space((await ask("spaces.create", {})) as string),
      get: async (id) =>
        space((await ask("spaces.get", { space: id })) as string),
      list: async () =>
        (await ask("spaces.list", {})) as unknown as SpaceEntry[],
    },
    close: async () => {
      if (closed) return;
      try {
        await ask("close", {});
      } finally {
        closed = true;
        core.terminate();
      }
    },
  };
};

/** 128 random bits from the browser's secure generator, as 32 hex digits (RFC 0002, "Object ids"). */
const randomId = (): Id => {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  return [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");
};

class SetElements {
  constructor(
    readonly add: boolean,
    readonly elements: unknown[],
  ) {}
}

/** Adds `elements` to a set or to many links (RFC 0009, "Field operations per field type"). */
export const add = <E>(...elements: E[]): Elements<E> =>
  new SetElements(true, elements) as unknown as Elements<E>;

/** Removes `elements` from a set or from many links: the additions of each that the copy holds. */
export const remove = <E>(...elements: E[]): Elements<E> =>
  new SetElements(false, elements) as unknown as Elements<E>;
