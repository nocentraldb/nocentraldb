// The types of accounts in the TypeScript SDK (NOC-316; RFC 0009,
// "Accounts and devices"; the shapes the owner decided on NOC-321,
// Carsten, 2026-11-15). What makes them is in accountCalls.ts, but for
// `nc.pairingAnswer`, which reads an answer's text as `nc.invitation` reads
// an invitation's.

import { NcError } from "./error.ts";

/** The account's recovery code, held until the person stored it (RFC 0010, "The recovery code"). Its string, JSON and inspect forms are `"RecoveryCode(redacted)"`. */
export interface RecoveryCode {
  /** The twelve words, for the person to write down or keep in a password manager. */
  words(): readonly string[];
  /** The person stored the code: the database forgets it, and `recoveryCode()` answers `null` from then on. */
  confirm(): Promise<void>;
}

/** A pairing answer (RFC 0010, "The pairing answer"): public data with a text form, as an invitation is. It carries the account key sealed to the one device that asked, which only that device opens. */
export interface PairingAnswer {
  /** Its text, `ncdb-pair-1:` and unpadded base64url, to show the new device as a QR code or text. */
  toString(): string;
}

const PREFIX = "ncdb-pair-1:";
/** Unpadded base64url: its alphabet, and a length no group of 4 leaves 1 character over. */
const BASE64URL = /^[A-Za-z0-9_-]+$/;

/** A pairing answer the SDK holds as its text. */
class PairingAnswerText implements PairingAnswer {
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

/** Reads a pairing answer's text, as the new device received it (NOC-321 point 3). It checks only the prefix and the encoding, and fails with `invalidValue` otherwise; `completePairing` checks the rest. */
export const pairingAnswer = (text: string): PairingAnswer => {
  const trimmed = text.trim();
  const body = trimmed.startsWith(PREFIX)
    ? trimmed.slice(PREFIX.length)
    : undefined;
  if (body === undefined || !BASE64URL.test(body) || body.length % 4 === 1) {
    throw new NcError(
      "invalidValue",
      "the text is not a pairing answer: ncdb-pair-1: and unpadded base64url",
    );
  }
  return new PairingAnswerText(trimmed);
};
