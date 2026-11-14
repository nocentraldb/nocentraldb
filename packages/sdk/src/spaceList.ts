// The space list as a report (NOC-315; RFC 0009, "Spaces"): the first
// `db.spaces.subscribe` asks the core's `spaces.follow`, and the core's
// `spaces` events keep the newest list, so a later subscriber gets it at
// once. Internal: the package's entry does not reach it.

import type { Cbor } from "./cbor.ts";
import { notify } from "./reports.ts";

type Ask = (call: string, args: Record<string, unknown>) => Promise<Cbor>;

/** The follower of the space list of one database. */
export const spaceListOf = <E>(ask: Ask) => {
  const listeners = new Set<(spaces: E[]) => void>();
  let newest: E[] | undefined;
  let following = false;

  const changed = (raw: unknown) => {
    newest = (Array.isArray(raw) ? raw : []) as E[];
    notify(listeners, newest);
  };
  const follow = () =>
    ask("spaces.follow", {}).then(changed, () => {
      // A closed database answers nothing more; its listeners are gone.
    });

  return {
    /** Calls `listener` with the list at once, or once the core answered the first follow, and with each new list. */
    subscribe(listener: (spaces: E[]) => void): () => void {
      listeners.add(listener);
      if (!following) {
        following = true;
        void follow();
      } else if (newest !== undefined) {
        notify(new Set([listener]), newest);
      }
      return () => {
        listeners.delete(listener);
      };
    },
    /** A `spaces` event of the core. */
    changed,
    /** Another tab took the database over (NOC-177): its core follows no list yet. */
    reopened() {
      if (following) void follow();
    },
    /** The database closed: no listener hears anything more. */
    clear() {
      listeners.clear();
    },
  };
};
