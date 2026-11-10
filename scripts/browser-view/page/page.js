// The page of the browser view test (NOC-174): it starts the harness in a
// dedicated worker and lets Playwright call it, one export at a time.
// `onMain` calls the same harness on the page's main thread, where the
// view must refuse to open (RFC 0006, "The browser").

let worker = null;
let next = 0;
const waiting = new Map();

const start = () => {
  worker = new Worker("worker.js", { type: "module" });
  worker.onmessage = ({ data }) => {
    const { resolve, reject } = waiting.get(data.id);
    waiting.delete(data.id);
    if ("error" in data) reject(new Error(data.error));
    else resolve(data.ok);
  };
};

/** Calls the harness export `call` in the worker, starting it if needed. */
window.inWorker = (call, ...args) => {
  if (!worker) start();
  const id = next++;
  return new Promise((resolve, reject) => {
    waiting.set(id, { resolve, reject });
    worker.postMessage({ id, call, args });
  });
};

/** Ends the worker, and with it every handle it holds. */
window.endWorker = () => {
  worker?.terminate();
  worker = null;
};

/** Calls the harness export `call` on the main thread. */
window.onMain = async (call, ...args) => {
  const harness = await import("./pkg/browser_view.js");
  await harness.default();
  return harness[call](...args);
};

window.ready = true;
