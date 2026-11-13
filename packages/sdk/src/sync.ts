// The sync report's type (RFC 0009, "Sync and status"; NOC-176 slice 4):
// what `db.sync.current()` gives and `db.sync.subscribe` passes. connect.ts
// keeps it from the core's `sync` events.

/** The relay connection and what syncing found, as `db.sync.current()` gives it (RFC 0009, "Sync and status"). */
export interface SyncStatus {
  /** `"offline"`: no relay reachable, and the SDK retries; `"none"`: opened without a relay. */
  readonly connection:
    | "connected"
    | "connecting"
    | "offline"
    | "paused"
    | "none";
  /** The relay `nc.open` named; absent without one. */
  readonly relay?: string;
  /** This member's changes the relay has not confirmed. */
  readonly unconfirmed: number;
  /** This member's changes the relay refused. */
  readonly refused: number;
  /** Whether the relay was restored and the SDK is pushing again what it lost. */
  readonly epochChanged: boolean;
  /** The last error that stops syncing, by its RFC 0007 code, such as `unsupported_version`, or `unknown_error`; never content. Absent while syncing runs. */
  readonly error?: string;
}
