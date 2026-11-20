# 2026-11-09 · NOC-64 · nc.open takes the model, its lock and one relay, and refuses a model that disagrees with the lock

- Decided by: Carsten, 2026-11-09, in chat with pm, on #78 (RFC 0009 decisions S1 and S16)
- Why: RFC 0005 says the core reads only the lock, and RFC 0006 had already decided `nc.open({ model, ... })`, so the open only adds `lock`; its check (`lockOutOfDate`) catches the most common mistake, a schema edited without `ncdb model build`. One relay per database for v1, because moving a space between relays is not designed (RFC 0007); a relay per space can come later as an option of `db.spaces.create`.
- Instead of: `nc.open({ lock })` typed with a generic and handles from the lock by name, which loses the handles that make queries typed (S1 B); a module that `ncdb model build` generates and the app opens with, a generated file in every app's repository (S1 C); the SDK building the lock in memory from the TypeScript model; a relay per space (S16 B).
- Topics: rfc-0009, sdk, api, model-lock, relay
