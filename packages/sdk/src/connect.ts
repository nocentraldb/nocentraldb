// The implementation behind `nc.open` (NOC-176 slices 1 and 2): the worker
// that runs the core, and the database the SDK builds on any core that
// answers the bridge's protocol (crates/nocentraldb/src/bridge/mod.rs).
// Subscriptions re-run their read after the core's `folded` events, which
// name the space and collections of each fold, never values. Internal:
// the package's entry does not reach it, so it is not in the API report;
// the vitest tests pass a fake core to `connect`.

import { type Cbor, decode, encode } from "./cbor.ts";
import type { Db, open, Page, Space, Tx } from "./db.ts";
import { SetElements } from "./elements.ts";
import { NcError } from "./error.ts";
import { Cursor, queryState } from "./query.ts";
import type { SpaceStatus } from "./status.ts";
import type { SyncStatus } from "./sync.ts";
import { type Platform, tabCore } from "./tabs.ts";

/** The core, as the SDK reaches it: one CBOR request, one CBOR answer, and the events it sends unasked. Internal; a test passes a fake. */
export interface Core {
  call(request: Uint8Array): Promise<Uint8Array>;
  terminate(): void;
  /** Gives the core's events, such as `{event: "folded", folds}`, to `listener`. */
  listen(listener: (event: Uint8Array) => void): void;
}

type OpenOptions = Parameters<typeof open>[0];
type Written = Awaited<ReturnType<Space["write"]>>;
type SpaceEntry = Awaited<ReturnType<Db["spaces"]["list"]>>[number];
type Id = string;

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
  let listener: ((event: Uint8Array) => void) | undefined;
  // The worker sends `{answer}` for each request, in order, and `{event}`
  // for what the core reports unasked.
  worker.onmessage = (
    message: MessageEvent<{ answer?: Uint8Array; event?: Uint8Array }>,
  ) => {
    const { answer, event } = message.data;
    if (event !== undefined) {
      listener?.(event);
      return;
    }
    failing.shift();
    if (answer !== undefined) waiting.shift()?.(answer);
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
    listen: (given) => {
      listener = given;
    },
  };
};

