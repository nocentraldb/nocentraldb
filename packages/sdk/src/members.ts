// Invitations and members (RFC 0009, "Invitations and members"; NOC-176
// slice 4b): the types an app sees and `nc.invitation`. membership.ts
// implements a space's `members` over the bridge's `invitation` and
// `members.*` calls (crates/nocentraldb/src/bridge/members.rs). A member id is its 64
// lowercase hex digits (Carsten, 2026-11-13, NOC-298). What RFC 0009 leaves
// open follows NOC-305 A and A (Carsten, 2026-11-13).

import { NcError } from "./error.ts";

/**
 * A member's invitation: its public identity, which an admin passes to
 * `space.members.add` (RFC 0009, "Invitations and members"). Nothing in it
 * is secret. `toString()` gives its text form, `ncdb-invite-1:…`.
 */
export interface Invitation {
  /** The fingerprint of its account, as twelve blocks of five digits: known for this member's own invitation, from `db.invitation()`; unset for one read with `nc.invitation`, whose fingerprint `members.add` gives with `unknownAccount`. */
  readonly fingerprint?: string;
  /** The invitation's text, `ncdb-invite-1:` and base64url. */
  toString(): string;
}

/** What a call that changes who can read or write does not do: its `code`, named by the call, and a plain English `text` for the developer (RFC 0009, "Notices"). */
export interface Notice {
  readonly code: string;
  readonly text: string;
}

/** A role in a space (RFC 0008, "Roles and rights"). */
type Role = "reader" | "writer" | "admin";

/** A member of a space, as `space.members.list()` gives it. */
interface Member {
  /** Its id, 64 lowercase hex digits, which `setRole` and `remove` take. */
  readonly id: string;
  /** The fingerprint of the account its certificate names. */
  readonly account: string;
  /** The fingerprint of the account `account` is linked into, when this database knows the link; `null` otherwise (RFC 0010, "Linking accounts drawn per database"). */
  readonly linkedTo: string | null;
  readonly kind: "device" | "agent" | "subAgent";
  /** Its role; `null` while no membership of it is folded. */
  readonly role: Role | null;
  /** Whether it is this member. */
  readonly self: boolean;
}

/** The members of a space (RFC 0009, "Invitations and members"). */
export interface Members {
  /** Adds the member of each invitation; `role` may be left out for an agent. An account new to this device fails with `unknownAccount`, carrying its fingerprint, until `fingerprint` passes the one a person compared. */
  add(
    invitations: Invitation | readonly Invitation[],
    options?: { role?: Role; fingerprint?: string },
  ): Promise<{ readonly notice: Notice }>;
  /** The current members. */
  list(): Promise<Member[]>;
  /** Raises or lowers a member's role. */
  setRole(member: string, role: Role): Promise<{ readonly notice: Notice }>;
  /** Removes a member and rotates the space key. */
  remove(member: string): Promise<{ readonly notice: Notice }>;
}

const PREFIX = "ncdb-invite-1:";
/** Unpadded base64url: its alphabet, and a length no group of 4 leaves 1 character over. */
const BASE64URL = /^[A-Za-z0-9_-]+$/;

/** An invitation the SDK holds as its text. */
class InvitationText implements Invitation {
  readonly fingerprint?: string;
  readonly #text: string;
  constructor(text: string, fingerprint?: string) {
    this.#text = text;
    if (fingerprint !== undefined) this.fingerprint = fingerprint;
  }
  toString(): string {
    return this.#text;
  }
  toJSON(): string {
    return this.#text;
  }
}

/** Reads an invitation from its text. It checks the form, `ncdb-invite-1:` and unpadded base64url, and fails with `invalidValue` otherwise; the core checks the rest when `members.add` reads it. */
export const invitation = (text: string): Invitation => {
  const trimmed = text.trim();
  const body = trimmed.startsWith(PREFIX)
    ? trimmed.slice(PREFIX.length)
    : undefined;
  if (body === undefined || !BASE64URL.test(body) || body.length % 4 === 1) {
    throw new NcError(
      "invalidValue",
      "the text is not an invitation: ncdb-invite-1: and unpadded base64url",
    );
  }
  return new InvitationText(trimmed);
};
