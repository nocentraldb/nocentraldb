// What makes a database the browser dropped visible to the app (NOC-445;
// RFC 0006, "What the SDK reports"). In an off-the-record context, a
// Chromium private window or Playwright's `browser.newContext()`, Chromium
// drops the origin's whole OPFS after about ten minutes without a file
// operation. The worker's keep-alive (keepAlive.ts, NOC-444) covers a
// database while it is open; once the app has closed it, nothing touches
// OPFS, and the files go. IndexedDB, where the member's keys live, stays
// (measured in Chromium 157, NOC-445). The next open would start an empty
// copy under the same keys, and nothing would tell the app.
//
// So the SDK records each database it opened twice: in IndexedDB, outside
// OPFS, and as a twin file on OPFS. An open that finds the record without
// its twin knows the files are gone, and refuses with `storageLost`
// before the worker starts, until the app opens with
// `ifStorageLost: "open-empty"`. The SDK cannot tell a private window from
// a normal one, so it records in every browser; where the browser deletes
// the origin's data as a whole, as at eviction or when the person clears
// it, the record goes with the files, and the next open is a first one.
// Internal: the package's entry does not reach it.

import { NcError } from "./error.ts";
import * as texts from "./texts.ts";

/** Where the SDK records the databases it opened: outside OPFS, and the twin on it. Internal; a test passes a fake. */
export interface Markers {
  /** Whether the record outside OPFS says this browser opened the database `name`. */
  recorded(name: string): Promise<boolean>;
  /** Whether OPFS still holds the twin of that record. */
  present(name: string): Promise<boolean>;
  /** Records that the database `name` was opened: the twin on OPFS first, then the record outside it. */
  record(name: string): Promise<void>;
}

/** What `nc.open` does with a database whose storage the browser dropped: `"refuse"`, the default, or `"open-empty"`. */
export type IfStorageLost = "refuse" | "open-empty";

/** Whether the files of the database `name` are gone while its record stayed; `false` when the markers cannot be read, so the core's own error says what is missing. */
const lost = async (markers: Markers, name: string) => {
  try {
    return (await markers.recorded(name)) && !(await markers.present(name));
  } catch {
    return false;
  }
};

/**
 * Opens the database `name` with `open`, unless its storage is lost and
 * the app did not accept that; records it once it is open. A record that
 * cannot be written is skipped: the database is open, and only a later
 * loss would go unreported.
 */
export const guardLoss = async <T>(
  markers: Markers | undefined,
  name: string,
  ifStorageLost: IfStorageLost | undefined,
  open: () => Promise<T>,
): Promise<T> => {
  if (markers === undefined) return open();
  if (ifStorageLost !== "open-empty" && (await lost(markers, name))) {
    throw new NcError("storageLost", texts.storageLost(name));
  }
  const db = await open();
  await markers.record(name).catch(() => {});
  return db;
};