/** Opens the database in the worker the package ships, or, when another tab owns it, through that tab (NOC-177; RFC 0006, "The browser"). */
export const connectWorker = async (options: OpenOptions): Promise<Db> => {
  checkName(options.name);
  const core =
    typeof navigator !== "undefined" &&
    navigator.locks !== undefined &&
    typeof BroadcastChannel !== "undefined"
      ? await tabCore(options.name ?? "default", {
          locks: navigator.locks as unknown as Platform["locks"],
          channel: (name) =>
            new BroadcastChannel(name) as unknown as ReturnType<
              Platform["channel"]
            >,
          startWorker,
        })
      : startWorker();
  return connect(core, options);
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

  let opened: Cbor;
  try {
    opened = await ask("open", {
      lock: JSON.stringify(options.lock),
      name: options.name ?? "default",
      relay: options.relay,
    });
  } catch (error) {
    core.terminate();
    throw error;
  }

  // The newest sync status the core sent: at open, then in its `sync`
  // events, so that `db.sync.current()` answers at once.
  // The relay that serves this database: this tab's option, or, when
  // another tab owns the database, that tab's (NOC-177).
  let relay = (opened as { relay?: string } | null)?.relay ?? options.relay;
  const statusOf = (raw: unknown): SyncStatus => {
    const given = (raw ?? {}) as Partial<Record<keyof SyncStatus, unknown>>;
    return {
      connection: (given.connection ?? "none") as SyncStatus["connection"],
      ...(relay === undefined ? {} : { relay }),
      unconfirmed: Number(given.unconfirmed ?? 0),
      refused: Number(given.refused ?? 0),
      epochChanged: given.epochChanged === true,
      ...(typeof given.error === "string" ? { error: given.error } : {}),
    };
  };
  let sync = statusOf((opened as { sync?: unknown } | null)?.sync);
  const syncListeners = new Set<(status: SyncStatus) => void>();
  // Who waits for the next sync status: `db.sync.confirmed`.
  const syncWaiters = new Set<{
    resolve: () => void;
    reject: (error: unknown) => void;
  }>();
  const nextSync = () => {
    let waiter!: { resolve: () => void; reject: (error: unknown) => void };
    const promise = new Promise<void>((resolve, reject) => {
      waiter = { resolve, reject };
    });
    syncWaiters.add(waiter);
    return { promise, cancel: () => syncWaiters.delete(waiter) };
  };
  const synced = (raw: unknown) => {
    sync = statusOf(raw);
    for (const listener of [...syncListeners]) {
      try {
        listener(sync);
      } catch {
        // A listener that throws loses this status, and the others still
        // get it.
      }
    }
    for (const waiter of [...syncWaiters]) {
      syncWaiters.delete(waiter);
      waiter.resolve();
    }
  };
  const isConfirmed = async (change: string) =>
    (await ask("sync.isConfirmed", { change })) === true;

  // Each space's newest status and who follows it, shared by every handle
  // of the space: the core answers `space.status` with it, and sends a
  // `spaceStatus` event whenever it changes.
  const statuses = new Map<
    Id,
    { status: SpaceStatus; listeners: Set<(status: SpaceStatus) => void> }
  >();
  const spaceStatusOf = (raw: unknown): SpaceStatus => {
    const given = (raw ?? {}) as Record<string, unknown>;
    const list = <T>(value: unknown) =>
      (Array.isArray(value) ? value : []) as T[];
    const counts = (value: unknown) =>
      Object.fromEntries(
        Object.entries((value ?? {}) as Record<string, unknown>).map(
          ([reason, count]) => [reason, Number(count)],
        ),
      );
    return {
      role: (given.role ?? "removed") as SpaceStatus["role"],
      initialSyncComplete: given.initialSyncComplete === true,
      behind: Number(given.behind ?? 0),
      gaps: list<string>(given.gaps),
      parked: counts(given.parked),
      ...(typeof given.newestModel === "number"
        ? { newestModel: given.newestModel }
        : {}),
      rejected: counts(given.rejected),
      refused: list<{ change: string; code: string }>(given.refused),
      lostGrants: list<string>(given.lostGrants),
      equivocations: list<string>(given.equivocations),
      exposed: given.exposed === true,
      waitingForKey: given.waitingForKey === true,
      badPersonKeyRecords: list<string>(given.badPersonKeyRecords),
      provisionalErasures: list<SpaceStatus["provisionalErasures"][number]>(
        given.provisionalErasures,
      ).map((erasure) => ({ ...erasure, age: Number(erasure.age) })),
    };
  };
  const statusChanged = (id: Id, raw: unknown) => {
    const entry = statuses.get(id);
    if (entry === undefined) return;
    entry.status = spaceStatusOf(raw);
    for (const listener of [...entry.listeners]) {
      try {
        listener(entry.status);
      } catch {
        // A listener that throws loses this status, and the others still
        // get it.
      }
    }
  };
  /** A handle of the space `id`, with its status, which the core follows from now on. */
  const handleOf = async (id: Id): Promise<Space> => {
    const raw = await ask("space.status", { space: id });
    const entry = statuses.get(id);
    if (entry === undefined) {
      statuses.set(id, { status: spaceStatusOf(raw), listeners: new Set() });
    } else {
      entry.status = spaceStatusOf(raw);
    }
    return space(id);
  };

  // The identity of this database, which its cursors carry.
  const identity = {};
  const subscriptions = new Set<Subscription>();
  const changeListeners = new Set<(change: Change) => void>();
  let pending: Fold[] = [];
  core.listen((bytes) => {
    if (closed) return;
    const message = decode(bytes) as {
      event?: string;
      folds?: Fold[];
      status?: unknown;
    };
    if (message.event === "sync") {
      synced(message.status);
      return;
    }
    if (message.event === "spaceStatus") {
      statusChanged((message as { space: Id }).space, message.status);
      return;
    }
    if (message.event === "reopened") {
      // Another tab took the database over (NOC-177): its core follows no
      // space yet and its reads may differ, so everything is read again.
      relay = (message as { relay?: string }).relay;
      synced((message as { sync?: unknown }).sync);
      for (const id of statuses.keys()) {
        ask("space.status", { space: id }).then(
          (raw) => statusChanged(id, raw),
          () => {},
        );
      }
      for (const subscription of subscriptions) rerun(subscription);
      return;
    }
    if (message.event !== "folded" || !message.folds) return;
    for (const fold of message.folds) {
      for (const changed of fold.changes ?? []) {
        const change: Change = {
          space: fold.space,
          collection: changed.collection,
          ids: changed.ids,
          origin: changed.origin,
        };
        for (const listener of [...changeListeners]) {
          try {
            listener(change);
          } catch {
            // A listener that throws loses this change, and the others
            // still get it.
          }
        }
      }
    }
    if (pending.length === 0) queueMicrotask(flush);
    pending.push(...message.folds);
  });
  // Re-runs, once, every subscription a batch of folds may have changed.
  const flush = () => {
    const folds = pending;
    pending = [];
    for (const subscription of subscriptions) {
      if (
        folds.some(
          (fold) =>
            fold.collections.includes(subscription.collection) &&
            (subscription.space === undefined ||
              subscription.space === fold.space),
        )
      ) {
        rerun(subscription);
      }
    }
  };
  // Runs a subscription's read; one that is asked again while it runs runs
  // once more after it, so a listener sees the newest state and never two
  // reads at once.
  const rerun = (subscription: Subscription) => {
    if (closed || !subscriptions.has(subscription)) return;
    if (subscription.running) {
      subscription.again = true;
      return;
    }
    subscription.running = true;
    subscription
      .run()
      .catch(() => {})
      .finally(() => {
        subscription.running = false;
        if (subscription.again) {
          subscription.again = false;
          rerun(subscription);
        }
      });
  };
  const follow = (
    collection: string,
    spaceId: string | undefined,
    run: () => Promise<void>,
  ) => {
    const subscription: Subscription = {
      collection,
      space: spaceId,
      run,
      running: false,
      again: false,
    };
    subscriptions.add(subscription);
    rerun(subscription);
    return () => {
      subscriptions.delete(subscription);
    };
  };

  /** A page of `query`, in `spaceId` when it names one. */
  const find = async <T>(query: object, spaceId?: string): Promise<Page<T>> => {
    if (closed) throw new NcError("closed", "the database is closed");
    const state = queryState(query);
    if (state === undefined) {
      throw new NcError(
        "invalidValue",
        "not a query; start one from an index, such as model.tasks.indexes.byPriority",
      );
    }
    const asked: Record<string, unknown> = {
      collection: nameOf(state.collection),
      index: state.index,
    };
    if (state.equal !== undefined) {
      asked.equal = state.equal.map((v) =>
        typeof v === "string" ? v.normalize("NFC") : v,
      );
    }
    if (state.take !== undefined) asked.take = state.take;
    if (state.after !== undefined) {
      const handle =
        state.after instanceof Cursor
          ? state.after.handleIn(identity)
          : undefined;
      if (handle === undefined) {
        throw new NcError(
          "cursorOfAnotherModel",
          "the cursor is not of this database; start from the first page",
        );
      }
      asked.after = handle;
    }
    const args: Record<string, unknown> = { query: asked };
    if (spaceId !== undefined) args.space = spaceId;
    const answer = (await ask("find", args)) as {
      items: { fields: Record<string, unknown> }[];
      cursor: number | null;
      hasMore: boolean;
    };
    return {
      items: answer.items.map(flatten) as T[],
      cursor:
        answer.cursor === null
          ? undefined
          : new Cursor(identity, answer.cursor),
      hasMore: answer.hasMore,
    };
  };

  /** The collection a query reads. */
  const collectionOf = (query: object): string => {
    const state = queryState(query);
    if (state === undefined) {
      throw new NcError("invalidValue", "not a query");
    }
    return nameOf(state.collection);
  };

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
      status: {
        current: () => (statuses.get(id) as { status: SpaceStatus }).status,
        subscribe: (listener) => {
          const entry = statuses.get(id);
          if (entry === undefined || closed) return () => {};
          entry.listeners.add(listener);
          listener(entry.status);
          return () => {
            entry.listeners.delete(listener);
          };
        },
      },
      get: (collection, object) => get(id, collection, object) as never,
      find: (query) => find(query, id),
      subscribe: ((
        first: object,
        second: unknown,
        third?: (object: unknown) => void,
      ) => {
        if (queryState(first) !== undefined) {
          const listener = second as (page: Page<unknown>) => void;
          return follow(collectionOf(first), id, async () =>
            listener(await find(first, id)),
          );
        }
        const object = second as string;
        const listener = third as (object: unknown) => void;
        return follow(nameOf(first), id, async () =>
          listener(await get(id, first, object)),
        );
      }) as Space["subscribe"],
    };
  };

  const get = async (spaceId: Id, collection: object, object: Id) => {
    if (closed) throw new NcError("closed", "the database is closed");
    const answer = (await ask("get", {
      space: spaceId,
      collection: nameOf(collection),
      id: object,
    })) as { fields: Record<string, unknown> } | null;
    return answer === null ? undefined : flatten(answer);
  };

  return {
    spaces: {
      create: async () => handleOf((await ask("spaces.create", {})) as string),
      get: async (id) =>
        handleOf((await ask("spaces.get", { space: id })) as string),
      list: async () =>
        (await ask("spaces.list", {})) as unknown as SpaceEntry[],
    },
    find: (query) => find(query),
    changes: {
      subscribe: (listener) => {
        changeListeners.add(listener);
        return () => {
          changeListeners.delete(listener);
        };
      },
    },
    subscribe: (query, listener) =>
      follow(collectionOf(query), undefined, async () =>
        listener(await find(query)),
      ),
    sync: {
      current: () => sync,
      subscribe: (listener) => {
        syncListeners.add(listener);
        listener(sync);
        return () => {
          syncListeners.delete(listener);
        };
      },
      pause: async () => {
        await ask("sync.pause", {});
      },
      resume: async () => {
        await ask("sync.resume", {});
      },
      isConfirmed,
      // A confirmation lowers `unconfirmed`, so it arrives as a sync
      // status: wait for the next one, registered before asking, so none
      // slips between the answer and the wait.
      confirmed: async (change) => {
        for (;;) {
          const next = nextSync();
          let held: boolean;
          try {
            held = await isConfirmed(change);
          } catch (error) {
            next.cancel();
            throw error;
          }
          if (held) {
            next.cancel();
            return;
          }
          await next.promise;
        }
      },
    },
    close: async () => {
      if (closed) return;
      try {
        await ask("close", {});
      } finally {
        closed = true;
        subscriptions.clear();
        changeListeners.clear();
        syncListeners.clear();
        for (const entry of statuses.values()) entry.listeners.clear();
        for (const waiter of syncWaiters) {
          waiter.reject(new NcError("closed", "the database is closed"));
        }
        syncWaiters.clear();
        core.terminate();
      }
    },
  };
};

/** One fold the core reports: its space, the collections whose reads it may have changed, and the ids it touched per collection with their origin. */
type Fold = {
  space: string;
  collections: string[];
  changes?: {
    collection: string;
    ids: string[];
    origin: "local" | "remote";
  }[];
};

/** What `db.changes.subscribe` gives a listener. */
type Change = Parameters<Parameters<Db["changes"]["subscribe"]>[0]>[0];

/** A read a subscription re-runs, and what it follows. */
type Subscription = {
  collection: string;
  space: string | undefined;
  run: () => Promise<void>;
  running: boolean;
  again: boolean;
};

/** An object as the core answers it, with its fields beside `$id`, `$space` and `$deleted`. */
const flatten = ({
  fields,
  ...rest
}: {
  fields: Record<string, unknown>;
}): Record<string, unknown> => ({ ...rest, ...fields });

/** 128 random bits from the browser's secure generator, as 32 hex digits (RFC 0002, "Object ids"). */
const randomId = (): Id => {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  return [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");
};
