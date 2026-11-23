// Where the SDK records, in a browser, each database it opened (NOC-445;
// storageLost.ts says why). The record outside OPFS is a key in the
// IndexedDB database `nocentraldb-opened`, object store `names`; its twin
// on OPFS is an empty file in the directory `.nocentraldb-opened`, both
// named after the database. IndexedDB outlives the drop of a Chromium
// private window's OPFS after ten quiet minutes (measured in Chromium 157,
// docs/browser-storage.md), as sessionStorage and localStorage do;
// IndexedDB was taken because a worker reaches it too, and the member's
// keys already live there, so the record goes exactly when they go.
// Internal: the package's entry does not reach it.

import type { Markers } from "./storageLost.ts";

/** The IndexedDB database that holds the records, and its object store. */
const DATABASE = "nocentraldb-opened";
const STORE = "names";
/** The directory on OPFS that holds the twins. */
const TWINS = ".nocentraldb-opened";

/** The result of `request`, once it succeeded. */
const done = <T>(request: IDBRequest<T>) =>
  new Promise<T>((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });

/** Runs `use` on the records' object store, in a transaction of `mode`, and waits for it to commit. */
const withStore = async <T>(
  mode: IDBTransactionMode,
  use: (store: IDBObjectStore) => IDBRequest<T>,
): Promise<T> => {
  const opening = indexedDB.open(DATABASE, 1);
  opening.onupgradeneeded = () => {
    opening.result.createObjectStore(STORE);
  };
  const db = await done(opening);
  try {
    const transaction = db.transaction(STORE, mode);
    const result = done(use(transaction.objectStore(STORE)));
    await new Promise<void>((resolve, reject) => {
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error);
      transaction.onabort = () => reject(transaction.error);
    });
    return await result;
  } finally {
    db.close();
  }
};

/** The twins' directory on OPFS; `create` makes it when it is missing. */
const twins = async (create: boolean) =>
  (await navigator.storage.getDirectory()).getDirectoryHandle(TWINS, {
    create,
  });

/** The records of this browser context, or undefined where it has no IndexedDB or no OPFS. */
export const browserMarkers = (): Markers | undefined => {
  if (
    typeof indexedDB === "undefined" ||
    typeof navigator === "undefined" ||
    typeof navigator.storage?.getDirectory !== "function"
  ) {
    return undefined;
  }
  return {
    recorded: async (name) =>
      (await withStore("readonly", (store) => store.count(name))) > 0,
    present: async (name) => {
      try {
        await (await twins(false)).getFileHandle(name);
        return true;
      } catch (error) {
        if ((error as DOMException).name === "NotFoundError") return false;
        throw error;
      }
    },
    record: async (name) => {
      await (await twins(true)).getFileHandle(name, { create: true });
      await withStore("readwrite", (store) => store.put(true, name));
    },
  };
};
