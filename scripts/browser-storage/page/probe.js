// The probe page's API, on window.probe, for Playwright (measure.mjs) and for
// the buttons a person presses in a manual check. Every call answers with
// plain data; a failure becomes "error: <name>: <message>", never a throw.

const describe = (error) =>
  `${error?.name ?? "Error"}: ${error?.message ?? String(error)}`;

let worker;
let nextId = 0;
const pending = new Map();

const startWorker = () => {
  worker = new Worker("worker.js", { type: "module" });
  worker.onmessage = ({ data }) => {
    const call = pending.get(data.id);
    pending.delete(data.id);
    if (data.ok) call.resolve(data.value);
    else
      call.reject(Object.assign(new Error(data.message), { name: data.name }));
  };
  worker.onerror = (event) => {
    for (const call of pending.values()) {
      call.reject(new Error(`worker failed: ${event.message ?? "unknown"}`));
    }
    pending.clear();
  };
};

const sqlite = (op, args = {}) =>
  new Promise((resolve, reject) => {
    if (!worker) startWorker();
    const id = nextId++;
    pending.set(id, { resolve, reject });
    worker.postMessage({ id, op, ...args });
  });

const IDB_NAME = "ncdb-probe";

const idb = (mode, action) =>
  new Promise((resolve, reject) => {
    const request = indexedDB.open(IDB_NAME, 1);
    request.onupgradeneeded = () => request.result.createObjectStore("probe");
    request.onerror = () => reject(request.error);
    request.onsuccess = () => {
      const db = request.result;
      const tx = db.transaction("probe", mode);
      const store = tx.objectStore("probe");
      const op = action(store);
      tx.oncomplete = () => {
        db.close();
        resolve(op.result ?? null);
      };
      tx.onerror = () => {
        db.close();
        reject(tx.error);
      };
    };
  });

const settle = async (promise) => {
  try {
    return await promise;
  } catch (error) {
    return `error: ${describe(error)}`;
  }
};

const withTimeout = (promise, ms) =>
  Promise.race([
    promise.then((value) => ({ answered: true, value })),
    new Promise((resolve) =>
      setTimeout(() => resolve({ answered: false }), ms),
    ),
  ]);

const probe = {
  /** Writes the token to SQLite on OPFS and to IndexedDB. */
  async write(token) {
    return {
      sqlite: await settle(sqlite("write", { token })),
      idb: await settle(
        idb("readwrite", (store) => store.put(token, "token")).then(() => "ok"),
      ),
    };
  },
  /** Reads the token back from both: the value, null, or an error. */
  async read() {
    return {
      sqlite: await settle(sqlite("read")),
      idb: await settle(idb("readonly", (store) => store.get("token"))),
    };
  },
  /** Calls navigator.storage.persist(); a prompt shows as no answer. */
  async persist(timeoutMs = 5000) {
    const result = await withTimeout(
      settle(navigator.storage.persist()),
      timeoutMs,
    );
    return result.answered
      ? { answered: true, granted: result.value === true, raw: result.value }
      : { answered: false, granted: null };
  },
  /** What the Storage API and SQLite report. */
  async info() {
    const estimate = await settle(navigator.storage.estimate());
    return {
      userAgent: navigator.userAgent,
      secureContext: isSecureContext,
      persisted: await settle(navigator.storage.persisted()),
      estimate:
        typeof estimate === "string"
          ? estimate
          : { usage: estimate.usage, quota: estimate.quota },
      opfs: await settle(navigator.storage.getDirectory().then(() => "ok")),
      sqliteVersion: await settle(sqlite("version")),
    };
  },
};

window.probe = probe;

// The buttons, for a manual check: Write stores the current time, so a read
// days later shows when the data was written.
const out = document.getElementById("out");
const show = async (label, promise) => {
  out.textContent = `${label}…`;
  const value = await promise;
  out.textContent = `${label} (${new Date().toISOString()}):\n${JSON.stringify(value, null, 2)}`;
};
const on = (id, action) =>
  document.getElementById(id)?.addEventListener("click", action);
on("write", () =>
  show("Write", probe.write(`written ${new Date().toISOString()}`)),
);
on("read", () => show("Read", probe.read()));
on("persist", () => show("persist()", probe.persist(60_000)));
on("info", () => show("Storage info", probe.info()));
show("Read on load", probe.read());
