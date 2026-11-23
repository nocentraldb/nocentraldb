// nc.open's `admission` option (RFC 0015, "In the SDK"; Carsten,
// 2026-11-23, NOC-493). The core asks for an admission with `{"event":
// "admission", "request": n, "identity": hex}` when the relay refuses a new
// space; the SDK calls the app's function with the identity and answers
// `admission.answer`, with null when the function throws or answers no
// text, so the space stays refused as without it. Neither the identity's
// answer nor an error is logged. Internal: the package's entry does not
// reach it.

import type { Cbor } from "./cbor.ts";

type Ask = (call: string, args: Record<string, unknown>) => Promise<Cbor>;

/** The handler of the core's `admission` events, for the app's `fetch`. */
export const admissionOf =
  (ask: Ask, fetch: ((identity: string) => Promise<string>) | undefined) =>
  (message: object): void => {
    const { request, identity } = message as {
      request?: unknown;
      identity?: unknown;
    };
    if (fetch === undefined || typeof identity !== "string") return;
    void (async () => {
      let admission: string | null = null;
      try {
        const text = await fetch(identity);
        admission = typeof text === "string" ? text : null;
      } catch {
        admission = null;
      }
      await ask("admission.answer", { request, admission }).catch(() => {});
    })();
  };
