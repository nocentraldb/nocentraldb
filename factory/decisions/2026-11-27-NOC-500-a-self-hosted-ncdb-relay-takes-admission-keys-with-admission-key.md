# 2026-11-27 · NOC-500 · A self-hosted `ncdb-relay` takes its admission keys with `--admission-key`, once per key

- Decided by: Carsten, 2026-11-27, NOC-516
- Why: `ncdb-relay` takes every setting as a command-line flag and reads nothing from its environment, so its admission keys follow `--allow-account`: `--admission-key <hex>`, the public half of an app's admission key as 64 hex digits, given once per key. The keys fill the same `Config::admission_keys` that `NCDB_ADMISSION_KEYS` fills on AWS, and a malformed key stops the relay at start with exit code 2 and the usage.
- Instead of: reading `NCDB_ADMISSION_KEYS` from the environment, the only setting the binary would take that way; or both the flag and the variable.
- Topics: relay, admission, rfc-0015, rfc-0007
