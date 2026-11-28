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

/** The `identity` of an answer of `open` or `account.completePairing`: the recovery key in hex, or null. */
const identityOf = (answer: unknown): string | null => {
  const identity = (answer as { identity?: unknown } | null)?.identity;
  return typeof identity === "string" ? identity : null;
};

/** A recovery code whose words only `words()` gives: its string, JSON and Node's inspect forms are redacted (RFC 0009). `challenge` holds the two positions the core picked (NOC-502). */
const recoveryCodeOf = (
  words: readonly string[],
  challenge: readonly [number, number],
  ask: Ask,
): RecoveryCode => {
  const kept = Object.freeze([...words]);
  const asked = Object.freeze([...challenge]) as readonly [number, number];
  const code = {
    words: () => kept,
    challenge: () => asked,
    confirm: async (answers: readonly [string, string]) => {
      await ask("account.confirmRecoveryCode", {
        answers: asked.map((position, i) => ({
          position,
          word: answers[i],
        })),
      });
    },
    toString: () => REDACTED,
    toJSON: () => REDACTED,
    [Symbol.toPrimitive]: () => REDACTED,
    [Symbol.for("nodejs.util.inspect.custom")]: () => REDACTED,
  };
  return Object.freeze(code);
};

/** `db.account` for the core `ask` speaks to; `identity` is the one the core answered on open (RFC 0015, "In the SDK"). */
export const accountOf = (ask: Ask, opened: Cbor) => {
  let identity = identityOf(opened);
  return {
    get identity(): string | null {
      return identity;
    },
    recoveryCode: async (): Promise<RecoveryCode | null> => {
      const answer = (await ask("account.recoveryCode", {})) as {
        words?: unknown;
        challenge?: unknown;
      } | null;
      return Array.isArray(answer?.words) && Array.isArray(answer?.challenge)
        ? recoveryCodeOf(
            answer.words as string[],
            answer.challenge as [number, number],
            ask,
          )
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
    pairingFingerprint: async (invitation: Invitation): Promise<string> =>
      (await ask("account.pairingFingerprint", {
        invitation: String(invitation),
      })) as string,
    completePairing: async (answer: PairingAnswer) => {
      const completed = (await ask("account.completePairing", {
        answer: String(answer),
      })) as unknown as { readonly notices: readonly Notice[] };
      identity = identityOf(completed);
      return { notices: completed.notices };
    },
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
  };
};
