// The editor binding of RFC 0013 ("An editor: the binding"), as small as the
// spike needs: a Y.Doc kept in step with a core, one change per pause in
// typing (500 ms idle, at most 2 s, and on hide), and what other members
// write applied with an origin of the binding's own, so it is not written
// back. `core` is either the in-page stand-in (core.js) or the real SDK
// (sdk-core.js); both fold every write and then tell every subscriber,
// the writer included, as db.changes does.
import * as Y from "yjs";

/** The origin of everything the binding applies from the core. */
export const FROM_CORE = Symbol("from the core");

const now = () => performance.timeOrigin + performance.now();

export class Binding {
  /**
   * @param {object} o
   * @param {object} o.core what folds and stores the field's updates
   * @param {number} o.clientID the leased Yjs client id
   * @param {boolean} [o.filterOwnOrigin] false turns the origin check off, to show the echo
   * @param {(tr: Y.Transaction) => string[]} [o.guard] what a local transaction deletes that the editor does not know
   */
  constructor({
    core,
    clientID,
    idleMs = 500,
    maxMs = 2000,
    filterOwnOrigin = true,
    guard = null,
  }) {
    this.core = core;
    this.clientID = clientID;
    this.idleMs = idleMs;
    this.maxMs = maxMs;
    this.filterOwnOrigin = filterOwnOrigin;
    this.guard = guard;
    this.listeners = new Set();
    this.stats = {
      localUpdates: 0,
      writes: [],
      refused: [],
      pulls: [],
      echoed: 0,
      reloads: 0,
    };
    this.doc = this.newDoc();
    this.stop = core.subscribe(() => this.pull());
    this.onHide = () => {
      if (document.visibilityState === "hidden") this.flush("hidden");
    };
    document.addEventListener("visibilitychange", this.onHide);
  }

  newDoc() {
    const doc = new Y.Doc();
    doc.clientID = this.clientID;
    this.pending = [];
    this.flagged = [];
    doc.on("update", (update, origin) => this.onUpdate(update, origin));
    doc.on("afterTransaction", (tr) => {
      if (tr.origin === FROM_CORE || !this.guard) return;
      this.flagged.push(...this.guard(tr));
    });
    return doc;
  }

  async open() {
    Y.applyUpdate(this.doc, await this.core.state(), FROM_CORE);
    return this;
  }

  /** Listens for `{ type }` events: "write", "pull", "reload". */
  on(listener) {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  emit(event) {
    for (const listener of this.listeners) listener(event);
  }

  onUpdate(update, origin) {
    if (origin === FROM_CORE) {
      if (this.filterOwnOrigin) return;
      this.stats.echoed += 1;
    }
    this.stats.localUpdates += 1;
    this.pending.push(update);
    if (this.pending.length === 1) this.firstAt = now();
    this.lastAt = now();
    clearTimeout(this.idle);
    this.idle = setTimeout(() => this.flush("idle"), this.idleMs);
    if (!this.max) this.max = setTimeout(() => this.flush("max"), this.maxMs);
  }

  /** Writes what the editor wrote since the last write, as one change. */
  async flush(reason) {
    clearTimeout(this.idle);
    clearTimeout(this.max);
    this.idle = null;
    this.max = null;
    if (this.pending.length === 0) return;
    const update = Y.mergeUpdates(this.pending);
    const parts = this.pending.length;
    const { firstAt, lastAt } = this;
    this.pending = [];
    const flagged = this.flagged;
    this.flagged = [];
    if (flagged.length > 0) {
      this.stats.refused.push({ reason, flagged, bytes: update.length });
      await this.reload();
      return;
    }
    const startedAt = now();
    this.onFlushStart?.(reason);
    await this.core.write(update, { clientID: this.clientID });
    const write = {
      reason,
      parts,
      bytes: update.length,
      clients: [
        ...new Set(Y.decodeUpdate(update).structs.map((s) => s.id.client)),
      ],
      firstAt,
      lastAt,
      startedAt,
      doneAt: now(),
    };
    this.stats.writes.push(write);
    this.emit({ type: "write", write });
  }

  /** Applies what the core holds and the binding does not. */
  pull() {
    const before = Y.encodeStateVector(this.doc);
    const diff = this.core.diff(before);
    let changed = false;
    const seen = () => {
      changed = true;
    };
    this.doc.on("update", seen);
    Y.applyUpdate(this.doc, diff, FROM_CORE);
    this.doc.off("update", seen);
    const pull = { at: now(), bytes: diff.length, changed };
    this.stats.pulls.push(pull);
    this.emit({ type: "pull", pull });
  }

  /** A refused write: a new Y.Doc from the core's state, which the editor must be rebuilt on. */
  async reload() {
    this.stats.reloads += 1;
    this.doc.destroy();
    this.doc = this.newDoc();
    Y.applyUpdate(this.doc, await this.core.state(), FROM_CORE);
    this.emit({ type: "reload" });
  }

  async close() {
    await this.flush("close");
    this.stop();
    document.removeEventListener("visibilitychange", this.onHide);
    this.doc.destroy();
  }
}
