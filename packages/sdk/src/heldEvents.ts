// The core's events on their way to `connect` (NOC-571): the core may ask
// the app's function while the database opens, such as its first `relay`
// ask (NOC-496), and `connect` listens only once `open` answered. Until a
// listener is set, the asks, which carry a request number and wait for an
// answer, are held, in order, and the listener gets them first. Any other
// event before it is dropped, as before: it reports a state, and the
// answer to `open`, which comes after it, carries a newer one.
// Internal: the worker host and the tabs layer use it.

import { decode } from "./cbor.ts";

/** Whether `event` is an ask of the core, which waits for its answer. */
const isAsk = (event: Uint8Array): boolean => {
  try {
    const message = decode(event) as { request?: unknown } | null;
    const request = message?.request;
    return typeof request === "number" || typeof request === "bigint";
  } catch {
    return false;
  }
};

/** Asks held until `listen` sets their listener, then every event passed on as it comes. */
export const heldEvents = () => {
  let listener: ((event: Uint8Array) => void) | undefined;
  const held: Uint8Array[] = [];
  return {
    emit: (event: Uint8Array) => {
      if (listener !== undefined) listener(event);
      else if (isAsk(event)) held.push(event);
    },
    listen: (given: (event: Uint8Array) => void) => {
      listener = given;
      for (const event of held.splice(0)) given(event);
    },
  };
};
