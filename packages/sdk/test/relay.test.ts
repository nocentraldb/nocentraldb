// nc.open's `relay` as a function (NOC-496): the app's function that gives
// the relay URL, so a short-lived token in it is renewed. The core asks for
// the URL with a `relay` event before each connect; the SDK calls the
// function and answers with `relay.answer`, null when the function fails.
// The sync status names the relay the function last gave, as it names a
// fixed one. Against a fake core.

import { expect, expectTypeOf, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import { nc } from "../src/index.ts";

const model = nc.model({
  tasks: nc.collection({ title: nc.string() }),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };
const OFFLINE = {
  connection: "offline",
  unconfirmed: 0,
  refused: 0,
  epochChanged: false,
  error: null,
};

const fake = () => {
  const calls: { call: string; args: Record<string, unknown> }[] = [];
  let listener: ((bytes: Uint8Array) => void) | undefined;
  const core: Core = {
    call: async (bytes) => {
      const { id, call, args } = decode(bytes) as {
        id: number;
        call: string;
        args: Record<string, unknown>;
      };
      calls.push({ call, args });
      if (call === "open") return encode({ id, ok: { sync: OFFLINE } });
      return encode({ id, ok: null });
    },
    terminate: () => {},
    listen: (given) => {
      listener = given;
    },
  };
  const ask = (request: number) =>
    listener?.(encode({ event: "relay", request }));
  const send = (status: Record<string, unknown>) =>
    listener?.(encode({ event: "sync", status }));
  return { core, calls, ask, send };
};

/** The `relay.answer` calls once there are `count` of them. */
const answers = async (calls: { call: string }[], count = 1) => {
  for (let i = 0; i < 100; i += 1) {
    const found = calls.filter((c) => c.call === "relay.answer");
    if (found.length >= count) return found;
    await new Promise((resolve) => setTimeout(resolve, 5));
  }
  throw new Error(`fewer than ${count} relay.answer`);
};

test("relay takes a URL or a function that gives one", () => {
  expectTypeOf<Parameters<typeof nc.open>[0]["relay"]>().toEqualTypeOf<
    string | (() => Promise<string>) | undefined
  >();
});

test("open tells the core that the app passed a relay function", async () => {
  const { core, calls } = fake();
  await connect(core, { model, lock, relay: async () => "wss://r.example" });
  expect(calls[0]?.args).toMatchObject({ relay: true });
});

test("each ask of the core calls the function again and answers its URL", async () => {
  const { core, calls, ask } = fake();
  let n = 0;
  await connect(core, {
    model,
    lock,
    relay: async () => {
      n += 1;
      return `wss://r.example/prod?token=t${n}`;
    },
  });
  ask(1);
  await answers(calls, 1);
  ask(2);
  const answered = await answers(calls, 2);
  expect(answered).toEqual([
    {
      call: "relay.answer",
      args: { request: 1, relay: "wss://r.example/prod?token=t1" },
    },
    {
      call: "relay.answer",
      args: { request: 2, relay: "wss://r.example/prod?token=t2" },
    },
  ]);
});

test("a function that throws, or gives no text, answers null", async () => {
  const { core, calls, ask } = fake();
  let n = 0;
  await connect(core, {
    model,
    lock,
    relay: async () => {
      n += 1;
      if (n === 1) throw new Error("the backend is down");
      return 42 as unknown as string;
    },
  });
  ask(1);
  ask(2);
  expect(await answers(calls, 2)).toEqual([
    { call: "relay.answer", args: { request: 1, relay: null } },
    { call: "relay.answer", args: { request: 2, relay: null } },
  ]);
});

test("the sync status names the relay the function last gave, without its token", async () => {
  const { core, calls, ask, send } = fake();
  const db = await connect(core, {
    model,
    lock,
    relay: async () => "wss://r.example/prod?token=secret",
  });
  expect("relay" in db.sync.current()).toBe(false);
  ask(1);
  await answers(calls);
  send({ ...OFFLINE, connection: "connected" });
  expect(db.sync.current()).toMatchObject({
    connection: "connected",
    relay: "wss://r.example/prod",
  });
  expect(JSON.stringify(db.sync.current())).not.toContain("secret");
});
