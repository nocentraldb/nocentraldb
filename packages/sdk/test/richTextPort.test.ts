// The binding's port onto the bridge (NOC-461, 4b): it opens a binding
// with `richText.open` and speaks `richText.pull`, `richText.write` and
// `richText.close` for its lease, as crates/nocentraldb/src/bridge/mod.rs
// documents them. A fake `ask` records each call.
import { expect, test } from "vitest";
import { openRichText } from "../src/richText/port.ts";

const target = {
  space: "0123456789abcdef0123456789abcdef",
  collection: "notes",
  id: "fedcba9876543210fedcba9876543210",
  field: "body",
};

const fakeAsk = () => {
  const calls: { call: string; args: Record<string, unknown> }[] = [];
  const ask = async (call: string, args: Record<string, unknown>) => {
    calls.push({ call, args });
    if (call === "richText.open") {
      return {
        lease: 3,
        client: 42,
        clock: 0,
        writable: true,
        state: new Uint8Array([0, 0]),
      };
    }
    if (call === "richText.pull") return new Uint8Array([0, 0]);
    if (call === "richText.write") return { change: "c".repeat(64) };
    return null;
  };
  return { ask, calls };
};

test("open asks for the field and answers the lease's client id, state and writability", async () => {
  const { ask, calls } = fakeAsk();
  const { opened } = await openRichText(ask, target);
  expect(calls).toEqual([{ call: "richText.open", args: target }]);
  expect(opened).toEqual({
    client: 42,
    state: new Uint8Array([0, 0]),
    writable: true,
  });
});

test("the port pulls, writes and closes under the lease's handle", async () => {
  const { ask, calls } = fakeAsk();
  const { port } = await openRichText(ask, target);
  const sv = new Uint8Array([1, 2]);
  const update = new Uint8Array([3, 4]);
  expect(await port.pull(sv)).toEqual(new Uint8Array([0, 0]));
  await port.write(update);
  await port.close();
  expect(calls.slice(1)).toEqual([
    { call: "richText.pull", args: { lease: 3, stateVector: sv } },
    { call: "richText.write", args: { lease: 3, update } },
    { call: "richText.close", args: { lease: 3 } },
  ]);
});
