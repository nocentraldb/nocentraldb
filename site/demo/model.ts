// The schema of the landing page's demo, which began as the task list of
// apps/tasks-ts and now has a model of its own (NOC-360): each task carries
// its creation time, and the index lists the newest first. `ncdb model build`,
// run in this folder, turns it into model.lock.json, an additive successor of
// the first lock, so a visitor's stored database still opens (RFC 0005).
// public/demo/demo.js declares the same model. It imports the SDK's build
// output, since site/ is outside the workspace: build the SDK first.
import { nc } from "../../packages/sdk/dist/index.js";

export const model = nc.model({
  tasks: nc.collection(
    {
      title: nc.string(),
      status: nc.oneOf(["open", "done"]),
      createdAt: nc.int().describe("Milliseconds since the epoch."),
    },
    {
      indexes: {
        // The first lock's index, which the lock keeps.
        byStatus: nc.index(["status", "title"], { equal: 1 }),
        // The tasks of one status, newest first. A task made before the
        // creation time existed has none, which sorts last descending; the
        // title orders those among themselves.
        byStatusNewest: nc.index(
          ["status", { field: "createdAt", direction: "desc" }, "title"],
          { equal: 1 },
        ),
      },
    },
  ),
});
