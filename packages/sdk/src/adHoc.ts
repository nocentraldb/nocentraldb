// Ad-hoc spaces for sub-agents over the bridge (NOC-318 S4; RFC 0009,
// "Sub-agents and ad-hoc spaces"; crates/nocentraldb/src/bridge/ad_hoc.rs).
// The owner decided the shapes on NOC-351 (Carsten, 2026-11-16): `copy`
// takes the objects the SDK read, the call answers each source with its
// copy and the counts, and `space.sourceOf` and `space.copyOf` find them
// later. Internal: the package's entry does not reach it.

import { NcError } from "./error.ts";
import type { Notice } from "./members.ts";
import * as texts from "./texts.ts";

type Id = string;
type Ask = (call: string, args: Record<string, unknown>) => Promise<unknown>;

/** Where an object the SDK read came from: what the core reads it by. */
interface Source {
  readonly space: Id;
  readonly collection: string;
  readonly id: Id;
}

/**
 * The objects one database's SDK read, each with its collection, which a
 * read object does not carry. Weakly held, so that a forgotten object is
 * not kept; a spread of one, or an object another database read, is not
 * among them.
 */
export const readObjects = () => {
  const read = new WeakMap<object, Source>();
  return {
    /** Remembers `object`, read from `collection`, and gives it back. */
    remember<T extends object>(object: T, collection: string): T {
      const { $space, $id } = object as { $space?: Id; $id?: Id };
      if ($space !== undefined && $id !== undefined) {
        read.set(object, { space: $space, collection, id: $id });
      }
      return object;
    },
    /** Where `object` came from; `invalidValue` when the SDK did not read it. */
    sourceOf(object: object): Source {
      const source = read.get(object);
      if (source === undefined) {
        throw new NcError("invalidValue", texts.AD_HOC_COPY_NOT_READ);
      }
      return source;
    },
  };
};

/** The options of `db.spaces.createAdHoc`. */
export interface AdHocOptions {
  readonly copy: readonly object[];
  readonly personal?: boolean;
  readonly local?: boolean;
}

/** What the core's `spaces.createAdHoc` answers, the space still as its id. */
export interface AdHocAnswer {
  readonly space: Id;
  readonly notice: Notice;
  readonly copies: readonly { readonly source: Id; readonly copy: Id }[];
  readonly clearedLinks: number;
  readonly pluginFieldsNotCopied: number;
}

/** Asks the core for an ad-hoc space; refuses before it any object the SDK did not read. */
export const createAdHoc = async (
  ask: Ask,
  read: ReturnType<typeof readObjects>,
  options: AdHocOptions,
): Promise<AdHocAnswer> => {
  const args: Record<string, unknown> = {
    copy: options.copy.map((object) => read.sourceOf(object)),
  };
  if (options.personal !== undefined) args.personal = options.personal;
  if (options.local !== undefined) args.local = options.local;
  return (await ask("spaces.createAdHoc", args)) as AdHocAnswer;
};

/** The lookups of the ad-hoc space `space`. */
export const lookupsOf = (ask: Ask, space: Id) => ({
  sourceOf: async (copy: Id) =>
    ((await ask("space.sourceOf", { space, copy })) ?? undefined) as
      | { readonly space: Id; readonly id: Id }
      | undefined,
  copyOf: async (sourceSpace: Id, source: Id) =>
    ((await ask("space.copyOf", { space, sourceSpace, source })) ??
      undefined) as Id | undefined,
});
