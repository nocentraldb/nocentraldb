// The model of the view's tests (NOC-77). `ncdb model build` writes
// model.lock.json from it; `crates/ncdb/tests/fixtures.rs` checks that the
// lock is still what the build writes. Both collections have an index named
// byName: index names are scoped per collection (RFC 0005). inSpace and
// inSpaceByTitle run in one space at a time (RFC 0005, "Indexes").
import { nc } from "../../../../packages/sdk/src/index.ts";

export const model = nc.model({
  task: nc.collection(
    {
      title: nc.string(),
      rank: nc.int(),
      tags: nc.set(nc.string()),
      owner: nc.link("project"),
    },
    {
      indexes: {
        byRank: nc.index(["rank"]),
        byTitle: nc.index(["title", { field: "rank", direction: "desc" }], { equal: 1 }),
        byName: nc.index(["title"]),
        inSpace: nc.index(["rank"], { scope: "space" }),
        inSpaceByTitle: nc.index(["title", { field: "rank", direction: "desc" }], {
          equal: 1,
          scope: "space",
        }),
      },
    },
  ),
  project: nc.collection(
    { name: nc.string() },
    { indexes: { byName: nc.index(["name"]) } },
  ),
});
