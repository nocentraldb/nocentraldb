// The schema functions of RFC 0005 (docs/rfc/0005-schema-and-model-lock.md),
// as far as `ncdb model build` needs them: a developer declares the model
// with them, and `ncdb model build` reads the description that `model`
// keeps under a hidden key and turns it into model.lock.json. The rules of
// the model (names, personal fields, indexes, the additive-change rule) are
// checked by the core when it builds the lock, not here, so that the build
// and the device check apply the same rules.
//
// Only `nc` and its members are public names; the interfaces below are not
// exported, so an app cannot name them. Each type carries a brand (a key it
// is never given at run time), so a collection, a name and an index cannot be
// passed for one another.

/** A field of the model: its type, and the modifiers chained after it. */
interface Field {
  /** Marks the field as personal, about `person`: `"self"` or a link field of the same collection. */
  personal(options: { person: string }): Field;
  /** Adds a description, which is copied into the lock. */
  describe(text: string): Field;
  /** Hides the field from the app, and keeps it in the model and the data. */
  deprecated(reason: string): Field;
}

/** A collection of the model: its fields, and the places for computed fields, views and indexes. */
interface Collection {
  readonly "nc.collection": true;
}

/** A computed field or a view: named now, designed by a later RFC. */
interface Named {
  readonly "nc.named": true;
}

/** A declared index. */
interface Index {
  readonly "nc.index": true;
}

/** The key under which `model` keeps the function that describes the model for `ncdb model build`. */
const DESCRIBE = Symbol.for("nocentraldb.model.describe");

/** A field's type, as the lock names it. */
type Spec =
  | { kind: "basic"; type: string; options?: string[] }
  | { kind: "set"; of: Field }
  | { kind: "link"; type: "ncdb/link" | "ncdb/links"; targets: string[] }
  | { kind: "reserved"; name: string };

/** A field's type with its modifiers. */
type FieldState = {
  spec: Spec;
  person?: string;
  description?: string;
  deprecated?: string;
};

type CollectionState = {
  fields: Record<string, Field>;
  computed: Record<string, Named>;
  views: Record<string, Named>;
  indexes: Record<string, Index>;
};

type IndexState = {
  fields: { field: string; direction: "asc" | "desc" }[];
  equal: number;
  scope?: "space";
};

const fieldStates = new WeakMap<object, FieldState>();
const collectionStates = new WeakMap<object, CollectionState>();
const indexStates = new WeakMap<object, IndexState>();
const names = new WeakSet<object>();

/** The field with this state; each modifier returns a new field. */
const field = (state: FieldState): Field => {
  const made: Field = {
    personal: ({ person }) => field({ ...state, person }),
    describe: (text) => field({ ...state, description: text }),
    deprecated: (reason) => field({ ...state, deprecated: reason }),
  };
  fieldStates.set(made, state);
  return made;
};

const basic = (type: string, options?: readonly string[]): Field =>
  field({
    spec:
      options === undefined
        ? { kind: "basic", type }
        : { kind: "basic", type, options: [...options] },
  });

const targets = (target: string | readonly string[]): string[] =>
  typeof target === "string" ? [target] : [...target];

/** Declares the model: the collections, by name. */
export function model<C extends Record<string, Collection>>(collections: C): C {
  const declared = { ...collections };
  Object.defineProperty(declared, DESCRIBE, {
    value: () => describeModel(declared),
    enumerable: false,
  });
  return declared;
}

/** Declares a collection: its fields by name, and its computed fields, views and indexes. */
export function collection(
  fields: Record<string, Field>,
  options: {
    computed?: Record<string, Named>;
    views?: Record<string, Named>;
    indexes?: Record<string, Index>;
  } = {},
): Collection {
  const made = {} as Collection;
  collectionStates.set(made, {
    fields: { ...fields },
    computed: { ...options.computed },
    views: { ...options.views },
    indexes: { ...options.indexes },
  });
  return made;
}

/** A string, basic value. */
export const string = (): Field => basic("ncdb/string");
/** A signed 64-bit integer, basic value. */
export const int = (): Field => basic("ncdb/int");
/** A binary64 number, basic value. */
export const float = (): Field => basic("ncdb/float");
/** A boolean, basic value. */
export const boolean = (): Field => basic("ncdb/boolean");
/** A choice from a fixed list of options, basic value. */
export const oneOf = (options: readonly string[]): Field =>
  basic("ncdb/choice", options);
/** A set of basic values. */
export const set = (element: Field): Field =>
  field({ spec: { kind: "set", of: element } });
/** A link to one object, in one collection or in one of several. */
export const link = (target: string | readonly string[]): Field =>
  field({
    spec: { kind: "link", type: "ncdb/link", targets: targets(target) },
  });
