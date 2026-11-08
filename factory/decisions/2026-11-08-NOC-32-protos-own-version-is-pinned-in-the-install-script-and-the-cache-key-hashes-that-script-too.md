# 2026-11-08 · NOC-32 · proto's own version is pinned in the install script, and the cache key hashes that script too

- Decided by: coding agent
- Why: setup-toolchain installed whatever proto release was latest (0.63.2 on 2026-11-08), so proto had no pin to keep; the cache now freezes a version, so it needs one. `.prototools` cannot hold it: proto ignores a `proto` pin there until each machine runs `proto trust`. The key is `proto-<os>-<hash of .prototools and scripts/install-toolchain.sh>`, so bumping proto installs cold. `.moon/toolchains.yml` pins the proto moon runs its tasks with to the same version, held equal by the contract test; without it moon 2.6.5 downloaded proto 0.62.2 on the first task of every gates run, unretried. Dependabot does not bump this pin.
- Instead of: a key on `.prototools` alone, which would keep an old proto after a version bump.
- Topics: ci, dependencies, security
