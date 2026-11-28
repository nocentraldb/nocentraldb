// The relay's settings, and the `NCDB_*` variables they become on the
// function (ncdb_relay_lambda::settings). This is the one place a new
// setting of the relay goes: a field in `RelaySettings`, its check, and its
// variable in `relayEnvironment`, or, for a setting of the construct's own
// resources such as the log retention, the function that reads it here.

import { RetentionDays } from "aws-cdk-lib/aws-logs";

/// What the relay itself is told, independent of where it is deployed.
export interface RelaySettings {
  /// The relay's stable id, 32 lowercase hex digits, drawn once per
  /// deployment and never changed: members sign it into every
  /// connection's authentication (RFC 0007, "Opening a connection").
  readonly relayId: string;
  /// The identities of the accounts whose members may create spaces, 64
  /// lowercase hex digits each: each account's recovery key, or the account
  /// key of an account without one (RFC 0015, "The account's identity").
  readonly allowedAccounts: readonly string[];
  /// The public admission keys whose admissions the relay takes, one per
  /// app with open signup, 64 lowercase hex digits each (RFC 0015,
  /// "Admission tokens"). Left out or empty, the relay takes none, and the
  /// function gets no `NCDB_ADMISSION_KEYS`.
  readonly admissionKeys?: readonly string[];
  /// How many days the relay keeps a connection's records after the
  /// connection ends: which member id was connected when, and to which
  /// spaces it subscribed, for abuse investigation (RFC 0007, "What the
  /// relay sees"; RFC 0007 suggests 30). Left out, the records go when the
  /// connection closes, and the function gets no
  /// `NCDB_KEEP_CONNECTIONS_DAYS`.
  readonly keepConnectionRecordsDays?: number;
  /// How many days CloudWatch Logs keeps the function's logs: 14 when left
  /// out, or a longer period CloudWatch Logs offers (30, 60, 90, 120, 150,
  /// 180, 365, 400, 545, 731, 1096, 1827, 2192, 2557, 2922, 3288 or 3653;
  /// RFC 0007, "What the relay sees").
  readonly logRetentionDays?: number;
}

/// The periods CloudWatch Logs offers that are no shorter than RFC 0007's
/// 14 days. Keeping logs forever is not a period, so it is not offered.
const LOG_RETENTION: readonly RetentionDays[] = Object.values(RetentionDays)
  .filter((days): days is RetentionDays => typeof days === "number")
  .filter((days) => days >= 14 && days !== RetentionDays.INFINITE);

/// How long the function's logs are kept, after checking the setting.
export function logRetention(settings: RelaySettings): RetentionDays {
  const days = settings.logRetentionDays ?? RetentionDays.TWO_WEEKS;
  const offered = LOG_RETENTION.find((period) => period === days);
  if (offered === undefined) {
    throw new Error(
      "logRetentionDays is 14 or a longer period CloudWatch Logs offers",
    );
  }
  return offered;
}

/// The settings as the function's environment, after checking them.
export function relayEnvironment(
  settings: RelaySettings,
): Record<string, string> {
  if (!/^[0-9a-f]{32}$/.test(settings.relayId)) {
    throw new Error("relayId is 32 lowercase hex digits");
  }
  for (const key of settings.allowedAccounts) {
    if (!/^[0-9a-f]{64}$/.test(key)) {
      throw new Error("an allowed account is 64 lowercase hex digits");
    }
  }
  const admissionKeys = settings.admissionKeys ?? [];
  for (const key of admissionKeys) {
    if (!/^[0-9a-f]{64}$/.test(key)) {
      throw new Error("an admission key is 64 lowercase hex digits");
    }
  }
  const keep = settings.keepConnectionRecordsDays;
  if (keep !== undefined && !(Number.isInteger(keep) && keep >= 1)) {
    throw new Error(
      "keepConnectionRecordsDays is a whole number of days, at least 1",
    );
  }
  return {
    NCDB_RELAY_ID: settings.relayId,
    NCDB_ALLOWED_ACCOUNTS: settings.allowedAccounts.join(","),
    ...(admissionKeys.length > 0
      ? { NCDB_ADMISSION_KEYS: admissionKeys.join(",") }
      : {}),
    ...(keep !== undefined ? { NCDB_KEEP_CONNECTIONS_DAYS: String(keep) } : {}),
  };
}
