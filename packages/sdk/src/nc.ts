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

import type { Query } from "./db.ts";
import { fromMarkdown } from "./firstDocument.ts";
import { makeQuery } from "./query.ts";
import * as texts from "./texts.ts";

/** What every field of the model carries: what it reads as (`T`), and whether it is deprecated (`D`). */
interface Value<T = unknown, D extends boolean = false> {
  readonly "nc.reads": T;
  readonly "nc.deprecated": D;
}

/** A field of the model that reads as `T`, deprecated when `D` is true: its type, and the modifiers chained after it. */
interface Field<T = unknown, D extends boolean = false> extends Value<T, D> {
  /** Marks the field as personal, about `person`: `"self"` or a link field of the same collection. */
  personal(options: { person: string }): Field<T, D>;
  /** Adds a description, which is copied into the lock. */
  describe(text: string): Field<T, D>;
  /** Hides the field from the app, and keeps it in the model and the data. */
  deprecated(reason: string): Field<T, true>;
}

/** A reference field, deprecated when `D` is true: it takes `describe` and `deprecated`, and cannot be personal (RFC 0005, "References into another space"). */
interface Ref<D extends boolean = false> extends Value<Target, D> {
  /** Adds a description, which is copied into the lock. */
  describe(text: string): Ref<D>;
  /** Hides the field from the app, and keeps it in the model and the data. */
  deprecated(reason: string): Ref<true>;
}

/** A rich-text field, deprecated when `D` is true: it reads as its Markdown, takes `describe` and `deprecated`, and cannot be personal (RFC 0013, "The field in the schema and the lock"). */
interface RichText<D extends boolean = false> extends Value<string, D> {
  readonly "nc.richText": true;
  /** Adds a description, which is copied into the lock. */
  describe(text: string): RichText<D>;
  /** Hides the field from the app, and keeps it in the model and the data. */
  deprecated(reason: string): RichText<true>;
}

/** The fields of a collection, by name. */
type Fields = Record<string, Value<unknown, boolean>>;

/** A collection of the model with the fields `F` and the indexes `I`, deprecated when `D` is true: its fields, and the places for computed fields, views and indexes. */
interface Collection<
  F extends Fields = Fields,
  D extends boolean = false,
  I extends Indexes = Indexes,
> {
  readonly "nc.collection": F;
  readonly "nc.deprecated": D;
  /** A query of every declared index, by name (RFC 0009, "Queries and indexes"). */
  readonly indexes: {
    readonly [K in keyof I]: Query<Item<F>, EqualOf<F, I[K]>>;
  };
  /** Hides the collection from the app, and keeps it in the model and the data. */
  deprecated(reason: string): Collection<F, true, I>;
}

/** The declared indexes of a collection, by name. */
type Indexes = Record<string, Index>;

/** The fields of `F` that are not deprecated, each optional, as the app reads them. */
type Readable<F extends Fields> = {
  [K in keyof F as F[K] extends Value<unknown, true>
    ? never
    : K]?: F[K]["nc.reads"];
};

/** An object of a collection with the fields `F`, as a read gives it: its fields and its ids (RFC 0009, "Get by id"). */
type Item<F extends Fields> = Readable<F> & {
  readonly $id: ObjectId;
  readonly $space: SpaceId;
  readonly $deleted: boolean;
};

/** The values `.where` and `.equal` take for the first `N` fields `K` of a collection with the fields `F`: one per field, of that field's type, or `null` for an unset field (RFC 0006, "Declared indexes and ordered pages"). */
type EqualValues<
  F extends Fields,
  K extends readonly string[],
  N extends number,
  Taken extends readonly unknown[] = [],
> = Taken["length"] extends N
  ? Taken
  : K extends readonly [
        infer H extends string,
        ...infer R extends readonly string[],
      ]
    ? EqualValues<
        F,
        R,
        N,
        [...Taken, Exclude<F[H & keyof F]["nc.reads"], undefined> | null]
      >
    : Taken;

/** The values `.equal` takes for the index `X` of a collection with the fields `F`. */
type EqualOf<F extends Fields, X> =
  X extends Index<infer K, infer N> ? EqualValues<F, K, N> : readonly [];

/** The collections of a model without the deprecated ones, which the app does not see. */
type Visible<C> = {
  [K in keyof C as C[K] extends Collection<Fields, true> ? never : K]: C[K];
};

/** A choice option the lock does not list yet, kept as written (RFC 0009, "Get by id"). */
export type Unknown = { unknown: string };

