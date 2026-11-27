/** Returns the package name; exists so the gates have real code to check. */
export function name(): string {
  return "@nocentraldb/sdk";
}

/** The account's recovery code (RFC 0009, "Accounts and devices"; NOC-316). */
export type { RecoveryCode } from "./account.ts";
/** The TypeScript types of a database, a space and a transaction (RFC 0009, "Public names"; Carsten, 2026-11-12, NOC-274). */
/** The TypeScript types of a query and its page (RFC 0009, "Public names"; NOC-176 slice 2). */
export type { Db, Page, Query, Space, Tx } from "./db.ts";
/** The durability of the copy (RFC 0006, "What the SDK reports"; RFC 0009, "Public names"; NOC-315). */
export type { Durability } from "./durability.ts";
/** A rich-text field's binding, its status and its vocabulary (RFC 0013, "An editor: the binding"; NOC-461; the status as the owner decided on NOC-538, Q2). */
export type {
  RichTextBinding,
  RichTextStatus,
  Vocabulary,
} from "./editor.ts";
/** An erasure request, its sign-offs and what `db.erase` did (RFC 0009, "Erasing a person"; NOC-406). */
export type { Erased, ErasureRequest, SignOffs } from "./erase.ts";
/** An invitation, and the notice a membership call returns (RFC 0009, "Invitations and members"; NOC-176 slice 4b). */
export type { Invitation, Notice } from "./members.ts";
/** The schema functions of RFC 0005: `nc.model`, `nc.collection`, the field types and `nc.index`, and the types `nc.Infer` and `nc.Unknown`. */
export * as nc from "./nc.ts";
/** A space's status (RFC 0009, "Sync and status"; NOC-176 slice 4). */
export type { SpaceStatus } from "./status.ts";
/** The sync report (RFC 0009, "Sync and status"; NOC-176 slice 4). */
export type { SyncStatus } from "./sync.ts";
/** A pairing answer, which a device of the account hands a new one (RFC 0009, "Accounts and devices"; NOC-316). Its own line, so that the export of `RecoveryCode` above stays as it was. */
export type PairingAnswer = import("./account.ts").PairingAnswer;
