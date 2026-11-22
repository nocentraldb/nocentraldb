// A stand-in for the core inside one page: a Y.Doc that folds every write
// and tells every subscriber, the writer included. The real core folds with
// Yrs in a worker; for the echo and the three measures only what it hands
// the binding matters, and that is Yjs's update encoding either way.
import * as Y from "yjs";

export const pageCore = () => {
  const doc = new Y.Doc();
  const subscribers = new Set();
  const writes = [];
  return {
    doc,
    writes,
    state: async () => Y.encodeStateAsUpdate(doc),
    diff: (stateVector) => Y.encodeStateAsUpdate(doc, stateVector),
    write: async (update, { clientID }) => {
      writes.push({ clientID, bytes: update.length, update });
      Y.applyUpdate(doc, update);
      for (const notify of [...subscribers]) queueMicrotask(notify);
    },
    subscribe: (notify) => {
      subscribers.add(notify);
      return () => subscribers.delete(notify);
    },
  };
};
