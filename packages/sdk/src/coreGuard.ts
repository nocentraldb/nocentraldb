// What the SDK's worker (worker.ts) answers in the core's place when the
// core fails: the module did not load, or the core trapped, as WebAssembly
// does when it runs out of memory. Every request still gets an answer, so
// the page never waits for one that does not come. Internal: the package's
// entry does not reach it.

import { decode, encode } from "./cbor.ts";
import * as texts from "./texts.ts";

/** The core as the worker calls it: one bridge request in, its answer out. */
export type Handle = (request: Uint8Array) => Promise<Uint8Array>;

/** The id of `request`, or 0 when it has none that can be read. */
const idOf = (request: Uint8Array): number => {
  try {
    const { id } = decode(request) as { id?: unknown };
    return typeof id === "number" ? id : 0;
  } catch {
    return 0;
  }
};

/** A `storageFailed` answer to `request`. */
const failure = (request: Uint8Array, message: string) =>
  encode({ id: idOf(request), error: { code: "storageFailed", message } });

/** Answers each request with the core that `ready` gives, or with `storageFailed` when the core fails. */
export const guardCore = (ready: Promise<Handle>) => ({
  answer: async (request: Uint8Array): Promise<Uint8Array> => {
    try {
      return await (await ready)(request);
    } catch (error) {
      return failure(request, texts.coreFailed(String(error)));
    }
  },
});
