// One tab owns a browser database, the others reach it (NOC-177; RFC 0006,
// "The browser"). Against fake Web Locks and fake workers, over Node's own
// BroadcastChannel, which carries messages between instances of one
// process as between tabs. Chromium runs the same with real tabs
// (scripts/sdk-browser/tabs.e2e.test.mjs).
import { expect, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import type { Core } from "../src/connect.ts";
import { type Platform, tabCore } from "../src/tabs.ts";

/** Web Locks for one origin: a lock is held while its callback's promise runs, and queued requests get it in order. */
const fakeLocks = (): Platform["locks"] => {
  const locks = new Map<string, { held: boolean; queue: (() => void)[] }>();
  return {
    request(name, options, callback) {
      const lock = locks.get(name) ?? { held: false, queue: [] };
      locks.set(name, lock);
      const run = async () => {
        lock.held = true;
        try {
          await callback({ name });
        } finally {
          lock.held = false;
          lock.queue.shift()?.();
        }
      };
      if (!lock.held) return run();
      if (options.ifAvailable) return Promise.resolve(callback(null));
      return new Promise((resolve, reject) => {
        const go = () => void run().then(resolve, reject);
        lock.queue.push(go);
        options.signal?.addEventListener("abort", () => {
          const at = lock.queue.indexOf(go);
          if (at >= 0) {
            lock.queue.splice(at, 1);
            reject(new Error("aborted"));
          }
        });
      });
    },
  };
};

/** A worker that answers like the bridge, logs each call, and never answers the calls in `hold`. */
const fakeWorker = (log: string[], hold: Set<string>) => {
  let listener: ((event: Uint8Array) => void) | undefined;
  let stopped = false;
  const core: Core = {
    call: async (bytes) => {
      const { id, call } = decode(bytes) as { id: number; call: string };
      log.push(call);
      if (hold.has(call)) return new Promise(() => {});
      switch (call) {
        case "open":
          return encode({ id, ok: { sync: { connection: "none" } } });
        case "spaces.list":
          return encode({ id, ok: [] });
        case "write":
          return encode({ id, ok: { change: "c", ids: [] } });
        default:
          return encode({ id, ok: null });
      }
    },
    terminate: () => {
      stopped = true;
    },
    listen: (given) => {
      listener = given;
    },
  };
  return {
    core,
    stopped: () => stopped,
    emit: (event: Record<string, unknown>) => listener?.(encode(event)),
  };
};

let counter = 0;
/** A fresh origin: its locks, and the tabs opened in it. */
const origin = () => {
  counter += 1;
  const name = `test-${counter}`;
  const locks = fakeLocks();
  /** A tab: its core, the calls its own worker got, the events it heard, whether its worker stopped, and its page, which a test hides and shows. */
  const tab = async (hold = new Set<string>()) => {
    const log: string[] = [];
    const heard: unknown[] = [];
    let worker: ReturnType<typeof fakeWorker> | undefined;
    let visible = true;
    const watchers: ((visible: boolean) => void)[] = [];
    const page = {
      visible: () => visible,
      onVisibility: (listener: (visible: boolean) => void) => {
        watchers.push(listener);
      },
    };
    const show = (now: boolean) => {
      visible = now;
      for (const watcher of watchers) watcher(now);
    };
    const core = await tabCore(name, {
      page,
      locks,
      channel: (channel) =>
        new BroadcastChannel(channel) as unknown as ReturnType<
          Platform["channel"]
        >,
      startWorker: () => {
        worker = fakeWorker(log, hold);
        return worker.core;
      },
    });
    core.listen((bytes) => heard.push(decode(bytes)));
    return {
      core,
      log,
      heard,
      worker: () => worker,
      hide: () => show(false),
      show: () => show(true),
    };
  };
  return { tab };
};

let id = 0;
const ask = async (
  core: Core,
  call: string,
  args: Record<string, unknown> = {},
) => {
  id += 1;
  return decode(await core.call(encode({ id, call, args }))) as {
    ok?: unknown;
    error?: { code: string; message: string };
  };
};

const LOCK = '{"model":{"id":"aa"}}';
const settle = () => new Promise((resolve) => setTimeout(resolve, 20));
const until = async (check: () => boolean) => {
  for (let i = 0; i < 200 && !check(); i += 1) await settle();
  expect(check()).toBe(true);
};

test("the first tab owns the database, and a second reaches it through the first", async () => {
  const { tab } = origin();
  const first = await tab();
  await ask(first.core, "open", { lock: LOCK });
  const second = await tab();
  expect((await ask(second.core, "open", { lock: LOCK })).ok).toEqual({
    sync: { connection: "none" },
  });
  expect((await ask(second.core, "spaces.list")).ok).toEqual([]);
  expect(first.log).toEqual(["open", "spaces.list"]);
  expect(second.log).toEqual([]);
});

test("every event of the owner's core reaches every tab", async () => {
  const { tab } = origin();
  const first = await tab();
  await ask(first.core, "open", { lock: LOCK });
  const second = await tab();
  await ask(second.core, "open", { lock: LOCK });
  first.worker()?.emit({ event: "folded", folds: [] });
  await until(() => second.heard.length === 1);
  expect(first.heard).toEqual([{ event: "folded", folds: [] }]);
  expect(second.heard).toEqual([{ event: "folded", folds: [] }]);
});

test("a tab that opens with another lock is refused with modelRefused", async () => {
  const { tab } = origin();
  const first = await tab();
  await ask(first.core, "open", { lock: LOCK });
  const second = await tab();
  const answer = await ask(second.core, "open", {
    lock: '{"model":{"id":"bb"}}',
  });
  expect(answer.error?.code).toBe("modelRefused");
});

test("when the owner closes, the next tab opens the database with its own request and tells the others", async () => {
  const { tab } = origin();
  const first = await tab();
  await ask(first.core, "open", { lock: LOCK });
  const second = await tab();
  await ask(second.core, "open", { lock: LOCK });
  const third = await tab();
  await ask(third.core, "open", { lock: LOCK });
  await ask(first.core, "close");
  first.core.terminate();
  await until(() => second.log.includes("open"));
  expect((await ask(third.core, "spaces.list")).ok).toEqual([]);
  expect(second.log).toEqual(["open", "spaces.list"]);
  await until(() =>
    third.heard.some(
      (event) => (event as { event: string }).event === "reopened",
    ),
  );
  expect(second.heard).toContainEqual({
    event: "reopened",
    sync: { connection: "none" },
  });
});

test("a write in flight when the owner goes away fails, and the new owner never runs it", async () => {
  const { tab } = origin();
  const first = await tab(new Set(["write", "spaces.list"]));
  await ask(first.core, "open", { lock: LOCK });
  const second = await tab();
  await ask(second.core, "open", { lock: LOCK });
  const write = ask(second.core, "write", { space: "s", ops: [] });
  const read = ask(second.core, "spaces.list");
  await until(() => first.log.includes("spaces.list"));
  first.core.terminate();
  const failed = await write.catch((error) => error);
  expect(failed.code).toBe("storageFailed");
  expect((await read).ok).toEqual([]);
  expect(second.log).toEqual(["open", "spaces.list"]);
  expect(first.log.filter((call) => call === "write")).toEqual(["write"]);
});

for (const call of [
  "members.add",
  "members.remove",
  "members.setRole",
  "space.leave",
  "account.pair",
  "account.completePairing",
  "account.removeMember",
]) {
  test(`${call} in flight when the owner goes away fails, and the new owner never runs it`, async () => {
    const { tab } = origin();
    const first = await tab(new Set([call]));
    await ask(first.core, "open", { lock: LOCK });
    const second = await tab();
    await ask(second.core, "open", { lock: LOCK });
    const changed = ask(second.core, call, { space: "s" });
    await until(() => first.log.includes(call));
    first.core.terminate();
    const failed = await changed.catch((error) => error);
    expect(failed.code).toBe("storageFailed");
    expect(second.log).toEqual(["open"]);
  });
}

test("closing a follower ends only its own connection", async () => {
  const { tab } = origin();
  const first = await tab();
  await ask(first.core, "open", { lock: LOCK });
  const second = await tab();
  await ask(second.core, "open", { lock: LOCK });
  await ask(second.core, "close");
  second.core.terminate();
  expect(first.log).toEqual(["open"]);
  expect((await ask(first.core, "spaces.list")).ok).toEqual([]);
});

test("a follower's open names the owner's relay, which serves every tab", async () => {
  const { tab } = origin();
  const first = await tab();
  await ask(first.core, "open", { lock: LOCK, relay: "ws://owner" });
  const second = await tab();
  const answer = await ask(second.core, "open", {
    lock: LOCK,
    relay: "ws://follower",
  });
  expect(answer.ok).toEqual({
    sync: { connection: "none" },
    relay: "ws://owner",
  });
});

// NOC-332: a phone freezes a hidden tab but leaves it the lock, so the
// owner gives up the database when its page is hidden, and the visible tab
// takes over.

test("a hidden owner gives up the database, and the visible tab takes it over and runs its write", async () => {
  const { tab } = origin();
  const first = await tab();
  await ask(first.core, "open", { lock: LOCK });
  const second = await tab();
  await ask(second.core, "open", { lock: LOCK });
  first.hide();
  await until(() => second.log.includes("open"));
  expect((await ask(second.core, "write", { space: "s", ops: [] })).ok).toEqual(
    {
      change: "c",
      ids: [],
    },
  );
  expect(second.log).toContain("write");
  expect(first.log).not.toContain("write");
});

test("a hidden owner first answers the calls it already took", async () => {
  const { tab } = origin();
  const first = await tab();
  await ask(first.core, "open", { lock: LOCK });
  const second = await tab();
  await ask(second.core, "open", { lock: LOCK });
  const write = ask(second.core, "write", { space: "s", ops: [] });
  await until(() => first.log.includes("write"));
  first.hide();
  expect((await write).ok).toEqual({ change: "c", ids: [] });
  expect(second.log.filter((call) => call === "write")).toEqual([]);
});

test("a hidden tab does not take the database over; a visible one does", async () => {
  const { tab } = origin();
  const first = await tab();
  await ask(first.core, "open", { lock: LOCK });
  const hidden = await tab();
  await ask(hidden.core, "open", { lock: LOCK });
  hidden.hide();
  const visible = await tab();
  await ask(visible.core, "open", { lock: LOCK });
  first.hide();
  await until(() => visible.log.includes("open"));
  expect(hidden.log).not.toContain("open");
});

test("with every tab hidden, a tab that calls takes the database over", async () => {
  const { tab } = origin();
  const first = await tab();
  await ask(first.core, "open", { lock: LOCK });
  const second = await tab();
  await ask(second.core, "open", { lock: LOCK });
  second.hide();
  first.hide();
  expect((await ask(second.core, "spaces.list")).ok).toEqual([]);
  expect(second.log).toContain("open");
});

test("an owner shown again takes the database back when no other tab holds it", async () => {
  const { tab } = origin();
  const only = await tab();
  await ask(only.core, "open", { lock: LOCK });
  only.hide();
  await settle();
  only.show();
  expect((await ask(only.core, "spaces.list")).ok).toEqual([]);
  expect(only.log.filter((call) => call === "open")).toEqual(["open", "open"]);
});

test("an owner with no call in flight gives the database up inside the hide handler, before a phone can freeze it", async () => {
  const { tab } = origin();
  const first = await tab();
  await ask(first.core, "open", { lock: LOCK });
  first.hide();
  // No await: a frozen page runs nothing after its visibilitychange handler.
  expect(first.worker()?.stopped()).toBe(true);
});
