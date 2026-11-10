// The model of the Rust SDK's tests (NOC-88). `ncdb model build` wrote
// model.lock.json from it, and the index `open` then got `"scope": "space"`
// in the lock by hand: `nc.index` in TypeScript does not carry `scope` yet
// (RFC 0005 Amendment 3), so a build would leave it out. `open` runs in one
// space (RFC 0006, Amendment 6); `byPriority` and `byStatusPriority` span
// every space of the member.
import { nc } from "../../../../packages/sdk/src/index.ts";

export const model = nc.model({
  tasks: nc.collection(
    {
      title: nc.string(),
      status: nc.oneOf(["open", "done"]),
      priority: nc.int(),
      estimate: nc.float(),
      urgent: nc.boolean(),
      tags: nc.set(nc.string()),
      project: nc.link("projects"),
    },
    {
      indexes: {
        byPriority: nc.index(["priority"]),
        byStatusPriority: nc.index(["status", "priority"], { equal: 1 }),
        open: nc.index(["status", "priority"], { equal: 1, scope: "space" }),
      },
    },
  ),
  projects: nc.collection({ name: nc.string() }),
});
