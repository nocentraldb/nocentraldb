// What the SDK's worker (worker.ts) answers in the core's place when the
// core fails: the module did not load, or the core trapped, as WebAssembly
// does when it runs out of memory. Every request still gets an answer, so
// the page never waits for one that does not come. A core that failed is
// not called again: a trap leaves its memory as it was mid-call, a bridge
// taken out among it, so it is broken for good (NOC-426). Every later
// request fails with `storageFailed` and a text that tells the app to open
// the database again, which starts a new worker; a `close` succeeds, so the
// app can. Internal: the package's entry does not reach it.

import { decode, encode } from "./cbor.ts";
import * as texts from "./texts.ts";

/** The core as the worker calls it: one bridge request in, its answer out. */
export type Handle = (request: Uint8Array) => Promise<Uint8Array>;

/** The id and the call of `request`, as far as they can be read. */
const read = (request: Uint8Array): { id: number; call?: unknown } => {
  try {
    const { id, call } = decode(request) as { id?: unknown; call?: unknown };
    return { id: typeof id === "number" ? id : 0, call };
  } catch {
    return { id: 0 };
  }
};

/** A `storageFailed` answer to `request`. */
const failure = (request: Uint8Array, message: string) =>
  encode({ id: read(request).id, error: { code: "storageFailed", message } });

/** The answer to `request` once the core failed. */
const stopped = (request: Uint8Array) => {
  const { id, call } = read(request);
  return call === "close"
    ? encode({ id, ok: null })
    : encode({
        id,
        error: { code: "storageFailed", message: texts.CORE_STOPPED },
      });
};

/**
 * Answers each request with the core that `ready` gives, or with
 * `storageFailed` once the core failed. `failed` takes an error the core
 * threw outside a call, which the worker sees as an uncaught one: a trap in
 * a step the core runs after the call returned leaves that call's promise
 * unsettled, so the request it was answering gets its answer here.
 */
export const guardCore = (ready: Promise<Handle>) => {
  let broken = false;
  let waiting: ((error: unknown) => void) | undefined;
  return {
    answer: async (request: Uint8Array): Promise<Uint8Array> => {
      if (broken) return stopped(request);
      try {
        const core = await ready;
        return await new Promise<Uint8Array>((resolve, reject) => {
          waiting = reject;
          core(request).then(resolve, reject);
        });
      } catch (error) {
        broken = true;
        return failure(request, texts.coreFailed(String(error)));
      } finally {
        waiting = undefined;
      }
    },
    failed: (error: unknown) => {
      broken = true;
      waiting?.(error);
    },
  };
};