/** Links to many objects, in one collection or in several. */
export const links = (target: string | readonly string[]): Field =>
  field({
    spec: { kind: "link", type: "ncdb/links", targets: targets(target) },
  });
/** Rich text: reserved, its field type belongs to a later RFC. */
export const richText = (): Field =>
  field({ spec: { kind: "reserved", name: "nc.richText" } });
/** A stream: reserved, its field type belongs to a later RFC. */
export const stream = (): Field =>
  field({ spec: { kind: "reserved", name: "nc.stream" } });

const name = (): Named => {
  const made = {} as Named;
  names.add(made);
  return made;
};

/** A computed function: named now, its arguments belong to the computed-fields RFC. */
export const reach = (..._args: unknown[]): Named => name();
/** A view: named now, its arguments belong to the computed-fields RFC. */
export const view = (..._args: unknown[]): Named => name();

/** Declares an index: fields in index order, each ascending unless it says otherwise, how many leading fields a query fixes with equality (`equal`, 0 by default), and with `scope: "space"` an index that runs in one space at a time instead of across every space of the member. */
export const index = (
  fields: readonly (string | { field: string; direction: "asc" | "desc" })[],
  options: { equal?: number; scope?: "space" } = {},
): Index => {
  const made = {} as Index;
  indexStates.set(made, {
    fields: fields.map((entry) =>
      typeof entry === "string"
        ? { field: entry, direction: "asc" }
        : { field: entry.field, direction: entry.direction },
    ),
    equal: options.equal ?? 0,
    ...(options.scope === undefined ? {} : { scope: options.scope }),
  });
  return made;
};

/** The model as `ncdb model build` reads it (the core's `Draft`). */
const describeModel = (collections: Record<string, Collection>) => {
  const described: Record<string, unknown> = {};
  for (const [name, declared] of Object.entries(collections)) {
    const state = collectionStates.get(declared);
    if (state === undefined) {
      throw new Error(
        `${name}: not a collection; declare it with nc.collection(...).`,
      );
    }
    described[name] = describeCollection(name, state);
  }
  return { collections: described, plugins: {} };
};

const describeCollection = (name: string, state: CollectionState) => {
  const fields: Record<string, unknown> = {};
  for (const [fieldName, declared] of Object.entries(state.fields)) {
    fields[fieldName] = describeField(`${name}.${fieldName}`, declared);
  }
  const named = (place: Record<string, Named>, what: string) =>
    Object.entries(place).map(([key, value]) => {
      if (!names.has(value)) {
        throw new Error(`${name}.${key}: not a ${what}.`);
      }
      return key;
    });
  const indexes: Record<string, IndexState> = {};
  for (const [key, value] of Object.entries(state.indexes)) {
    const declared = indexStates.get(value);
    if (declared === undefined) {
      throw new Error(
        `${name}.${key}: not an index; declare it with nc.index(...).`,
      );
    }
    indexes[key] = declared;
  }
  return {
    fields,
    computed: named(
      state.computed,
      "computed field; declare it with nc.reach(...)",
    ),
    views: named(state.views, "view; declare it with nc.view(...)"),
    indexes,
  };
};

const describeField = (
  path: string,
  declared: Field,
): Record<string, unknown> => {
  const state = fieldStates.get(declared);
  if (state === undefined) {
    throw new Error(
      `${path}: not a field; declare it with a field function such as nc.string().`,
    );
  }
  const { spec } = state;
  const described: Record<string, unknown> = {
    ...describeType(path, spec),
    rule: 1,
  };
  if (state.person !== undefined) described.personal = { person: state.person };
  if (state.description !== undefined)
    described.description = state.description;
  if (state.deprecated !== undefined) described.deprecated = state.deprecated;
  return described;
};

const describeType = (path: string, spec: Spec): Record<string, unknown> => {
  switch (spec.kind) {
    case "basic":
      return spec.options === undefined
        ? { type: spec.type }
        : { type: spec.type, options: spec.options };
    case "link":
      return { type: spec.type, targets: spec.targets };
    case "reserved":
      throw new Error(
        `${path}: ${spec.name} is reserved; its field type belongs to a later RFC (RFC 0005).`,
      );
    case "set": {
      const element = fieldStates.get(spec.of);
      if (element === undefined || element.spec.kind !== "basic") {
        throw new Error(
          `${path}: a set's element is a basic value: nc.string(), nc.int(), nc.float(), nc.boolean() or nc.oneOf([...]).`,
        );
      }
      if (
        element.person !== undefined ||
        element.description !== undefined ||
        element.deprecated !== undefined
      ) {
        throw new Error(
          `${path}: a set's element takes no modifiers; put them on the set.`,
        );
      }
      return { type: "ncdb/set", of: describeType(path, element.spec) };
    }
  }
};
