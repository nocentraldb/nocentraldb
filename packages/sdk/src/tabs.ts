// One tab owns a browser database, the others reach it (NOC-177; RFC 0006,
// "The browser": "One tab owns the database"; RFC 0009, "Open and close").
// A pool of OPFS access handles is exclusive, so only one tab's worker can
// open a database. The first tab to open it takes the Web Lock
// `nocentraldb/<name>` and runs the worker; every other tab queues for the
// lock and, meanwhile, sends its calls to the owner over the
// BroadcastChannel `nocentraldb/<name>`. The owner runs them through its
// worker, in order with its own, and passes every event of the core to
// every tab. When the owner closes or its tab goes away, the lock passes to
// the next tab in the queue, which starts its own worker and opens the
// database with its own options.
//
// What this module gives `connect` is a `Core` like the worker's: the
// bridge's bytes pass through unchanged. Internal: the package's entry
// does not reach it.

import { type Cbor, decode, encode } from "./cbor.ts";
import type { Core } from "./connect.ts";
import { NcError } from "./error.ts";

/** What a tab reaches the others with; the browser's, or a test's fakes. */
export interface Platform {
  /** The Web Locks API, as `navigator.locks` gives it. */
  locks: {
    request(
      name: string,
      options: { ifAvailable?: boolean; signal?: AbortSignal },
      callback: (lock: unknown) => Promise<void> | void,
    ): Promise<unknown>;
  };
  /** A channel to every other tab of the origin, as `new BroadcastChannel(name)`. */
  channel(name: string): {
    postMessage(message: unknown): void;
    onmessage: ((event: { data: unknown }) => void) | null;
    close(): void;
  };
  /** Starts the worker that runs the core. */
  startWorker(): Core;
}

/** What the tabs of one database send each other. */
type Message =
  | { kind: "request"; from: string; id: number; bytes: Uint8Array }
  | { kind: "answer"; to: string; id: number; bytes: Uint8Array }
  | { kind: "event"; bytes: Uint8Array }
  | { kind: "who" }
  | { kind: "owner"; tab: string }
  | { kind: "gone"; tab: string };

/** Calls that change the database or its syncing: a tab never sends one twice, since it cannot know whether a lost owner ran it. */
const CHANGES = new Set([
  "write",
  "spaces.create",
  "sync.pause",
  "sync.resume",
  "members.add",
  "members.remove",
  "members.setRole",
  "space.leave",
]);

type Request = { id: number; call: string; args: Record<string, Cbor> };

/** A call this tab sent another tab, while it waits for the answer. */
type Sent = {
  bytes: Uint8Array;
  call: string;
  resolve: (answer: Uint8Array) => void;
  reject: (error: unknown) => void;
  /** The owner it went to; undefined while no owner is known. */
  to: string | undefined;
};

const failure = (id: number, code: string, message: string) =>
  encode({ id, error: { code, message } });

