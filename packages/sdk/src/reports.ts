// The sync status and a space's status as the core sends them (NOC-176
// slice 4; RFC 0009, "Sync and status"), read into the SDK's types, and the
// listeners that follow them. connect.ts keeps the newest of each.

import type { SpaceStatus } from "./status.ts";
import type { SyncStatus } from "./sync.ts";

/**
 * The relay's URL as the SDK shows it: without the user and password before
 * its host, and without its query string and fragment, any of which may
 * carry a credential for the relay's authorizer (NOC-495). The core still
 * connects with the whole URL.
 */
const shownRelay = (url: string): string => {
  const cut = url.split(/[?#]/, 1)[0] ?? url;
  const start = cut.indexOf("://") + 3;
  if (start < 3) return cut;
  const end = cut.indexOf("/", start);
  const host = end < 0 ? cut.slice(start) : cut.slice(start, end);
  const at = host.lastIndexOf("@");
  return at < 0 ? cut : cut.slice(0, start) + cut.slice(start + at + 1);
};

/** The sync status the core sent, with the relay that serves the database, shown by `shownRelay`. */
export const syncStatusOf = (
  raw: unknown,
  relay: string | undefined,
): SyncStatus => {
  const given = (raw ?? {}) as Partial<Record<keyof SyncStatus, unknown>>;
  return {
    connection: (given.connection ?? "none") as SyncStatus["connection"],
    ...(relay === undefined ? {} : { relay: shownRelay(relay) }),
    unconfirmed: Number(given.unconfirmed ?? 0),
    refused: Number(given.refused ?? 0),
    epochChanged: given.epochChanged === true,
    ...(typeof given.error === "string" ? { error: given.error } : {}),
  };
};

/** A space's status the core sent. */
export const spaceStatusOf = (raw: unknown): SpaceStatus => {
  const given = (raw ?? {}) as Record<string, unknown>;
  const list = <T>(value: unknown) =>
    (Array.isArray(value) ? value : []) as T[];
  const counts = (value: unknown) =>
    Object.fromEntries(
      Object.entries((value ?? {}) as Record<string, unknown>).map(
        ([reason, count]) => [reason, Number(count)],
      ),
    );
  return {
    role: (given.role ?? "removed") as SpaceStatus["role"],
    initialSyncComplete: given.initialSyncComplete === true,
    behind: Number(given.behind ?? 0),
    gaps: list<string>(given.gaps),
    parked: counts(given.parked),
    ...(typeof given.newestModel === "number"
      ? { newestModel: given.newestModel }
      : {}),
    rejected: counts(given.rejected),
    refused: list<{ change: string; code: string }>(given.refused),
    lostGrants: list<string>(given.lostGrants),
    equivocations: list<string>(given.equivocations),
    exposed: given.exposed === true,
    waitingForKey: given.waitingForKey === true,
    badPersonKeyRecords: list<string>(given.badPersonKeyRecords),
    provisionalErasures: list<SpaceStatus["provisionalErasures"][number]>(
      given.provisionalErasures,
    ).map((erasure) => ({ ...erasure, age: Number(erasure.age) })),
    onlyMember: given.onlyMember === true,
  };
};

/** Gives `value` to each listener; one that throws loses it, and the others still get it. */
export const notify = <T>(listeners: Set<(value: T) => void>, value: T) => {
  for (const listener of [...listeners]) {
    try {
      listener(value);
    } catch {
      // The app's bug; it does not stop the other listeners.
    }
  }
};
