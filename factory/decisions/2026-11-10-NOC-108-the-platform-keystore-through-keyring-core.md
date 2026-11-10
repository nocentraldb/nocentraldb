# 2026-11-10 · NOC-108 · The platform keystore through keyring-core and one store per platform

- Decided by: coding agent (NOC-108)
- Why: RFC 0003 names the Keychain on macOS, the Credential Manager on Windows and the Secret Service on Linux. `keyring-core` =1.0.1 with `apple-native-keyring-store` =1.0.2, `windows-native-keyring-store` =1.1.0 and `zbus-secret-service-keyring-store` =1.0.1 (`crypto-rust`) reach them, each only on its own platform, and the Linux store talks D-Bus in pure Rust, so building needs no system library. The `keyring` crate itself recommends linking these directly. A keystore that cannot be reached or refuses access is `KeyStoreUnavailable`. The tests of the keystore run on a fake keystore in CI, which has none; the test of the platform's own keystore runs where one is reachable and prints why it is skipped elsewhere.
- Instead of: the `keyring` crate, which sets a process-wide default store; `dbus-secret-service`, which needs libdbus; the Linux kernel keyring, which the RFC does not name and which does not survive a reboot.
- What this does not do: iOS, Android and the browser have no keystore here; on those targets a database at a path needs a key file. The crate as a whole is native only.
- Topics: sdk-rust, keys, crates, rfc-0003
