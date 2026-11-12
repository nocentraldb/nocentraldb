// The schema of the example task list (NOC-237), written once in
// TypeScript (RFC 0005). `ncdb model build` turns it into model.lock.json,
// which the Rust app reads; rebuild the lock after every change here.
import { nc } from "@nocentraldb/sdk";

export const model = nc.model({
  tasks: nc.collection(
    {
      title: nc.string(),
      status: nc.oneOf(["open", "done"]),
    },
    {
      indexes: {
        // Every task of every space of the device, by status, then by
        // title.
        byStatus: nc.index(["status", "title"]),
      },
    },
  ),
});
