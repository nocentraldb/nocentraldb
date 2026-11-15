// Types and helpers of connect.ts, in a module of their own to keep
// connect.ts under the file size limit (NOC-315). Internal: the package's
// entry does not reach it.

import type { Db } from "./db.ts";

type Id = string;

/** One fold the core reports: its space, the collections whose reads it may have changed, and the ids it touched per collection with their origin. */
export type Fold = {
  space: string;
  collections: string[];
  changes?: {
    collection: string;
    ids: string[];
    origin: "local" | "remote";
  }[];
};

/** What `db.changes.subscribe` gives a listener. */
export type Change = Parameters<Parameters<Db["changes"]["subscribe"]>[0]>[0];

/** A read a subscription re-runs, and what it follows. */
export type Subscription = {
  collection: string;
  space: string | undefined;
  run: () => Promise<void>;
  running: boolean;
  again: boolean;
};

/** An object as the core answers it, with its fields beside `$id`, `$space` and `$deleted`. */
export const flatten = ({
  fields,
  ...rest
}: {
  fields: Record<string, unknown>;
}): Record<string, unknown> => ({ ...rest, ...fields });

/** 128 random bits from the browser's secure generator, as 32 hex digits (RFC 0002, "Object ids"). */
export const randomId = (): Id => {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  return [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");
};
