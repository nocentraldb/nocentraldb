// What keeps a database's OPFS alive in a Chromium private window
// (NOC-444). In an off-the-record context Chromium keeps the origin's OPFS
// in memory and drops it after about ten minutes without a file operation
// (measured in Chromium 157: kept after 450 s of quiet, gone after 600 s).
// Every access handle then fails, which SQLite reports as a disk I/O
// error, and the files are gone. A call on a file or directory handle
// every minute keeps all of it, handles and files, even a call that finds
// nothing; `getDirectory()` and `estimate()` alone do not. worker.ts starts the keep-alive when an open
// begins, which may run for minutes on a large database, and stops it when
// the open fails or the worker stops. It costs one call a
// minute, in every browser, since the SDK cannot tell a private window
// from a normal one. Internal: the package's entry does not reach it.

/** How often the keep-alive touches OPFS, in milliseconds: well inside the ten minutes. */
export const KEEP_ALIVE_EVERY = 60_000;

/** Calls `touch` every `every` milliseconds between `start` and `stop`; a touch that fails is ignored, and the next one runs. */
export const keepAlive = (
  touch: () => Promise<unknown>,
  every = KEEP_ALIVE_EVERY,
) => {
  let timer: ReturnType<typeof setInterval> | undefined;
  return {
    start() {
      if (timer !== undefined) return;
      timer = setInterval(() => {
        touch().catch(() => {});
      }, every);
    },
    stop() {
      clearInterval(timer);
      timer = undefined;
    },
  };
};
