# 2026-11-08 · NOC-32 · CI installs proto with its own script instead of moonrepo/setup-toolchain

- Decided by: coding agent
- Why: setup-toolchain v0.6.4 downloads proto's install script and proto itself on every run, before it looks at its cache, and restores with a fallback key; it has no input to change either. `scripts/install-toolchain.sh` downloads proto only when the cache lacks it, retries with backoff, and names the host it could not reach.
- Instead of: keeping setup-toolchain and only adding retries around it, which still needs proto's download host on every run.
- Topics: ci, dependencies
