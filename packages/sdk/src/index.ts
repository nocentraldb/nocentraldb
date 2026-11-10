/** Returns the package name; exists so the gates have real code to check. */
export function name(): string {
  return "@nocentraldb/sdk";
}

/** The schema functions of RFC 0005: `nc.model`, `nc.collection`, the field types and `nc.index`, and the types `nc.Infer` and `nc.Unknown`. */
export * as nc from "./nc.ts";
