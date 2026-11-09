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
// passed for one another. A field's brands also carry what it reads as and
// whether it is deprecated, and a collection's carry its fields, which is
// what `Infer` reads.

/** A field of the model that reads as `T`, deprecated when `D` is true: its type, and the modifiers chained after it. */
interface Field<T = unknown, D extends boolean = false> {
  readonly "nc.reads": T;
  readonly "nc.deprecated": D;
  /** Marks the field as personal, about `person`: `"self"` or a link field of the same collection. */
  personal(options: { person: string }): Field<T, D>;
  /** Adds a description, which is copied into the lock. */
  describe(text: string): Field<T, D>;
  /** Hides the field from the app, and keeps it in the model and the data. */
  deprecated(reason: string): Field<T, true>;
}

/** The fields of a collection, by name. */
type Fields = Record<string, Field<unknown, boolean>>;

/** A collection of the model with the fields `F`, deprecated when `D` is true: its fields, and the places for computed fields, views and indexes. */
interface Collection<F extends Fields = Fields, D extends boolean = false> {
  readonly "nc.collection": F;
  readonly "nc.deprecated": D;
  /** Hides the collection from the app, and keeps it in the model and the data. */
  deprecated(reason: string): Collection<F, true>;
}

/** The collections of a model without the deprecated ones, which the app does not see. */
type Visible<C> = {
  [K in keyof C as C[K] extends Collection<Fields, true> ? never : K]: C[K];
};

/** A choice option the lock does not list yet, kept as written (RFC 0009, S5). */
export type Unknown = { unknown: string };

/**
 * An object of the collection `C` as the app reads it (RFC 0005, "Types for
 * the app"): every field optional, without the deprecated ones. A choice
 * reads as its options or `Unknown`, an `nc.int` as a `number` (D16), a set
 * as an array, a link as an object id.
 */
export type Infer<C extends Collection<Fields, boolean>> =
  C extends Collection<infer F, false>
    ? {
        [K in keyof F as F[K] extends Field<unknown, true>
          ? never
          : K]?: F[K]["nc.reads"];
      }
    : never;

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
  | { kind: "set"; of: Field<unknown, boolean> }
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
  deprecated?: string;
  fields: Fields;
  computed: Record<string, Named>;
  views: Record<string, Named>;
  indexes: Record<string, Index>;
};

type IndexState = {
  fields: { field: string; direction: "asc" | "desc" }[];
  equal: number;
};

/** An object id, as a link reads. */
type ObjectId = string;

const fieldStates = new WeakMap<object, FieldState>();
const collectionStates = new WeakMap<object, CollectionState>();
const indexStates = new WeakMap<object, IndexState>();
const names = new WeakSet<object>();

/** The field with this state; each modifier returns a new field. */
const field = <T>(state: FieldState): Field<T> => {
  const made = {
    personal: ({ person }: { person: string }) => field({ ...state, person }),
    describe: (text: string) => field({ ...state, description: text }),
    deprecated: (reason: string) => field({ ...state, deprecated: reason }),
  } as unknown as Field<T>;
  fieldStates.set(made, state);
  return made;
};

const basic = <T>(type: string, options?: readonly string[]): Field<T> =>
  field({
    spec:
      options === undefined
        ? { kind: "basic", type }
        : { kind: "basic", type, options: [...options] },
  });

const targets = (target: string | readonly string[]): string[] =>
  typeof target === "string" ? [target] : [...target];

/** Declares the model: the collections, by name. */
export function model<C extends Record<string, Collection<Fields, boolean>>>(
  collections: C,
): Visible<C> {
  const declared = { ...collections };
  Object.defineProperty(declared, DESCRIBE, {
    value: () => describeModel(declared),
    enumerable: false,
  });
  return declared as Visible<C>;
}

/** Declares a collection: its fields by name, and its computed fields, views and indexes. */
export function collection<F extends Fields>(
  fields: F,
  options: {
    computed?: Record<string, Named>;
    views?: Record<string, Named>;
    indexes?: Record<string, Index>;
  } = {},
): Collection<F> {
  return makeCollection({
    fields: { ...fields },
    computed: { ...options.computed },
    views: { ...options.views },
    indexes: { ...options.indexes },
  });
}

/** The collection with this state; `deprecated` returns a new collection. */
const makeCollection = <F extends Fields, D extends boolean>(
  state: CollectionState,
): Collection<F, D> => {
  const made = {
    deprecated: (reason: string) =>
      makeCollection({ ...state, deprecated: reason }),
  } as unknown as Collection<F, D>;
  collectionStates.set(made, state);
  return made;
};

/** A string, basic value. */
export const string = (): Field<string> => basic("ncdb/string");
/** A signed 64-bit integer, basic value. The app reads and writes it as a `number`, a safe integer (D16). */
export const int = (): Field<number> => basic("ncdb/int");
/** A binary64 number, basic value. */
export const float = (): Field<number> => basic("ncdb/float");
/** A boolean, basic value. */
export const boolean = (): Field<boolean> => basic("ncdb/boolean");
/** A choice from a fixed list of options, basic value. */
export const oneOf = <const O extends readonly string[]>(
  options: O,
): Field<O[number] | Unknown> => basic("ncdb/choice", options);
/** A set of basic values. */
export const set = <T>(element: Field<T>): Field<T[]> =>
  field({ spec: { kind: "set", of: element } });
/** A link to one object, in one collection or in one of several. */
export const link = (target: string | readonly string[]): Field<ObjectId> =>
  field({
    spec: { kind: "link", type: "ncdb/link", targets: targets(target) },
  });
/** Links to many objects, in one collection or in several. */
export const links = (target: string | readonly string[]): Field<ObjectId[]> =>
  field({
    spec: { kind: "link", type: "ncdb/links", targets: targets(target) },
  });
/** Rich text: reserved, its field type belongs to a later RFC. */
export const richText = (): Field<never> =>
  field({ spec: { kind: "reserved", name: "nc.richText" } });
/** A stream: reserved, its field type belongs to a later RFC. */
export const stream = (): Field<never> =>
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

/** Declares an index: fields in index order, each ascending unless it says otherwise, and how many leading fields a query fixes with equality (`equal`, 0 by default). */
export const index = (
  fields: readonly (string | { field: string; direction: "asc" | "desc" })[],
  options: { equal?: number } = {},
): Index => {
  const made = {} as Index;
  indexStates.set(made, {
    fields: fields.map((entry) =>
      typeof entry === "string"
        ? { field: entry, direction: "asc" }
        : { field: entry.field, direction: entry.direction },
    ),
    equal: options.equal ?? 0,
  });
  return made;
};

/** The model as `ncdb model build` reads it (the core's `Draft`). */
const describeModel = (
  collections: Record<string, Collection<Fields, boolean>>,
) => {
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
    ...(state.deprecated === undefined ? {} : { deprecated: state.deprecated }),
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
  declared: Field<unknown, boolean>,
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
