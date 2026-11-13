// The runtime of queries and cursors (RFC 0009, "Queries and indexes";
// NOC-176 slice 2). A query is an immutable builder on a collection handle
// and an index name; a cursor is an opaque handle of the position the core
// keeps in the worker, tied to the database that made it, because a
// cursor holds plaintext field values and never leaves the device (RFC
// 0006, "Ordered pages"). Internal: the package's entry does not reach
// this module, and the app sees only the types `Query` and `Page`.

/** What a query asks: the collection handle, the index, and the page. */
export type QueryState = {
  collection: object;
  index: string;
  equal?: unknown[];
  take?: number;
  after?: Cursor;
};

const queries = new WeakMap<object, QueryState>();

/** A query of the index `index` of `collection`, from `state` on. */
const query = (state: QueryState): object => {
  const made = {
    equal: (...values: unknown[]) => query({ ...state, equal: values }),
    take: (n: number) => query({ ...state, take: n }),
    after: (cursor: Cursor) => query({ ...state, after: cursor }),
  };
  queries.set(made, state);
  return made;
};

/** The query of the first page of the index `index` of `collection`. */
export const makeQuery = (collection: object, index: string): object =>
  query({ collection, index });

/** What `query` asks; `undefined` for an object that is no query. */
export const queryState = (made: object): QueryState | undefined =>
  queries.get(made);

const REDACTED = "[redacted]";

/** Where the page after another starts: a handle the core holds, of the database `db`. It prints as `[redacted]`. */
export class Cursor {
  readonly #db: object;
  readonly #handle: number;

  constructor(db: object, handle: number) {
    this.#db = db;
    this.#handle = handle;
  }

  /** The core's handle, when the cursor belongs to `db`. */
  handleIn(db: object): number | undefined {
    return this.#db === db ? this.#handle : undefined;
  }

  toString(): string {
    return REDACTED;
  }

  toJSON(): string {
    return REDACTED;
  }

  [Symbol.for("nodejs.util.inspect.custom")](): string {
    return `Cursor(${REDACTED})`;
  }
}
