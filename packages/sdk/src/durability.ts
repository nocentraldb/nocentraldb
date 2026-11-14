// The types of the durability of the copy and of the view's report
// (NOC-315, slice 3; RFC 0006, "What the SDK reports"; RFC 0009, "Sync and
// status"; the shapes the owner decided on NOC-317, Carsten, 2026-11-14).
// What follows them is in durabilityReport.ts.

/** What this copy would lose if the browser deleted it now (RFC 0006, "What the SDK reports"). */
export interface Durability {
  /** `"persistent"`: native, or the browser keeps the copy; `"best-effort"`: the browser may delete it; `"temporary"`: in memory, where OPFS is missing. */
  readonly storage: "persistent" | "best-effort" | "temporary";
  /** This member's changes the relay has not confirmed: they exist only on this device. */
  readonly unconfirmed: number;
  /** When the oldest of them was written, in milliseconds since the epoch, or `null`. */
  readonly oldestUnconfirmedAt: number | null;
  /** Whether some changes are unconfirmed while the copy is not persistent. */
  readonly atRisk: boolean;
}

/** The view's report: whether a rebuild runs, and how far it is, in frames of the log. */
export interface ViewStatus {
  readonly rebuild: { readonly folded: number; readonly total: number } | null;
}
