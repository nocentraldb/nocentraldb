// The types of accounts in the TypeScript SDK (NOC-316; RFC 0009,
// "Accounts and devices"; the shapes the owner decided on NOC-321,
// Carsten, 2026-11-15). What makes them is in accountCalls.ts.

/** The account's recovery code, held until the person stored it (RFC 0010, "The recovery code"). Its string, JSON and inspect forms are `"RecoveryCode(redacted)"`. */
export interface RecoveryCode {
  /** The twelve words, for the person to write down or keep in a password manager. */
  words(): readonly string[];
  /** The person stored the code: the database forgets it, and `recoveryCode()` answers `null` from then on. */
  confirm(): Promise<void>;
}
