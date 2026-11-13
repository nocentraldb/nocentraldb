// A space's status (RFC 0009, "Sync and status"; NOC-176 slice 4): what
// `space.status.current()` gives and `space.status.subscribe` passes.
// connect.ts keeps it from the core's `space.status` answer and its
// `spaceStatus` events.

/** What a space holds that a read does not show, as `space.status.current()` gives it (RFC 0009, "Sync and status"). */
export interface SpaceStatus {
  /** This member's role, or `"removed"`; a removed member keeps reading its copy. */
  readonly role: "reader" | "writer" | "admin" | "removed";
  /** Whether the first pull reached the relay's head. */
  readonly initialSyncComplete: boolean;
  /** How many relay positions the member has heard of but not folded. */
  readonly behind: number;
  /** Dependencies the member does not hold, by change id. */
  readonly gaps: readonly string[];
  /** The parked changes and operations, per reason, such as `anotherModel`. */
  readonly parked: Readonly<Record<string, number>>;
  /** The highest version of the app's model that parked data needs; absent when none names one. An app that sees it above its own version can say that some data needs a newer version of the app. */
  readonly newestModel?: number;
  /** The rejected changes, per reason, such as `badSignature`. */
  readonly rejected: Readonly<Record<string, number>>;
  /** This member's changes in this space the relay refused, by change id and RFC 0007 code. */
  readonly refused: readonly {
    readonly change: string;
    readonly code: string;
  }[];
  /** Grants a concurrent removal of their member capped, by change id. */
  readonly lostGrants: readonly string[];
  /** Changes of members that wrote two changes under one sequence, by change id. */
  readonly equivocations: readonly string[];
  /** Whether members write under a key generation a removed member may hold. */
  readonly exposed: boolean;
  /** Whether this member waits for its key envelope. */
  readonly waitingForKey: boolean;
  /** The fingerprints of accounts whose person-key records failed their checks. */
  readonly badPersonKeyRecords: readonly string[];
  /** The erasures of this space that are still provisional; an app must show them. */
  readonly provisionalErasures: readonly ProvisionalErasure[];
}

/** A provisional erasure in a space with one account (RFC 0008, "A provisional erasure in a space with one account"). */
interface ProvisionalErasure {
  /** The shred, by change id. */
  readonly change: string;
  /** How long ago this device first held the shred, in milliseconds. */
  readonly age: number;
  /** Whether this device may take the erasure back. */
  readonly canUndo: boolean;
  /** The member that performed it. */
  readonly performer: string;
  /** The notice `erasurePending`. */
  readonly notice: { readonly code: string; readonly text: string };
}
