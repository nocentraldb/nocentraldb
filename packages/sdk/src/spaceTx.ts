// The transaction a space's write records (RFC 0009, "One write, one
// change"): each create, update, delete and restore as an op of the
// bridge's `write`, in the order recorded.

import type { Tx } from "./db.ts";

type AnyCollection = Parameters<Tx["delete"]>[0];

/** The ops `record` records, with `nameOf` naming a collection, `values` turning written values into the bridge's and `randomId` drawing a new object's id. */
export const recordOps = (
  record: (tx: Tx) => void,
  nameOf: (collection: AnyCollection) => string,
  values: (collection: string, written: Record<string, unknown>) => unknown,
  randomId: () => string,
): Record<string, unknown>[] => {
  const ops: Record<string, unknown>[] = [];
  const tx: Tx = {
    create: (collection, written) => {
      const name = nameOf(collection);
      const created = randomId();
      ops.push({
        op: "create",
        collection: name,
        id: created,
        values: values(name, written),
      });
      return created;
    },
    update: (collection, object, written) => {
      const name = nameOf(collection);
      ops.push({
        op: "update",
        collection: name,
        id: object,
        values: values(name, written),
      });
    },
    delete: (collection, object) => {
      ops.push({ op: "delete", collection: nameOf(collection), id: object });
    },
    restore: (collection, object) => {
      ops.push({ op: "restore", collection: nameOf(collection), id: object });
    },
  };
  record(tx);
  return ops;
};