/**
 * An object of the collection `C` as the app reads it (RFC 0005, "Types for
 * the app"): every field optional, without the deprecated ones. A choice
 * reads as its options or `Unknown`, an `nc.int` as a `number`, an
 * `nc.date` as a `"YYYY-MM-DD"` string, an `nc.timestamp` as a `Date`, a set as
 * an array, a link as an object id, a reference as `{ space, id }`
 * (RFC 0009, "Get by id").
 */
export type Infer<C extends Collection<Fields, boolean>> =
  C extends Collection<infer F, false>
    ? {
        [K in keyof F as F[K] extends Value<unknown, true>
          ? never
          : K]?: F[K]["nc.reads"];
      }
    : never;

/** A computed field or a view: named now, designed by a later RFC. */
interface Named {
  readonly "nc.named": true;
}

/** A declared index on the fields `K`, whose first `N` a query fixes with equality. */
interface Index<
  K extends readonly string[] = readonly string[],
  N extends number = number,
> {
  readonly "nc.index": true;
  readonly "nc.fields": K;
  readonly "nc.equal": N;
}

/** The field names of an index's entries, in order. */
type Names<E extends readonly (string | { field: string })[]> = {
  [P in keyof E]: E[P] extends string
    ? E[P]
    : E[P] extends { field: infer N extends string }
      ? N
      : never;
};

/** The key under which `model` keeps the function that describes the model for `ncdb model build`. */
const DESCRIBE = Symbol.for("nocentraldb.model.describe");

/** A field's type, as the lock names it. */
type Spec =
  | { kind: "basic"; type: string; options?: string[] }
  | { kind: "set"; of: Field<unknown, boolean> }
  | {
      kind: "link";
      type: "ncdb/link" | "ncdb/links" | "ncdb/ref";
      targets: string[];
    }
  | { kind: "richText"; vocabulary?: Vocabulary }
  | { kind: "reserved"; name: string };

/** A rich-text field's vocabulary: the names of the nodes and the marks its editor uses (RFC 0013, "The field in the schema and the lock"). */
type Vocabulary = { nodes: string[]; marks: string[] };

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
  scope?: "space";
  /** For the named form: what the build refuses in it, if anything. */
  named?: NamedRefusal | undefined;
};

/** A named index the build refuses: one without a field, one that names a field twice, or one with a field in both `where` and `orderBy`. */
type NamedRefusal =
  | { kind: "noField" }
  | { kind: "twice"; field: string }
  | { kind: "both"; field: string };

/** An object id, as a link reads: its 16 bytes as 32 lowercase hex digits (RFC 0009, "Get by id"). */
type ObjectId = string;

/** A space id, in the form `$space` carries it. */
type SpaceId = string;

/** A reference as the app reads it: the target's space id and object id, without its state (RFC 0009, "Get by id"). */
type Target = { space: SpaceId; id: ObjectId };

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

