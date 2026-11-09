// The schema functions of RFC 0005 (docs/rfc/0005-schema-and-model-lock.md).

/** A field of the model. */
interface Field {
  personal(options: { person: string }): Field;
  describe(text: string): Field;
  deprecated(reason: string): Field;
}

/** A collection of the model. */
type Collection = {};

/** A computed field or a view, by name only. */
type Named = {};

/** A declared index. */
type Index = {};

const todo = (): never => {
  throw new Error("not implemented");
};

export function model<C extends Record<string, Collection>>(collections: C): C {
  return collections;
}
export function collection(
  _fields: Record<string, Field>,
  _options?: {
    computed?: Record<string, Named>;
    views?: Record<string, Named>;
    indexes?: Record<string, Index>;
  },
): Collection {
  return todo();
}
export const string = (): Field => todo();
export const int = (): Field => todo();
export const float = (): Field => todo();
export const boolean = (): Field => todo();
export const oneOf = (_options: readonly string[]): Field => todo();
export const set = (_element: Field): Field => todo();
export const link = (_target: string | readonly string[]): Field => todo();
export const links = (_target: string | readonly string[]): Field => todo();
export const richText = (): Field => todo();
export const stream = (): Field => todo();
export const reach = (..._args: unknown[]): Named => todo();
export const view = (..._args: unknown[]): Named => todo();
export const index = (
  _fields: readonly (string | { field: string; direction: "asc" | "desc" })[],
  _options?: { equal?: number },
): Index => todo();
