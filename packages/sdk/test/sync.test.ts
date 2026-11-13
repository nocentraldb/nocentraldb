// db.sync (RFC 0009, "Sync and status"; NOC-176 slice 4): the relay
// connection as a report. `current()` reads the newest status the core
// sent, at open and in its `sync` events (crates/nocentraldb/src/bridge/
// mod.rs); `subscribe` gives it at once and after every change. Against a
// fake core.
import { expect, expectTypeOf, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import { NcError } from "../src/error.ts";
import { nc } from "../src/index.ts";

const model = nc.model({
  tasks: nc.collection({ title: nc.string() }),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };
const CHANGE = "cc".repeat(32);

const OFFLINE = {
  connection: "offline",
  unconfirmed: 0,
  refused: 0,
  epochChanged: false,
  error: null,
};

/** A fake core: answers `open` with `opened`, records every call, and answers `sync.isConfirmed` from `confirmed`. */
const fake = (opened: Record<string, unknown> = OFFLINE) => {
  let listener: ((event: Uint8Array) => void) | undefined;
  const calls: { call: string; args: Record<string, unknown> }[] = [];
  const confirmed = new Set<string>();
  const core: Core = {
    call: async (bytes) => {
      const { id, call, args } = decode(bytes) as {
        id: number;
        call: string;
        args: Record<string, unknown>;
      };
      calls.push({ call, args });
      if (call === "open") return encode({ id, ok: { sync: opened } });
      if (call === "sync.isConfirmed") {
        return encode({ id, ok: confirmed.has(args.change as string) });
      }
      return encode({ id, ok: null });
    },
    terminate: () => {},
    listen: (given) => {
      listener = given;
    },
  };
  const send = (status: Record<string, unknown>) =>
    listener?.(encode({ event: "sync", status }));
  return { core, calls, confirmed, send };
};

const relay = "ws://127.0.0.1:7700";

test("db.sync.current() gives the status the core answered at open, with the relay", async () => {
  const { core } = fake();
  const db = await connect(core, { model, lock, relay });
  expect(db.sync.current()).toEqual({
    connection: "offline",
    relay,
    unconfirmed: 0,
    refused: 0,
    epochChanged: false,
  });
});

test("without a relay, the connection is none and no relay is named", async () => {
  const { core } = fake({ ...OFFLINE, connection: "none" });
  const db = await connect(core, { model, lock });
  const status = db.sync.current();
  expect(status.connection).toBe("none");
  expect("relay" in status).toBe(false);
});

test("a sync event changes current(), and its error is a code", async () => {
  const { core, send } = fake();
  const db = await connect(core, { model, lock, relay });
  send({ ...OFFLINE, connection: "connected", unconfirmed: 2 });
  expect(db.sync.current()).toMatchObject({
    connection: "connected",
    unconfirmed: 2,
  });
  send({ ...OFFLINE, error: "unsupported_version" });
  expect(db.sync.current().error).toBe("unsupported_version");
});

test("db.sync.subscribe gives the status at once and after each change, until stopped or closed", async () => {
  const { core, send } = fake();
  const db = await connect(core, { model, lock, relay });
  const seen: string[] = [];
  const stop = db.sync.subscribe((status) => seen.push(status.connection));
  const kept: string[] = [];
  db.sync.subscribe((status) => kept.push(status.connection));
  expect(seen).toEqual(["offline"]);
  send({ ...OFFLINE, connection: "connecting" });
  stop();
  send({ ...OFFLINE, connection: "connected" });
  expect(seen).toEqual(["offline", "connecting"]);
  expect(kept).toEqual(["offline", "connecting", "connected"]);
  await db.close();
  send({ ...OFFLINE, connection: "offline" });
  expect(kept).toEqual(["offline", "connecting", "connected"]);
});

test("a listener that throws does not stop the others", async () => {
  const { core, send } = fake();
  const db = await connect(core, { model, lock, relay });
  const seen: string[] = [];
  let first = true;
  db.sync.subscribe(() => {
    if (first) {
      first = false;
      return;
    }
    throw new Error("app bug");
  });
  db.sync.subscribe((status) => seen.push(status.connection));
  send({ ...OFFLINE, connection: "connected" });
  expect(seen).toEqual(["offline", "connected"]);
});

test("pause and resume ask the core", async () => {
  const { core, calls } = fake();
  const db = await connect(core, { model, lock, relay });
  await db.sync.pause();
  await db.sync.resume();
  expect(calls.map(({ call }) => call).slice(1)).toEqual([
    "sync.pause",
    "sync.resume",
  ]);
});

test("isConfirmed asks the core whether the relay holds the change", async () => {
  const { core, calls, confirmed } = fake();
  const db = await connect(core, { model, lock, relay });
  expect(await db.sync.isConfirmed(CHANGE)).toBe(false);
  confirmed.add(CHANGE);
  expect(await db.sync.isConfirmed(CHANGE)).toBe(true);
  expect(calls.at(-1)).toEqual({
    call: "sync.isConfirmed",
    args: { change: CHANGE },
  });
});

test("confirmed resolves once a status change finds the change confirmed", async () => {
  const { core, confirmed, send } = fake({ ...OFFLINE, unconfirmed: 1 });
  const db = await connect(core, { model, lock, relay });
  let done = false;
  const waiting = db.sync.confirmed(CHANGE).then(() => {
    done = true;
  });
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(done).toBe(false);
  send({ ...OFFLINE, connection: "connected", unconfirmed: 1 });
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(done).toBe(false);
  confirmed.add(CHANGE);
  send({ ...OFFLINE, connection: "connected", unconfirmed: 0 });
  await waiting;
  expect(done).toBe(true);
});

test("confirmed fails with closed when the database closes first", async () => {
  const { core } = fake();
  const db = await connect(core, { model, lock, relay });
  const waiting = db.sync.confirmed(CHANGE);
  await new Promise((resolve) => setTimeout(resolve, 0));
  await db.close();
  await expect(waiting).rejects.toBeInstanceOf(NcError);
  await expect(waiting).rejects.toMatchObject({ code: "closed" });
});

test("its types are RFC 0009's", async () => {
  const { core } = fake();
  const db = await connect(core, { model, lock, relay });
  const status = db.sync.current();
  expectTypeOf(status.connection).toEqualTypeOf<
    "connected" | "connecting" | "offline" | "paused" | "none"
  >();
  expectTypeOf(status.unconfirmed).toEqualTypeOf<number>();
  expectTypeOf(status.refused).toEqualTypeOf<number>();
  expectTypeOf(status.epochChanged).toEqualTypeOf<boolean>();
  expectTypeOf(status.error).toEqualTypeOf<string | undefined>();
  expectTypeOf(status.relay).toEqualTypeOf<string | undefined>();
  expectTypeOf(db.sync.isConfirmed).returns.toEqualTypeOf<Promise<boolean>>();
  expectTypeOf(db.sync.confirmed).returns.toEqualTypeOf<Promise<void>>();
});

test("in a tab that reaches another tab's database, current() names the owner's relay", async () => {
  const { core } = fake({ ...OFFLINE });
  const reached: Core = {
    ...core,
    call: async (bytes) => {
      const answer = decode(await core.call(bytes)) as {
        id: number;
        ok?: Record<string, unknown>;
      };
      const { call } = decode(bytes) as { call: string };
      return encode(
        call === "open"
          ? { ...answer, ok: { ...answer.ok, relay: "ws://owner" } }
          : answer,
      );
    },
  };
  const db = await connect(reached, { model, lock, relay });
  expect(db.sync.current().relay).toBe("ws://owner");
});
