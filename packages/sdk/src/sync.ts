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
  /** The relay `nc.open` named, or the one its relay function last gave, without user, password, query string and fragment; absent without one. */
  readonly relay?: string;
  /** This member's changes the relay has not confirmed. */
  readonly unconfirmed: number;
  /** This member's changes the relay refused. */
  readonly refused: number;
  /** Whether the relay was restored and the SDK is pushing again what it lost. */
  readonly epochChanged: boolean;
  /** The last error of syncing; never content. An error that stops syncing, by its RFC 0007 code, such as `unsupported_version`, or `unknown_error`; or `upload_failed` while a large change's upload is refused, which keeps syncing and retries the upload after a backoff; or `relay_url_failed` while the function given as `relay` throws or gives no `ws://` or `wss://` URL, which the SDK calls again after a backoff (NOC-496). A browser does not tell the SDK why a WebSocket failed, so a connection the relay's authorizer refuses shows as `"offline"` without an error, as an unreachable relay does; the Rust SDK reports it as `connect_refused`. Absent while syncing runs. */
  readonly error?: string;
}
