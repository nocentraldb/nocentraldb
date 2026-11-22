# 2026-11-22 · NOC-447 · Later edits to a moved object stay in the old space, the SDK reports them, and the app carries them over

- Decided by: Carsten, 2026-11-22, on RFC 0016's questions, through pm (NOC-449)
- Why: a carried-over edit gets a new stamp and could overwrite newer writes in the new space, so the app or a person decides; the edits stay identifiable on every member of the old space.
- Instead of: the mover's devices, or any member of both spaces, carrying them over on their own.
- Topics: move, convergence, sdk-ts, sdk-rust, rfc-0016
