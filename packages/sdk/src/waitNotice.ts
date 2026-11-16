// When a worker tells its tab that it waits for the database's storage,
// which another tab of the app still holds (NOC-355): the tab reports it as
// `db.durability`'s `waitingForTab`. worker.ts calls `started` when it finds
// the storage held and `ended` once it holds it. The wait is reported only
// once it has lasted `WAIT_NOTICE_AFTER`, so a normal handover, where the
// old tab's worker is still stopping for a few milliseconds, reports
// nothing (Carsten, 2026-11-16, NOC-355). The worker's timer runs while a
// frozen owner holds the storage, in WebKit too (measured, NOC-355).
// Internal: the package's entry does not reach it.

/** How long a wait lasts before it is reported, in milliseconds. */
export const WAIT_NOTICE_AFTER = 1000;

/** Reports `true` to `report` once a wait has lasted `after`, and `false` when a reported wait ends. */
export const waitNotice = (
  report: (waiting: boolean) => void,
  after = WAIT_NOTICE_AFTER,
) => {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let told = false;
  return {
    started() {
      clearTimeout(timer);
      timer = setTimeout(() => {
        told = true;
        report(true);
      }, after);
    },
    ended() {
      clearTimeout(timer);
      timer = undefined;
      if (told) report(false);
      told = false;
    },
  };
};
