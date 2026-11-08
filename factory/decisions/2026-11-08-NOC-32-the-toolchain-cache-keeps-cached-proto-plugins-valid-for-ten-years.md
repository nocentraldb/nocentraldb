# 2026-11-08 · NOC-32 · The toolchain cache keeps cached proto plugins valid for ten years

- Decided by: coding agent
- Why: proto deletes and re-downloads a cached plugin once its file is older than proto's cache duration (between 160 and 250 days in a local test), and a restored cache keeps the files' original times; a long-lived cache entry would then go back to ghcr.io on a warm run. The script sets `PROTO_CACHE_DURATION` for the whole job. The plugin versions are fixed by proto's version, so a long duration loosens no pin.
- Instead of: proto's default, or `cache-duration` in `.prototools`, which proto ignores in an untrusted config.
- Topics: ci, dependencies
