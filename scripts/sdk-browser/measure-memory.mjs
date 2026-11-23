// Measures the memory of one large write in a browser tab (NOC-422), for
// docs/bulk-import.md: the TypeScript SDK in a normal window on a fresh
// profile (engine.mjs), with no relay, writes `--records` tasks of the
// bulk import measurement into one space as one write, one change, and
// prints the peak resident memory of the tab's renderer process before
// and after. The core's worker, its WebAssembly memory and the page all
// live in that process. The peak is read from /proc, so this runs on
// Linux only.
//
//   node scripts/sdk-browser/build.mjs
//   node scripts/sdk-browser/measure-memory.mjs --records 40000
//
// SDK_BROWSER_ENGINE names the engine, as for the e2e files. Not a test.
import { readdirSync, readFileSync } from "node:fs";
import { parseArgs } from "node:util";
import { LOCK } from "./build.mjs";
import { engine, launch } from "./engine.mjs";
import { startServer } from "./server.mjs";

const { values } = parseArgs({
  options: { records: { type: "string", default: "40000" } },
});
const records = Number(values.records);
if (!Number.isInteger(records) || records <= 0) {
  throw new Error("--records takes a positive integer");
}

/** The highest peak resident memory of a renderer process, in MB. */
const rendererPeak = () => {
  let peak = 0;
  for (const pid of readdirSync("/proc").filter((p) => /^\d+$/.test(p))) {
    try {
      const command = readFileSync(`/proc/${pid}/cmdline`, "utf8");
      if (!command.includes("--type=renderer")) continue;
      const status = readFileSync(`/proc/${pid}/status`, "utf8");
      const kib = Number(status.match(/VmHWM:\s+(\d+)/)?.[1] ?? 0);
      peak = Math.max(peak, (kib * 1024) / 1e6);
    } catch {
      // The process ended while it was read.
    }
  }
  return peak;
};

const lock = JSON.parse(readFileSync(LOCK, "utf8"));
const server = await startServer({ port: 0 });
const browser = await launch();
try {
  const page = await (await browser.newContext()).newPage();
  await page.goto(`${server.url}/`);
  await page.waitForFunction(() => window.ready === true);
  await page.evaluate(
    async ({ lock }) => {
      const { nc } = window;
      // The test lock's model, as crates/nocentraldb/tests/model/model.ts
      // declares it.
      const model = nc.model({
        tasks: nc.collection(
          {
            title: nc.string(),
            status: nc.oneOf(["open", "done"]),
            priority: nc.int(),
            estimate: nc.float(),
            urgent: nc.boolean(),
            tags: nc.set(nc.string()),
            project: nc.link("projects"),
            customer: nc.ref("projects"),
            doneOn: nc.date(),
            dueAt: nc.timestamp(),
          },
          {
            indexes: {
              byPriority: nc.index(["priority"]),
              byStatusPriority: nc.index(["status", "priority"], { equal: 1 }),
              open: nc.index(["status", "priority"], {
                equal: 1,
                scope: "space",
              }),
              byDueAt: nc.index([{ field: "dueAt", direction: "desc" }]),
            },
          },
        ),
        projects: nc.collection({ name: nc.string() }),
      });
      window.measured = {
        model,
        db: await nc.open({ model, lock, name: "measure-memory" }),
      };
      window.measured.space = await window.measured.db.spaces.create();
    },
    { lock },
  );
  const before = rendererPeak();
  const { outcome, ms } = await page.evaluate(async (records) => {
    const { nc } = window;
    const { model, space } = window.measured;
    const start = performance.now();
    // The records of examples/bulk_import of the crate nocentraldb.
    const outcome = await space
      .write((tx) => {
        for (let i = 0; i < records; i++) {
          tx.create(model.tasks, {
            title: `Imported task ${String(i).padStart(6, "0")}: follow up on the old record`,
            status: i % 3 === 0 ? "done" : "open",
            priority: i % 5,
            estimate: 0.5 * (i % 8),
            urgent: i % 10 === 0,
            tags: nc.add("imported", i % 2 === 0 ? "even" : "odd"),
          });
        }
      })
      .then(
        () => "written",
        (e) => `${e.code ?? e.name}: ${e.message}`,
      );
    await window.measured.db.close();
    return { outcome, ms: performance.now() - start };
  }, records);
  const after = rendererPeak();
  console.log(`${engine}, ${records} records in one write: ${outcome}`);
  console.log(`written in:            ${(ms / 1000).toFixed(1)} s`);
  console.log(`renderer peak before:  ${before.toFixed(0)} MB`);
  console.log(`renderer peak after:   ${after.toFixed(0)} MB`);
} finally {
  await browser.close();
  await server.close();
}
