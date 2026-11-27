// `space.richText(collection, id, field)` (RFC 0013, "An editor: the
// binding"; NOC-461): it opens a binding on the field through the bridge,
// with the field's vocabulary from the lock (NOC-553), and pulls what the
// core folded after each fold of the object. A core without rich text
// answers `notSupported` (Carsten, 2026-11-27, NOC-538, Q5).
import { NcError } from "../error.ts";
import * as texts from "../texts.ts";
import { type Binding, bind, type Vocabulary } from "./binding.ts";
import { openRichText } from "./port.ts";

type Ask = (call: string, args: Record<string, unknown>) => Promise<unknown>;
type Change = {
  readonly space: string;
  readonly collection: string;
  readonly ids: readonly string[];
};

/** The vocabulary of `field` of `collection` in `lock`: empty when it declares none (NOC-558). */
export const vocabularyOf = (
  lock: unknown,
  collection: string,
  field: string,
): Vocabulary => {
  const declared = (
    lock as {
      collections?: Record<
        string,
        { fields?: Record<string, { vocabulary?: Partial<Vocabulary> }> }
      >;
    }
  )?.collections?.[collection]?.fields?.[field]?.vocabulary;
  return { nodes: declared?.nodes ?? [], marks: declared?.marks ?? [] };
};

/** The space's `richText`. */
export const richTextOf =
  (
    ask: Ask,
    space: string,
    nameOf: (collection: object) => string,
    lock: unknown,
    supported: () => boolean,
    onChange: (listener: (change: Change) => void) => () => void,
  ) =>
  async (collection: object, id: string, field: string): Promise<Binding> => {
    if (!supported()) {
      throw new NcError("notSupported", texts.RICH_TEXT_NOT_SUPPORTED);
    }
    const name = nameOf(collection);
    const { opened, port } = await openRichText(ask, {
      space,
      collection: name,
      id,
      field,
    });
    let stop = () => {};
    const body = bind(
      opened,
      {
        ...port,
        close: async () => {
          stop();
          await port.close();
        },
      },
      () => {},
      { vocabulary: vocabularyOf(lock, name, field) },
    );
    stop = onChange((change) => {
      if (
        change.space === space &&
        change.collection === name &&
        change.ids.includes(id)
      ) {
        body.folded().catch(() => {});
      }
    });
    return body;
  };
