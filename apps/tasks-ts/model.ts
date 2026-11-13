// The schema of the browser task list, written once in TypeScript
// (RFC 0005). `ncdb model build` turns it into model.lock.json, which the
// app opens its database with; rebuild the lock after every change here.
import { nc } from "@nocentraldb/sdk";

export const model = nc.model({
  tasks: nc.collection(
    {
      title: nc.string(),
      status: nc.oneOf(["open", "done"]),
    },
    {
      indexes: {
        // The tasks of one status, in every space of the device, by
        // title: a query fixes the status with `.equal(...)`.
        byStatus: nc.index(["status", "title"], { equal: 1 }),
      },
    },
  ),
});
