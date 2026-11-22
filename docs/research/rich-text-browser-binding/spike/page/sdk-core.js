// A core over the real TypeScript SDK, for the latency: nc.richText is not
// built yet, so each change the binding writes is one task object of the
// test lock, the Yjs update as base64 in its title, and the fold is a Y.Doc
// in the page that applies every such object the tab learns of. The path
// from one tab to another, or one browser to another, is the SDK's own:
// space.create in the worker, the fold into the database on OPFS, db.changes
// in the other tab, the relay between browsers.
import * as Y from "yjs";

const toBase64 = (bytes) => {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s);
};
const fromBase64 = (text) =>
  Uint8Array.from(atob(text), (c) => c.charCodeAt(0));

/** A core on `db`'s space `spaceId`; `model` is the test lock's model. */
export const sdkCore = async ({ db, model, spaceId }) => {
  const space = await db.spaces.get(spaceId);
  const doc = new Y.Doc();
  const applied = new Set();
  const subscribers = new Set();
  const arrivals = [];
  let seq = 0;

  const applyIds = async (ids) => {
    let any = false;
    for (const id of ids) {
      if (applied.has(id)) continue;
      const task = await space.get(model.tasks, id);
      if (!task || typeof task.title !== "string") continue;
      applied.add(id);
      Y.applyUpdate(doc, fromBase64(task.title));
      any = true;
    }
    return any;
  };

  // Every task of the space, as a reader that opens late sees them.
  const loadAll = async () => {
    const ids = [];
    const query = model.tasks.indexes.open.equal("open").take(1000);
    let page = await space.find(query);
    for (;;) {
      ids.push(...page.items.map((t) => t.$id));
      if (!page.hasMore) break;
      page = await space.find(query.after(page.cursor));
    }
    await applyIds(ids);
  };
  await loadAll();

  const notify = () => {
    for (const fn of [...subscribers]) fn();
  };

  const stop = db.changes.subscribe(async (change) => {
    if (change.space !== spaceId || change.collection !== "tasks") return;
    const at = performance.timeOrigin + performance.now();
    if (await applyIds(change.ids)) {
      arrivals.push({ at, origin: change.origin, ids: change.ids.length });
      notify();
    }
  });

  return {
    doc,
    arrivals,
    state: async () => Y.encodeStateAsUpdate(doc),
    diff: (stateVector) => Y.encodeStateAsUpdate(doc, stateVector),
    write: async (update) => {
      seq += 1;
      const { ids } = await space.create(model.tasks, {
        title: toBase64(update),
        status: "open",
        priority: seq,
      });
      for (const id of ids) applied.add(id);
      Y.applyUpdate(doc, update);
      notify();
    },
    subscribe: (fn) => {
      subscribers.add(fn);
      return () => subscribers.delete(fn);
    },
    close: () => stop(),
  };
};
