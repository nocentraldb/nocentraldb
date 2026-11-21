// The erasure calls of the TypeScript SDK (NOC-406; RFC 0009, "Erasing a
// person"), over the core's `erasureRequest`, `account.signOff`, `erase`
// and `erasures.takeBack` (crates/nocentraldb/src/bridge/erase.rs). The
// types and the text readers are in erase.ts. Internal: the package's
// entry does not reach it.

import type { readObjects } from "./adHoc.ts";
import {
  type Erased,
  type ErasureRequest,
  erasureRequest,
  type SignOffs,
  signOffs,
} from "./erase.ts";
import type { Notice } from "./members.ts";
import * as texts from "./texts.ts";

type Id = string;
type Ask = (call: string, args: Record<string, unknown>) => Promise<unknown>;

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
