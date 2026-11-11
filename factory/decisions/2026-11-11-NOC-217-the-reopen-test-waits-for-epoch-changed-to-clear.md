# 2026-11-11 · NOC-217 · The reopen-after-restore test waits for `epoch_changed` to clear; the SDK is unchanged

- Decided by: coding agent (NOC-217)
- Why: since NOC-145 a reopened database keeps the relay epoch, so after a restore its first `welcome` sees a new epoch and `epoch_changed` is true, as RFC 0007 ("Restores and the relay epoch") and the field's documentation ("the relay was restored and the SDK is pushing again what it lost") want. The SDK clears it when its pass over the spaces ends, which can come after the relay confirmed the last change, the only thing the test's `synced` waits for. The test read it in between and failed about one run in eight under load. It now waits for the flag to clear, the state it asserts.
- Instead of: clearing `epoch_changed` as soon as no change is unconfirmed, which changes what the SDK reports while it still pulls the restored log from 0, for a test's sake.
- Topics: sdk-rust, sync, tests
