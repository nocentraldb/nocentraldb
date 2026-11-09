// The SQLite half of the probe: one database on OPFS through the
// opfs-sahpool VFS, the VFS RFC 0006 picks for the browser. Runs in a
// dedicated worker, since the VFS needs synchronous access handles.
import sqlite3InitModule from "./sqlite/index.mjs";

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const open = async () => {
  const sqlite3 = await sqlite3InitModule();
  let lastError;
  // A reload can reach the VFS before the previous page's worker has let go
  // of its access handles, so a failed install is retried for a while.
  for (let attempt = 0; attempt < 20; attempt += 1) {
    try {
      const pool = await sqlite3.installOpfsSAHPoolVfs({
        name: "ncdb-probe",
        preserveOnInitFailure: true,
        forceReinitIfPreviouslyFailed: true,
      });
      const db = new pool.OpfsSAHPoolDb("/probe.sqlite3");
      db.exec(
        "CREATE TABLE IF NOT EXISTS probe (k TEXT PRIMARY KEY, v TEXT NOT NULL)",
      );
      return { db, version: sqlite3.version.libVersion };
    } catch (error) {
      lastError = error;
      await sleep(250);
    }
  }
  throw lastError;
};

let opened;
const database = () => {
  opened ??= open();
  return opened;
};

const ops = {
  async version() {
    return (await database()).version;
  },
  async write({ token }) {
    const { db } = await database();
    db.exec({
      sql: "INSERT OR REPLACE INTO probe (k, v) VALUES ('token', ?)",
      bind: [token],
    });
    return "ok";
  },
  async read() {
    const { db } = await database();
    const rows = db.exec({
      sql: "SELECT v FROM probe WHERE k = 'token'",
      returnValue: "resultRows",
    });
    return rows[0]?.[0] ?? null;
  },
};

self.onmessage = async ({ data }) => {
  try {
    postMessage({ id: data.id, ok: true, value: await ops[data.op](data) });
  } catch (error) {
    postMessage({
      id: data.id,
      ok: false,
      name: error?.name ?? "Error",
      message: error?.message ?? String(error),
    });
  }
};
