// nc.open's `admission` option (RFC 0015, "In the SDK"; names waiting on
// the owner's NOC-493): the app's function that fetches an admission from
// its backend. The core asks for one with an `admission` event; the SDK
// calls the function with the identity and answers with
// `admission.answer`, null when the function fails. Against a fake core.

import { expect, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import { nc } from "../src/index.ts";

const model = nc.model({
  tasks: nc.collection({ title: nc.string() }),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };
const IDENTITY = "ab".repeat(32);

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
      return encode({ id, ok: null });
    },
    terminate: () => {},
    listen: (given) => {
      listener = given;
    },
  };
  const ask = (request: number) =>
    listener?.(encode({ event: "admission", request, identity: IDENTITY }));
  return { core, calls, ask };
};

const answered = async (calls: { call: string }[]) => {
  for (let i = 0; i < 100; i += 1) {
    const found = calls.find((c) => c.call === "admission.answer");
    if (found) return found;
    await new Promise((resolve) => setTimeout(resolve, 5));
  }
  throw new Error("no admission.answer");
};

test("open tells the core that the app passed an admission function", async () => {
  const withFn = fake();
  await connect(withFn.core, { model, lock, admission: async () => "x" });
  expect(withFn.calls[0]?.args).toMatchObject({ admission: true });
  const without = fake();
  await connect(without.core, { model, lock });
  expect(without.calls[0]?.args.admission).toBeUndefined();
});

test("the SDK calls the function with the identity and answers the core", async () => {
  const { core, calls, ask } = fake();
  const seen: string[] = [];
  await connect(core, {
    model,
    lock,
    admission: async (identity) => {
      seen.push(identity);
      return "ncdb-admit-1:AA";
    },
  });
  ask(7);
  const answer = await answered(calls);
  expect(seen).toEqual([IDENTITY]);
  expect(answer).toEqual({
    call: "admission.answer",
    args: { request: 7, admission: "ncdb-admit-1:AA" },
  });
});

test("a function that throws answers null", async () => {
  const { core, calls, ask } = fake();
  await connect(core, {
    model,
    lock,
    admission: async () => {
      throw new Error("the backend is down");
    },
  });
  ask(3);
  expect(await answered(calls)).toEqual({
    call: "admission.answer",
    args: { request: 3, admission: null },
  });
});
