/** Returns the package name; exists so the gates have real code to check. */
export function name(): string {
  return "@nocentraldb/sdk";
}

/** The TypeScript types of a database, a space and a transaction (RFC 0009, "Public names"; Carsten, 2026-11-12, NOC-274). */
/** The TypeScript types of a query and its page (RFC 0009, "Public names"; NOC-176 slice 2). */
export type { Db, Page, Query, Space, Tx } from "./db.ts";
/** The schema functions of RFC 0005: `nc.model`, `nc.collection`, the field types and `nc.index`, and the types `nc.Infer` and `nc.Unknown`. */
export * as nc from "./nc.ts";
