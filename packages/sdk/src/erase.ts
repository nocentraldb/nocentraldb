// Erasing a person in the TypeScript SDK (NOC-406; RFC 0009, "The Rust
// SDK", "Erasing a person"; Carsten, 2026-11-15, NOC-326): the Rust SDK's
// `Db::erasure_request`, `Account::sign_off`, `Db::erase` and
// `Erasures::take_back` in camel case, over the core's `erasureRequest`,
// `account.signOff`, `erase` and `erasures.takeBack`
// (crates/nocentraldb/src/bridge/erase.rs). The request and the
// sign-offs are their texts, which the app carries between devices, as a
// pairing answer is (NOC-321): `nc.erasureRequest` and `nc.signOffs` read
// them back.

import type { readObjects } from "./adHoc.ts";
import { NcError } from "./error.ts";
import type { Notice } from "./members.ts";
import * as texts from "./texts.ts";

type Id = string;
type Ask = (call: string, args: Record<string, unknown>) => Promise<unknown>;

/** The request to erase a person (RFC 0009, "Erasing a person"): for each space where this member may write and the person has a key, the space, the person-key id and the performer, and nothing personal. Its text, `ncdb-erase-1:` and unpadded base64url, goes to a device of another account to sign off. */
export interface ErasureRequest {
  /** Its text, to hand a device of another account. */
  toString(): string;
}

/** The sign-offs of an erasure request, one per space a second account signed off. Its text, `ncdb-sign-off-1:` and unpadded base64url, goes back to the performer's `db.erase`. */
export interface SignOffs {
  /** Its text, to hand back to the performer. */
  toString(): string;
}

/** What `db.erase` did: one shred per space it erased in, the spaces it skipped and why, and the notice `erased`. */
export interface Erased {
  /** Each shred written, with its space and whether it is provisional; `shred` is the id `space.erasures.list()` names. */
  readonly shreds: readonly {
    readonly space: Id;
    readonly shred: string;
    readonly provisional: boolean;
  }[];
  /** The spaces of the request it did not erase in: no sign-off came, this member may not write there, or the person has no key there left. */
  readonly skipped: readonly {
    readonly space: Id;
    readonly reason: "noSignOff" | "notWriter" | "noKey";
  }[];
  /** `erased`, saying what the erasure does not reach. */
  readonly notice: Notice;
}

/** A text the SDK holds as it is, for a request or its sign-offs. */
class Text {
  readonly #text: string;
  constructor(text: string) {
    this.#text = text;
  }
  toString(): string {
    return this.#text;
  }
  toJSON(): string {
    return this.#text;
  }
}

/** Unpadded base64url: its alphabet, and a length no group of 4 leaves 1 character over. */
const BASE64URL = /^[A-Za-z0-9_-]+$/;

/** `text` trimmed when it is `prefix` and unpadded base64url, else `undefined`. */
const checked = (text: string, prefix: string): string | undefined => {
  const trimmed = text.trim();
  if (!trimmed.startsWith(prefix)) return undefined;
  const body = trimmed.slice(prefix.length);
  return BASE64URL.test(body) && body.length % 4 !== 1 ? trimmed : undefined;
};

/** Reads an erasure request's text, as the signing device received it. It checks only the prefix and the encoding, and fails with `invalidValue` otherwise; `db.account.signOff` checks the rest. */
export const erasureRequest = (text: string): ErasureRequest => {
  const read = checked(text, "ncdb-erase-1:");
  if (read === undefined) {
    throw new NcError("invalidValue", texts.NOT_AN_ERASURE_REQUEST);
  }
  return new Text(read);
};

/** Reads the text of sign-offs, as the performer received it. It checks only the prefix and the encoding, and fails with `invalidValue` otherwise; `db.erase` checks the rest. */
export const signOffs = (text: string): SignOffs => {
  const read = checked(text, "ncdb-sign-off-1:");
  if (read === undefined) {
    throw new NcError("invalidValue", texts.NOT_SIGN_OFFS);
  }
  return new Text(read);
};

/** `db.erasureRequest` and `db.erase` for the core `ask` speaks to; an object the SDK of this database did not read is refused before the core is asked. */
export const erasureCallsOf = (
  ask: Ask,
  read: ReturnType<typeof readObjects>,
) => ({
  erasureRequest: async (object: object): Promise<ErasureRequest> => {
    const source = read.sourceOf(object, texts.ERASURE_REQUEST_NOT_READ);
    return erasureRequest(
      (await ask("erasureRequest", { ...source })) as string,
    );
  },
  erase: async (request: ErasureRequest, given: SignOffs): Promise<Erased> =>
    (await ask("erase", {
      request: String(request),
      signOffs: String(given),
    })) as Erased,
});

/** `db.account.signOff` for the core `ask` speaks to. */
export const signOffOf =
  (ask: Ask) =>
  async (
    request: ErasureRequest,
  ): Promise<{
    readonly signOffs: SignOffs;
    readonly notSigned: readonly Id[];
  }> => {
    const signed = (await ask("account.signOff", {
      request: String(request),
    })) as { signOffs: string; notSigned?: Id[] };
    return {
      signOffs: signOffs(signed.signOffs),
      notSigned: signed.notSigned ?? [],
    };
  };

/** `space.erasures.takeBack` of the space `space`. */
export const takeBackOf =
  (ask: Ask, space: Id) =>
  async (shred: string): Promise<Notice> =>
    (await ask("erasures.takeBack", { space, shred })) as Notice;
