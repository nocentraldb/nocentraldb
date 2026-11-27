// The page's side of the worker that runs the core (RFC 0009, "Open and
// close"): it starts the worker shipped next to this module, sends it one
// bridge request at a time and hands back its answers, in order, and its
// events. Asked to end, it lets the worker end itself, so that it gives
// the database's storage back first (NOC-352). Internal: `connect` and the tabs layer reach it as a `Core`.

import type { Core } from "./connect.ts";
import { NcError } from "./error.ts";
import { heldEvents } from "./heldEvents.ts";
import * as texts from "./texts.ts";

/** What the page sends the worker, besides bridge requests, to end it (worker.ts). */
const STOP = "stop";

/** The worker that runs the core, shipped next to this module. */
export const startWorker = (): Core => {
  const worker = new Worker(new URL("./worker.js", import.meta.url), {
    type: "module",
    name: "nocentraldb",
  });
  const waiting: ((answer: Uint8Array) => void)[] = [];
  let failed: Error | undefined;
  const failing: ((error: Error) => void)[] = [];
  // Events before `connect` listens, such as the first `relay` ask while
  // the database opens, are held for it (NOC-571).
  const events = heldEvents();
  // The worker sends `{answer}` for each request, in order, and `{event}`
  // for what the core reports unasked.
  worker.onmessage = (
    message: MessageEvent<{ answer?: Uint8Array; event?: Uint8Array }>,
  ) => {
    const { answer, event } = message.data;
    if (event !== undefined) {
      events.emit(event);
      return;
    }
    failing.shift();
    if (answer !== undefined) waiting.shift()?.(answer);
  };
  worker.onerror = (event) => {
    failed = new NcError("storageFailed", texts.workerFailed(event.message));
    for (const fail of failing.splice(0)) fail(failed);
    waiting.length = 0;
  };
  return {
    call: (request) =>
      new Promise((resolve, reject) => {
        if (failed) return reject(failed);
        waiting.push(resolve);
        failing.push(reject);
        worker.postMessage(request, [request.buffer]);
      }),
    // The worker answers what it took, gives the database's storage back
    // and ends itself (NOC-352, worker.ts). Never ended from here, not
    // even late: the browser would free its storage after the lock passes
    // on, and the next tab's open would fail.
    terminate: () => worker.postMessage(STOP),
    listen: events.listen,
  };
};
