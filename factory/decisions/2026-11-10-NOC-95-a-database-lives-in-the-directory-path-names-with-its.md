# 2026-11-10 · NOC-95 · A database lives in the directory path names, with its keys in the platform keystore or a key file

- Decided by: Carsten, 2026-11-10, on NOC-95 (RFC 0009 decision S19, RFC 0003 decision 16)
- Why: `OpenOptions::path(dir)` for the database, the member key in the platform keystore, `key_file(path)` for a headless agent without one, and `in_memory()` for tests: it works the same for a desktop app, a server and an agent in a container, and only an app that asks for a key file gets one.
- Instead of: the platform's app data directory found from an app id, which suits apps but rarely servers and agents (S19 B); in memory only until a keystore ticket, which keeps no native data (S19 C).
- Topics: rfc-0009, rfc-0003, sdk-rust, keys, on-disk-format
