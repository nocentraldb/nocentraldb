// nc.open checks the model against its lock (NOC-323; RFC 0009, "Open and
// close", 2. "Checks the model against the lock"): every collection,
// field and index the model names is in the lock with the same type and
// options. The core folds by the lock alone, so a model changed without
// `ncdb model build` would write fields no lock knows. The model may name
// less than the lock: an app need not declare every collection. A field's
// description and deprecation are no type or option, and are not
// compared. Internal: the package's entry does not reach it.

import * as texts from "./texts.ts";

/** The key under which `nc.model` keeps the function that describes the model, as `ncdb model build` reads it. */
const DESCRIBE = Symbol.for("nocentraldb.model.describe");

type Json = unknown;
type Entries = Record<string, Record<string, Json> | undefined>;

/** What a field's type is in the lock and in the model's description: the type, with its options, targets, element and person. */
const shape = (field: Record<string, Json>): Json => ({
  type: field.type,
  options: field.options,
  targets: field.targets,
  of: field.of,
  personal: field.personal,
});

/** What an index is: its fields with their directions, its equality prefix and its scope. */
const indexShape = (index: Record<string, Json>): Json => ({
  fields: index.fields,
  equal: index.equal ?? 0,
  scope: index.scope ?? "member",
});

/** A JSON value's text with every object's keys sorted, as the lock writes them, so that key order does not count. */
const canonical = (value: Json): string =>
  JSON.stringify(value, (_key, inner: Json) =>
    inner !== null && typeof inner === "object" && !Array.isArray(inner)
      ? Object.fromEntries(
          Object.entries(inner as Record<string, Json>).sort(([a], [b]) =>
            a < b ? -1 : a > b ? 1 : 0,
          ),
        )
      : inner,
  );

/** Whether two JSON values are equal, whatever the order of their keys. */
const same = (a: Json, b: Json): boolean => canonical(a) === canonical(b);

/** Why `lock` is out of date for `model`, or `undefined` when every collection, field and index the model names is in the lock as the model names it. */
export const lockOutOfDate = (
  model: object,
  lock: Json,
): string | undefined => {
  const describe = (model as { [DESCRIBE]?: () => { collections: Entries } })[
    DESCRIBE
  ];
  if (describe === undefined) return undefined;
  const locked = ((lock as { collections?: Entries } | null)?.collections ??
    {}) as Entries;
  for (const [name, collection] of Object.entries(describe().collections)) {
    const inLock = locked[name];
    if (inLock === undefined) return texts.lockHasNoCollection(name);
    const fields = (collection?.fields ?? {}) as Entries;
    const lockFields = (inLock.fields ?? {}) as Entries;
    for (const [field, described] of Object.entries(fields)) {
      const kept = lockFields[field];
      if (kept === undefined) return texts.lockHasNoField(name, field);
      if (!same(shape(described ?? {}), shape(kept)))
        return texts.lockFieldDiffers(name, field);
    }
    const indexes = (collection?.indexes ?? {}) as Entries;
    const lockIndexes = (inLock.indexes ?? {}) as Entries;
    for (const [index, described] of Object.entries(indexes)) {
      const kept = lockIndexes[index];
      if (kept === undefined) return texts.lockHasNoIndex(name, index);
      if (!same(indexShape(described ?? {}), indexShape(kept)))
        return texts.lockIndexDiffers(name, index);
    }
  }
  return undefined;
};