/** The core of the database `name` as this tab reaches it: its own worker while it owns the database, the owner tab's otherwise. */
export const tabCore = async (
  name: string,
  platform: Platform,
): Promise<Core> => {
  const me = crypto.randomUUID();
  const lockName = `nocentraldb/${name}`;
  const channel = platform.channel(lockName);
  const post = (message: Message) => channel.postMessage(message);

  let listener: ((event: Uint8Array) => void) | undefined;
  const emit = (bytes: Uint8Array) => listener?.(bytes);

  // This tab while it owns the database.
  let worker: Core | undefined;
  let release: (() => void) | undefined;
  let ownOpen: Request | undefined;
  let opened: () => void = () => {};
  let ready = new Promise<void>((resolve) => {
    opened = resolve;
  });
  let sync: Cbor = null;

  // This tab while it follows another.
  let owner: string | undefined;
  let next = 0;
  const sent = new Map<number, Sent>();
  const waiting = new AbortController();
  let closed = false;
  // Why every call fails: this tab could not open the database it took over.
  let failed: NcError | undefined;
  // The open request of this tab, kept to open the database when it takes
  // over, which may come before `connect` sends it.
  let openRequest: Uint8Array | undefined;
  let openGiven: () => void = () => {};
  const openSent = new Promise<void>((resolve) => {
    openGiven = resolve;
  });

  /** The newest sync status, from an answer to `open` or a `sync` event. */
  const noteSync = (bytes: Uint8Array) => {
    const message = decode(bytes) as {
      event?: string;
      status?: Cbor;
      ok?: { sync?: Cbor };
    };
    if (message.event === "sync") sync = message.status ?? null;
    else if (message.ok?.sync !== undefined) sync = message.ok.sync;
  };

  const workerEvent = (bytes: Uint8Array) => {
    noteSync(bytes);
    emit(bytes);
    post({ kind: "event", bytes });
  };

  /** The relay this tab opened with, which serves every tab while it owns the database. */
  const relayOfOwner = (): { relay?: Cbor } =>
    typeof ownOpen?.args.relay === "string"
      ? { relay: ownOpen.args.relay }
      : {};

  /** The owner's answer to another tab's call. */
  const serve = async (from: string, id: number, bytes: Uint8Array) => {
    await ready;
    const request = decode(bytes) as Request;
    let answer: Uint8Array;
    if (request.call === "open") {
      answer =
        ownOpen?.args.lock === request.args.lock
          ? encode({ id: request.id, ok: { sync, ...relayOfOwner() } })
          : failure(
              request.id,
              "modelRefused",
              "another tab has this database open with another lock",
            );
    } else {
      answer = await (worker as Core)
        .call(bytes)
        .catch((error) => failure(request.id, "storageFailed", String(error)));
    }
    post({ kind: "answer", to: from, id, bytes: answer });
  };

  /** Sends this tab's call to the owner, or holds it while none is known. */
  const send = (bytes: Uint8Array, call: string): Promise<Uint8Array> =>
    new Promise((resolve, reject) => {
      next += 1;
      const entry: Sent = { bytes, call, resolve, reject, to: owner };
      sent.set(next, entry);
      if (owner !== undefined) {
        post({ kind: "request", from: me, id: next, bytes });
      }
    });

  /** A new owner, `tab`: a call the old one never answered goes to it again, unless it changes the database. */
  const ownerIs = (tab: string) => {
    owner = tab;
    for (const [id, entry] of sent) {
      if (
        entry.to !== undefined &&
        entry.to !== tab &&
        CHANGES.has(entry.call)
      ) {
        sent.delete(id);
        entry.reject(
          new NcError(
            "storageFailed",
            "the tab that held the database closed before it answered; the call may or may not have run",
          ),
        );
        continue;
      }
      entry.to = tab;
      if (tab === me) {
        sent.delete(id);
        if (entry.call === "open") {
          // This tab opened the database itself as it took over.
          const { id: asked } = decode(entry.bytes) as Request;
          entry.resolve(encode({ id: asked, ok: { sync } }));
        } else {
          (worker as Core).call(entry.bytes).then(entry.resolve, entry.reject);
        }
      } else {
        post({ kind: "request", from: me, id, bytes: entry.bytes });
      }
    }
  };

  channel.onmessage = ({ data }) => {
    if (closed) return;
    const message = data as Message;
    switch (message.kind) {
      case "request":
        if (worker !== undefined)
          void serve(message.from, message.id, message.bytes);
        return;
      case "answer": {
        if (message.to !== me) return;
        const entry = sent.get(message.id);
        if (entry === undefined) return;
        sent.delete(message.id);
        entry.resolve(message.bytes);
        return;
      }
      case "event":
        if (worker === undefined) emit(message.bytes);
        return;
      case "who":
        if (worker !== undefined) post({ kind: "owner", tab: me });
        return;
      case "owner":
        if (worker === undefined && message.tab !== owner) ownerIs(message.tab);
        return;
      case "gone":
        if (message.tab === owner) owner = undefined;
        return;
    }
  };

  /** Starts this tab's worker as the owner; `open` is the request it opens with. */
  const own = (): Core => {
    const started = platform.startWorker();
    started.listen(workerEvent);
    worker = started;
    return started;
  };

  /** Takes the database over from an owner that went away. */
  const takeOver = async () => {
    await openSent;
    if (closed || openRequest === undefined) return;
    const started = own();
    ready = new Promise<void>((resolve) => {
      opened = resolve;
    });
    const answer = await started.call(openRequest.slice());
    noteSync(answer);
    const result = decode(answer) as {
      error?: { code: string; message: string };
    };
    if (result.error) {
      // This tab cannot open the database: it gives the lock to the next.
      const error = new NcError(result.error.code, result.error.message);
      for (const entry of sent.values()) entry.reject(error);
      sent.clear();
      failed = error;
      closed = true;
      worker = undefined;
      started.terminate();
      release?.();
      return;
    }
    ownOpen = decode(openRequest) as Request;
    opened();
    post({ kind: "owner", tab: me });
    ownerIs(me);
    // Every tab re-reads what it follows from the new owner.
    const reopened = encode({ event: "reopened", sync, ...relayOfOwner() });
    emit(reopened);
    post({ kind: "event", bytes: reopened });
  };

  const held = (lock: unknown): Promise<void> | void => {
    if (!lock) return;
    return new Promise<void>((resolve) => {
      release = resolve;
    });
  };

  const owns = await new Promise<boolean>((resolve) => {
    void platform.locks.request(lockName, { ifAvailable: true }, (lock) => {
      resolve(Boolean(lock));
      return held(lock);
    });
  });

  if (owns) {
    own();
  } else {
    post({ kind: "who" });
    void platform.locks
      .request(lockName, { signal: waiting.signal }, (lock) => {
        const holding = held(lock);
        void takeOver();
        return holding;
      })
      .catch(() => {
        // Aborted: this tab closed while it waited.
      });
  }

  const shutDown = () => {
    if (closed) return;
    closed = true;
    waiting.abort();
    if (worker !== undefined) {
      post({ kind: "gone", tab: me });
      worker.terminate();
      release?.();
    }
    for (const entry of sent.values()) {
      entry.reject(new NcError("closed", "the database is closed"));
    }
    sent.clear();
    channel.close();
  };

  return {
    call: async (bytes) => {
      if (failed) throw failed;
      const request = decode(bytes) as Request;
      if (request.call === "open") {
        openRequest = bytes.slice();
        openGiven();
      }
      if (worker !== undefined) {
        const answer = await worker.call(bytes);
        if (request.call === "open") {
          noteSync(answer);
          ownOpen = request;
          opened();
        }
        return answer;
      }
      if (request.call === "close") {
        shutDown();
        return encode({ id: request.id, ok: null });
      }
      return send(bytes, request.call);
    },
    terminate: shutDown,
    listen: (given) => {
      listener = given;
    },
  };
};
