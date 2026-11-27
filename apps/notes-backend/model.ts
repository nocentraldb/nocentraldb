// The schema of the notes app whose backend this is, as the guide on spaces
// and invitations (docs/spaces-and-invitations.md) shows it. `ncdb model
// build` turns it into model.lock.json, which the journey test's devices
// read; rebuild the lock after every change here.
import { nc } from "@nocentraldb/sdk";

export const model = nc.model({
  // One object in each of the app's spaces, which says what the space is
  // for: a user's personal space, a project, or a note shared on its own.
  // The SDK names no space; the account space is the SDK's and holds none.
  spaceInfo: nc.collection(
    {
      kind: nc.oneOf(["personal", "project", "note"]),
      name: nc.string(),
    },
    {
      indexes: {
        // The spaces of every kind this device holds, by kind and name.
        byKind: nc.index(["kind", "name"]),
      },
    },
  ),
  notes: nc.collection(
    { title: nc.string() },
    { indexes: { byTitle: nc.index(["title"]) } },
  ),
});
