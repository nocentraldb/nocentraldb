// Field values between the app and the bridge, by the type the TypeScript
// model declares for each field (RFC 0009, "Field operations per field
// type"): what a write sends, and how a read object comes back. Internal,
// split from connect.ts.

import { fromCore, toCore } from "./dates.ts";
import { SetElements } from "./elements.ts";
import { NcError } from "./error.ts";
import * as texts from "./texts.ts";

/** The model's collections as `nc.model` describes them: each field's type, and a set's element type. */
export type Described = Record<
  string,
  { fields: Record<string, { type: string; of?: { type: string } }> }
>;

/** The conversions of field values for the model `described`. */
export const valuesOf = (described: Described) => {
  const value = (
    collection: string,
    field: string,
    written: unknown,
  ): unknown => {
    if (written === null) return null;
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

  /** `object`, read from `collection`, with each point in time as a Date. */
  const dated = (collection: string, object: Record<string, unknown>) =>
    fromCore(described[collection]?.fields, object);

  /** The values a create or an update of `collection` sends, by field name; `undefined` sends nothing. */
  const values = (collection: string, written: Record<string, unknown>) => {
    const out: Record<string, unknown> = {};
    for (const [field, v] of Object.entries(written)) {
      if (v !== undefined) out[field] = value(collection, field, v);
    }
    return out;
  };

  return { values, dated };
};
