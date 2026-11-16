// When a worker tells its tab that it waits for the database's storage,
// which another tab of the app still holds (NOC-355): the tab reports it as
// `db.durability`'s `waitingForTab`. worker.ts calls `started` when it finds
// the storage held and `ended` once it holds it. Internal: the package's
// entry does not reach it.

/** Reports `true` to `report` when a wait starts, and `false` when it ends. */
export const waitNotice = (report: (waiting: boolean) => void) => {
  let told = false;
  return {
    started() {
      told = true;
      report(true);
    },
    ended() {
      if (told) report(false);
      told = false;
    },
  };
};
