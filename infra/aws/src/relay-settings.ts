// The relay's settings, and the `NCDB_*` variables they become on the
// function (ncdb_relay_lambda::settings). This is the one place a new
// setting of the relay goes: a field in `RelaySettings`, its check, and its
// variable in `relayEnvironment`.

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
  return {
    NCDB_RELAY_ID: settings.relayId,
    NCDB_ALLOWED_ACCOUNTS: settings.allowedAccounts.join(","),
    ...(admissionKeys.length > 0
      ? { NCDB_ADMISSION_KEYS: admissionKeys.join(",") }
      : {}),
  };
}