/** A field without `personal`, with this state: a reference (RFC 0005, "References into another space") or rich text (RFC 0013). */
const unpersonal = <F>(state: FieldState): F => {
  const made = {
    describe: (text: string) => unpersonal({ ...state, description: text }),
    deprecated: (reason: string) =>
      unpersonal({ ...state, deprecated: reason }),
  } as unknown as F;
  fieldStates.set(made as object, state);
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
export function collection<
  F extends Fields,
  const I extends Indexes = Record<never, never>,
>(
  fields: F,
  options: {
    computed?: Record<string, Named>;
    views?: Record<string, Named>;
    indexes?: I;
  } = {},
): Collection<F, false, I> {
  return makeCollection({
    fields: { ...fields },
    computed: { ...options.computed },
    views: { ...options.views },
    indexes: { ...options.indexes },
  });
}

/** The collection with this state; `deprecated` returns a new collection. */
const makeCollection = <F extends Fields, D extends boolean, I extends Indexes>(
  state: CollectionState,
): Collection<F, D, I> => {
  const made: Record<string, unknown> = {
    deprecated: (reason: string) =>
      makeCollection({ ...state, deprecated: reason }),
  };
  made.indexes = Object.fromEntries(
    Object.keys(state.indexes).map((name) => [name, makeQuery(made, name)]),
  );
  collectionStates.set(made, state);
  return made as unknown as Collection<F, D, I>;
};

/** A string, basic value. */
export const string = (): Field<string> => basic("ncdb/string");
/** A signed 64-bit integer, basic value. The app reads and writes it as a `number`, a safe integer (RFC 0005, "Types for the app"). */
export const int = (): Field<number> => basic("ncdb/int");
/** A binary64 number, basic value. */
export const float = (): Field<number> => basic("ncdb/float");
/** A boolean, basic value. */
export const boolean = (): Field<boolean> => basic("ncdb/boolean");
/** A choice from a fixed list of options, basic value. */
export const oneOf = <const O extends readonly string[]>(
  options: O,
): Field<O[number] | Unknown> => basic("ncdb/choice", options);
/** A calendar day, basic value, read and written as a `"YYYY-MM-DD"` string, never converted to a time zone (RFC 0005, "Days and points in time"). */
export const date = (): Field<string> => basic("ncdb/date");
/** A point in time to the millisecond, in UTC, basic value, read and written as a `Date` (RFC 0005, "Days and points in time"). */
export const timestamp = (): Field<Date> => basic("ncdb/timestamp");
/** A set of basic values, other than days and points in time. */
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
/** A reference to one object, which may live in another space, in one collection or in one of several (RFC 0005, "References into another space"). */
export const ref = (target: string | readonly string[]): Ref =>
  unpersonal({
    spec: { kind: "link", type: "ncdb/ref", targets: targets(target) },
  });
/** Rich text: a shared document, which reads as its Markdown. `vocabulary` names the nodes and marks its editor uses; a later model may add names, never remove one (RFC 0013, "The field in the schema and the lock"). */
export const richText = Object.assign(
  (options?: {
    vocabulary?: { nodes?: readonly string[]; marks?: readonly string[] };
  }): RichText => {
    const vocabulary = options?.vocabulary;
    return unpersonal({
      spec:
        vocabulary === undefined
          ? { kind: "richText" }
          : {
              kind: "richText",
              vocabulary: {
                nodes: [...(vocabulary.nodes ?? [])],
                marks: [...(vocabulary.marks ?? [])],
              },
            },
    });
  },
  {
    /**
     * A first document from `markdown`, which a create or an update writes into a rich-text field: CommonMark with GitHub's tables, strikethrough and task lists, read by the core into TipTap's standard vocabulary, as the Rust SDK's `RichText::from_markdown` is (RFC 0013, "The first content"). It writes the whole document, and only into a field that holds no content in the copy, on `create` or on a field nothing was written to or whose every block was deleted; on a field with content the write is refused with `richTextNotEmpty`, as replacing a document others may be editing would overwrite their edits. Markdown the core does not write a document from, such as HTML or an image, is refused with `invalidValue`. An app that imports a document writes it once this way and edits it from then on.
     */
    fromMarkdown,
  },
);
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

/** An index in the named form (RFC 0018, "Named equality and order in an index"): the fields a query fixes with equality, `where`, and the fields that order the result, `orderBy`, each ascending unless its name starts with `-`. */
type NamedIndex<
  W extends readonly string[] = readonly string[],
  O extends readonly string[] = readonly string[],
> = { where?: W; orderBy?: O; scope?: "space" };

/** The field names of `orderBy`, without the `-` of a descending one. */
type Unsigned<O extends readonly string[]> = {
  [P in keyof O]: O[P] extends `-${infer N}` ? N : O[P];
};

/**
 * Declares an index in the named form: `where` lists the fields a query
 * fixes with `.where(...)`, in index order, and `orderBy` the fields that
 * order the result, each ascending unless its name starts with `-`; with
 * `scope: "space"` it runs in one space at a time instead of across every
 * space of the member. It builds the same lock entry as today's form with
 * the `where` fields first and `equal` their count (RFC 0018, "Named equality and order in an index").
 */
export function index<
  const W extends readonly string[] = readonly [],
  const O extends readonly string[] = readonly [],
>(spec: NamedIndex<W, O>): Index<[...W, ...Unsigned<O>], W["length"]>;
/** Declares an index: fields in index order, each ascending unless it says otherwise, how many leading fields a query fixes with equality (`equal`, 0 by default), and with `scope: "space"` an index that runs in one space at a time instead of across every space of the member. */
export function index<
  const K extends readonly (
    | string
    | { field: string; direction: "asc" | "desc" }
  )[],
  const N extends number = 0,
>(fields: K, options?: { equal?: N; scope?: "space" }): Index<Names<K>, N>;
export function index(
  given: readonly (string | IndexField)[] | NamedIndex,
  options: { equal?: number; scope?: "space" } = {},
): Index {
  const made = {} as Index;
  indexStates.set(
    made,
    isNamed(given) ? namedState(given) : todayState(given, options),
  );
  return made;
}

type IndexField = { field: string; direction: "asc" | "desc" };

const isNamed = (
  given: readonly (string | IndexField)[] | NamedIndex,
): given is NamedIndex => !Array.isArray(given);

const scoped = (scope: "space" | undefined) =>
  scope === undefined ? {} : { scope };

const todayState = (
  fields: readonly (string | IndexField)[],
  options: { equal?: number; scope?: "space" },
): IndexState => ({
  fields: fields.map((entry) =>
    typeof entry === "string"
      ? { field: entry, direction: "asc" }
      : { field: entry.field, direction: entry.direction },
  ),
  equal: options.equal ?? 0,
  ...scoped(options.scope),
});

const namedState = (spec: NamedIndex): IndexState => {
  const where = spec.where ?? [];
  const orderBy = (spec.orderBy ?? []).map(
    (name): IndexField =>
      name.startsWith("-")
        ? { field: name.slice(1), direction: "desc" }
        : { field: name, direction: "asc" },
  );
  return {
    fields: [
      ...where.map((field): IndexField => ({ field, direction: "asc" })),
      ...orderBy,
    ],
    equal: where.length,
    ...scoped(spec.scope),
    named: refusal(where, orderBy),
  };
};

/** What the build refuses in a named index, when anything. */
const refusal = (
  where: readonly string[],
  orderBy: readonly IndexField[],
): NamedRefusal | undefined => {
  if (where.length + orderBy.length === 0) return { kind: "noField" };
  const ordered = orderBy.map((entry) => entry.field);
  for (const list of [where, ordered]) {
    const twice = list.find((name, i) => list.indexOf(name) !== i);
    if (twice !== undefined) return { kind: "twice", field: twice };
  }
  const both = where.find((name) => ordered.includes(name));
  return both === undefined ? undefined : { kind: "both", field: both };
};

/** The model as `ncdb model build` reads it (the core's `Draft`). */
const describeModel = (
  collections: Record<string, Collection<Fields, boolean>>,
) => {
  const described: Record<string, unknown> = {};
  for (const [name, declared] of Object.entries(collections)) {
    const state = collectionStates.get(declared);
    if (state === undefined) {
      throw new Error(texts.notACollection(name));
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
  const named = (
    place: Record<string, Named>,
    notOne: (collection: string, key: string) => string,
  ) =>
    Object.entries(place).map(([key, value]) => {
      if (!names.has(value)) {
        throw new Error(notOne(name, key));
      }
      return key;
    });
  const indexes: Record<string, IndexState> = {};
  for (const [key, value] of Object.entries(state.indexes)) {
    const declared = indexStates.get(value);
    if (declared === undefined) {
      throw new Error(texts.notAnIndex(name, key));
    }
    const { named, ...entry } = declared;
    if (named !== undefined) {
      throw new Error(refused(name, key, named));
    }
    indexes[key] = entry;
  }
  return {
    ...(state.deprecated === undefined ? {} : { deprecated: state.deprecated }),
    fields,
    computed: named(state.computed, (collection, key) =>
      texts.notAComputedField(collection, key),
    ),
    views: named(state.views, (collection, key) =>
      texts.notAView(collection, key),
    ),
    indexes,
  };
};

const refused = (
  collection: string,
  key: string,
  named: NamedRefusal,
): string => {
  switch (named.kind) {
    case "noField":
      return texts.indexNamesNoField(collection, key);
    case "twice":
      return texts.indexNamesFieldTwice(collection, key, named.field);
    case "both":
      return texts.indexFieldInWhereAndOrderBy(collection, key, named.field);
  }
};

const describeField = (
  path: string,
  declared: Value<unknown, boolean>,
): Record<string, unknown> => {
  const state = fieldStates.get(declared);
  if (state === undefined) {
    throw new Error(texts.notAField(path));
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
    case "richText":
      return spec.vocabulary === undefined
        ? { type: "ncdb/richText" }
        : { type: "ncdb/richText", vocabulary: spec.vocabulary };
    case "reserved":
      throw new Error(texts.reservedFieldType(path, spec.name));
    case "set": {
      const element = fieldStates.get(spec.of);
      if (
        element === undefined ||
        element.spec.kind !== "basic" ||
        element.spec.type === "ncdb/date" ||
        element.spec.type === "ncdb/timestamp"
      ) {
        throw new Error(texts.setElementNotBasic(path));
      }
      if (
        element.person !== undefined ||
        element.description !== undefined ||
        element.deprecated !== undefined
      ) {
        throw new Error(texts.setElementModified(path));
      }
      return { type: "ncdb/set", of: describeType(path, element.spec) };
    }
  }
};

export { pairingAnswer } from "./account.ts";
/** Opens this member's database (RFC 0009, "Open and close"). */
/** Field operations on sets and many links (RFC 0009, "Field operations per field type"). */
export { add, open, remove } from "./db.ts";
export { erasureRequest, signOffs } from "./erase.ts";
/** The one error class the SDK throws (RFC 0009, "Errors"). */
export { NcError as Error } from "./error.ts";
export { invitation } from "./members.ts";
