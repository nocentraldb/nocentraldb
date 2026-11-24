// nc.open's `relay` as a function (NOC-496): the core asks for the relay
// URL with `{"event": "relay", "request": n}` before each connect; the SDK
// calls the app's function and answers `relay.answer`, with null when the
// function throws or answers no text, which the core reports as
// `relay_url_failed` and asks again after a backoff. Neither the URL nor
// an error is logged. Internal: the package's entry does not reach it.

import type { Cbor } from "./cbor.ts";

type Ask = (call: string, args: Record<string, unknown>) => Promise<Cbor>;

/** The handler of the core's `relay` events, for the app's `given`; `gave` hears each URL it answered. */
export const relayAskOf =
  (
    ask: Ask,
    given: (() => Promise<string>) | undefined,
    gave: (url: string) => void,
  ) =>
  (message: object): void => {
    const { request } = message as { request?: unknown };
    if (given === undefined) return;
    void (async () => {
      let relay: string | null = null;
      try {
        const url = await given();
        relay = typeof url === "string" ? url : null;
      } catch {
        relay = null;
      }
      if (relay !== null) gave(relay);
      await ask("relay.answer", { request, relay }).catch(() => {});
    })();
  };
