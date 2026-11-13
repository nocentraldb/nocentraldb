// `nc.Error`, the one error class of the SDK (RFC 0009, "Errors"; Carsten,
// 2026-11-12, NOC-274): a string `code` that maps one to one to the Rust
// SDK's `ErrorCode`, the ids the error concerns as text, and, for
// `unknownAccount`, the account's fingerprint as a plain string of twelve
// blocks. An error never carries content, a key or a personal field (RFC
// 0003, "Keys never reach logs, errors or telemetry").

/** The one error class the SDK throws; match on `code`. */
export class NcError extends Error {
  /** RFC 0009's error code, such as `notFound` or `invalidValue`. */
  readonly code: string;
  /** The ids the error concerns, each as text. */
  readonly ids: readonly string[];
  /** For `unknownAccount`: the account's fingerprint, twelve blocks. */
  readonly fingerprint?: string;

  constructor(
    code: string,
    message: string,
    ids: readonly string[] = [],
    fingerprint?: string,
  ) {
    super(message);
    this.name = "nc.Error";
    this.code = code;
    this.ids = [...ids];
    if (fingerprint !== undefined) this.fingerprint = fingerprint;
  }
}
