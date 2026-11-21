// What the worker answers once its core failed (NOC-426): a core that
// traps, as WebAssembly does when a write is too large for its memory, is
// broken for good, so the request it was answering fails with
// `storageFailed`, and so does every later one, with a text that tells the
// app to open the database again. A `close` still succeeds, so the app can
// close and open it.
import { expect, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { guardCore, type Handle } from "../src/coreGuard.ts";
import * as texts from "../src/texts.ts";

const request = (id: number, call = "get") => encode({ id, call, args: {} });

const answered = async (answer: Promise<Uint8Array>) =>
  decode(await answer) as {
    id: number;
    ok?: unknown;
    error?: { code: string; message: string };
  };

/** A core that answers `ok: id` until `trap` is set, and then throws as a trap does. */
const core = () => {
  const state = { trap: false, calls: 0 };
  const handle: Handle = async (bytes) => {
    state.calls += 1;
    if (state.trap) throw new Error("RuntimeError: unreachable");
    const { id } = decode(bytes) as { id: number };
    return encode({ id, ok: id });
  };
  return { state, handle };
};

test("a core that answers is passed through", async () => {
  const { handle } = core();
  const guard = guardCore(Promise.resolve(handle));
  expect(await answered(guard.answer(request(1)))).toEqual({ id: 1, ok: 1 });
});

test("after the core traps, every later call fails with storageFailed and a text that says to open the database again, without the core", async () => {
  const { state, handle } = core();
  const guard = guardCore(Promise.resolve(handle));
  state.trap = true;
  const first = await answered(guard.answer(request(1)));
  expect(first.id).toBe(1);
  expect(first.error?.code).toBe("storageFailed");
  expect(first.error?.message).toBe(
    texts.coreFailed("Error: RuntimeError: unreachable"),
  );

  state.trap = false;
  const later = await answered(guard.answer(request(2)));
  expect(later).toEqual({
    id: 2,
    error: { code: "storageFailed", message: texts.CORE_STOPPED },
  });
  expect(state.calls).toBe(1);
});

test("after the core traps, a close succeeds, so the app can open the database again", async () => {
  const { state, handle } = core();
  const guard = guardCore(Promise.resolve(handle));
  state.trap = true;
  await guard.answer(request(1));
  expect(await answered(guard.answer(request(2, "close")))).toEqual({
    id: 2,
    ok: null,
  });
});
