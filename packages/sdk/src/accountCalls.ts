// The account calls of the TypeScript SDK (NOC-316; RFC 0009, "Accounts and
// devices"; the shapes the owner decided on NOC-321, Carsten, 2026-11-15).
// Internal: the package's entry does not reach it.

import {
  type PairingAnswer,
  pairingAnswer,
  type RecoveryCode,
} from "./account.ts";
import type { Cbor } from "./cbor.ts";
import type { Invitation, Notice } from "./members.ts";

type Ask = (call: string, args: Record<string, unknown>) => Promise<Cbor>;

const REDACTED = "RecoveryCode(redacted)";

/** A recovery code whose words only `words()` gives: its string, JSON and Node's inspect forms are redacted (RFC 0009). */
const recoveryCodeOf = (words: readonly string[], ask: Ask): RecoveryCode => {
  const kept = Object.freeze([...words]);
  const code = {
    words: () => kept,
    confirm: async () => {
      await ask("account.confirmRecoveryCode", {});
    },
    toString: () => REDACTED,
    toJSON: () => REDACTED,
    [Symbol.toPrimitive]: () => REDACTED,
    [Symbol.for("nodejs.util.inspect.custom")]: () => REDACTED,
  };
  return Object.freeze(code);
};

/** `db.account` for the core `ask` speaks to. */
export const accountOf = (ask: Ask) => ({
  recoveryCode: async (): Promise<RecoveryCode | null> => {
    const answer = (await ask("account.recoveryCode", {})) as {
      words?: unknown;
    } | null;
    return Array.isArray(answer?.words)
      ? recoveryCodeOf(answer.words as string[], ask)
      : null;
  },
  recovered: async (): Promise<Notice | null> =>
    ((await ask("account.recovered", {})) as Notice | null) ?? null,
  pair: async ({
    kind,
    invitation,
  }: {
    kind: "device" | "agent";
    invitation: Invitation;
  }) => {
    const paired = (await ask("account.pair", {
      kind,
      invitation: String(invitation),
    })) as unknown as {
      answer: string;
      added: { space: string; notice: Notice }[];
      askAdmins: string[];
    };
    return {
      answer: pairingAnswer(paired.answer) as PairingAnswer,
      added: paired.added ?? [],
      askAdmins: paired.askAdmins ?? [],
    };
  },
  completePairing: async (answer: PairingAnswer) =>
    (await ask("account.completePairing", {
      answer: String(answer),
    })) as unknown as { readonly notices: readonly Notice[] },
  removeMember: async (member: string, { recover }: { recover: string }) =>
    (await ask("account.removeMember", {
      member,
      recover,
    })) as unknown as {
      readonly removed: readonly {
        readonly space: string;
        readonly notice: Notice;
      }[];
      readonly askAdmins: readonly string[];
      readonly notice: Notice;
    },
});
