// Field values between the app and the bridge, by the type the TypeScript
// model declares for each field (RFC 0009, "Field operations per field
// type"): what a write sends, and how a read object comes back. A field
// written `nc.ref` sends `{ space, id }` whichever form the app wrote, and
// reads as `{ space, id }` even where the lock keeps it as `ncdb/link`,
// whose value the core reads as an object id (RFC 0018, "One function, two
// cases" and "Existing link fields read as references"). A first document
// from Markdown sends `{ $markdown }` (RFC 0013, "The first content").
// Internal, split from connect.ts.

import { fromCore, toCore } from "./dates.ts";
import { SetElements } from "./elements.ts";
import { NcError } from "./error.ts";
import { Markdown } from "./markdown.ts";
import * as texts from "./texts.ts";

/** The model's collections as `nc.model` describes them: each field's type, and a set's element type. */
export type Described = Record<
  string,
  { fields: Record<string, { type: string; of?: { type: string } }> }
>;

const REF = "ncdb/ref";

/** `written`, in a reference field of an object in `space`, as `{ space, id }`: a bare object id names `space`, an object the SDK read its `$space` and `$id`. Anything else passes as it is, for the core to refuse. */
const reference = (written: unknown, space: string): unknown => {
  if (typeof written === "string") return { space, id: written };
  if (typeof written === "object" && written !== null) {
    const { $space, $id } = written as { $space?: unknown; $id?: unknown };
    if (typeof $space === "string" && typeof $id === "string") {
      return { space: $space, id: $id };
    }
  }
  return written;
};

/** The conversions of field values for the model `described`. */
export const valuesOf = (described: Described) => {
  const value = (
    collection: string,
    field: string,
    written: unknown,
    space: string,
  ): unknown => {
    if (written === null) return null;
    if (described[collection]?.fields[field]?.type === REF) {
      return reference(written, space);
    }
    // Sent as written, without the NFC of a string, as the Rust SDK sends
    // it, so the same Markdown gives the same document (NOC-578).
    if (written instanceof Markdown) return { $markdown: written.markdown };
    if (written instanceof SetElements) {
      const elements = written.elements.map((e) =>
        element(collection, field, e, true),
      );
      return written.add ? { $add: elements } : { $remove: elements };
    }
    return element(collection, field, written, false);
  };

  const element = (
    collection: string,
    field: string,
    written: unknown,
    inSet: boolean,
  ): unknown => {
    const type = described[collection]?.fields[field];
    const kind = inSet ? (type?.of?.type ?? type?.type) : type?.type;
    if (
      kind === "ncdb/int" &&
      !(typeof written === "number" && Number.isSafeInteger(written))
    ) {
      throw new NcError(
        "invalidValue",
        texts.notASafeInteger(collection, field),
      );
    }
    const sent = toCore(kind, collection, field, written);
    return typeof sent === "string" ? sent.normalize("NFC") : sent;
  };

  /** `object`, read from `collection`, with each point in time as a Date, and each reference the core read as an object id as `{ space, id }` in the object's own space. */
  const dated = (collection: string, object: Record<string, unknown>) => {
    const fields = described[collection]?.fields ?? {};
    for (const [name, field] of Object.entries(fields)) {
      const id = object[name];
      if (field.type === REF && typeof id === "string") {
        object[name] = { space: object.$space, id };
      }
    }
    return fromCore(fields, object);
  };

  /** The values a create or an update of `collection` in `space` sends, by field name; `undefined` sends nothing. */
  const values = (
    collection: string,
    written: Record<string, unknown>,
    space: string,
  ) => {
    const out: Record<string, unknown> = {};
    for (const [field, v] of Object.entries(written)) {
      if (v !== undefined) out[field] = value(collection, field, v, space);
    }
    return out;
  };

  return { values, dated };
};
