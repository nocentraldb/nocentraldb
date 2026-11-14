// The durability of the copy and the view's report as reports (NOC-315,
// slice 3; RFC 0006, "What the SDK reports"). `open` answers both, the
// core's `durability` and `view` events keep them current, and the page
// asks the browser for persistent storage: a worker can read `persisted()`
// but not call `persist()`, so the page asks and the core reads again.
// Internal: the package's entry does not reach it.

import type { Cbor } from "./cbor.ts";
import type { Durability, ViewStatus } from "./durability.ts";
import { notify } from "./reports.ts";

type Ask = (call: string, args: Record<string, unknown>) => Promise<Cbor>;

/** A durability the core sent. */
const durabilityOf = (raw: unknown): Durability => {
  const given = (raw ?? {}) as Record<string, unknown>;
  return {
    storage: (given.storage ?? "temporary") as Durability["storage"],
    unconfirmed: Number(given.unconfirmed ?? 0),
    oldestUnconfirmedAt:
      given.oldestUnconfirmedAt === null ||
      given.oldestUnconfirmedAt === undefined
        ? null
        : Number(given.oldestUnconfirmedAt),
    atRisk: given.atRisk === true,
  };
};

/** A view's report the core sent. */
const viewOf = (raw: unknown): ViewStatus => {
  const rebuild = ((raw ?? {}) as { rebuild?: unknown }).rebuild as
    | { folded?: unknown; total?: unknown }
    | null
    | undefined;
  return {
    rebuild:
      rebuild === null || rebuild === undefined
        ? null
        : { folded: Number(rebuild.folded), total: Number(rebuild.total) },
  };
};

/** The browser's `navigator.storage.persist()`, where it has one. */
const persist = (): (() => Promise<boolean>) | undefined => {
  const storage = (
    globalThis as { navigator?: { storage?: { persist?: unknown } } }
  ).navigator?.storage;
  return typeof storage?.persist === "function"
    ? () => (storage.persist as () => Promise<boolean>).call(storage)
    : undefined;
};

/** The durability and view reports of one database. */
export const reportsOf = (
  ask: Ask,
  opened: { durability?: unknown; view?: unknown },
  persistentStorage: "on-open" | "on-request" = "on-open",
) => {
  let durability = durabilityOf(opened.durability);
  let view = viewOf(opened.view);
  const durabilityListeners = new Set<(durability: Durability) => void>();
  const viewListeners = new Set<(view: ViewStatus) => void>();

  const durabilityChanged = (raw: unknown) => {
    durability = durabilityOf(raw);
    notify(durabilityListeners, durability);
  };
  const viewChanged = (raw: unknown) => {
    view = viewOf(raw);
    notify(viewListeners, view);
  };
  /** Asks the core to read again whether the browser keeps the copy. */
  const refresh = () =>
    ask("durability", {}).then(durabilityChanged, () => {
      // A closed database answers nothing more.
    });

  const requestPersistent = async (): Promise<boolean> => {
    const asking = persist();
    let answer = false;
    if (asking !== undefined) {
      try {
        answer = (await asking()) === true;
      } catch {
        answer = false;
      }
    }
    await refresh();
    return answer;
  };

  // RFC 0006: on open, without waiting; `subscribe` reports the answer.
  // Where the browser has no `persist()`, there is nothing to ask.
  if (persistentStorage === "on-open" && persist() !== undefined) {
    void requestPersistent();
  }

  // RFC 0006: a user can grant or revoke it in the browser's settings.
  const page = (
    globalThis as {
      document?: {
        visibilityState?: string;
        addEventListener?(kind: string, handler: () => void): void;
        removeEventListener?(kind: string, handler: () => void): void;
      };
    }
  ).document;
  const visible = () => {
    if (page?.visibilityState === "visible") void refresh();
  };
  page?.addEventListener?.("visibilitychange", visible);

  return {
    durability: {
      current: () => durability,
      subscribe: (listener: (durability: Durability) => void) => {
        durabilityListeners.add(listener);
        notify(new Set([listener]), durability);
        return () => {
          durabilityListeners.delete(listener);
        };
      },
      requestPersistent,
    },
    view: {
      current: () => view,
      subscribe: (listener: (view: ViewStatus) => void) => {
        viewListeners.add(listener);
        notify(new Set([listener]), view);
        return () => {
          viewListeners.delete(listener);
        };
      },
    },
    /** A `durability` event of the core. */
    durabilityChanged,
    /** A `view` event of the core. */
    viewChanged,
    /** Another tab took the database over (NOC-177): read both again. */
    reopened() {
      void refresh();
      void ask("view", {}).then(viewChanged, () => {});
    },
    /** The database closed. */
    clear() {
      durabilityListeners.clear();
      viewListeners.clear();
      page?.removeEventListener?.("visibilitychange", visible);
    },
  };
};
