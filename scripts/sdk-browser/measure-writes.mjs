// Measures what writes cost in a browser as the database grows (NOC-439),
// for the browser section of docs/bulk-import.md: the TypeScript SDK in a
// normal window on a fresh profile (engine.mjs), with no relay, writes
// `--objects` tasks into one space, `--batch` to a write, and prints the
// mean cost of a write over each `--window` objects. A normal window, not
// a private one: Chromium's off-the-record context keeps OPFS in memory
// and is about 13 times slower.
//
//   node scripts/sdk-browser/build.mjs
//   node scripts/sdk-browser/measure-writes.mjs --objects 100000 --batch 1
//
// SDK_BROWSER_ENGINE names the engine, as for the e2e files. Not a test:
// long/writeCost.e2e.test.mjs keeps the curve flat.
import { readFileSync } from "node:fs";
import { parseArgs } from "node:util";
import { LOCK } from "./build.mjs";
import { engine, launch } from "./engine.mjs";
import { startServer } from "./server.mjs";

const { values } = parseArgs({
  options: {
    objects: { type: "string", default: "100000" },
    batch: { type: "string", default: "1" },
    window: { type: "string", default: "10000" },
  },
});
const objects = Number(values.objects);
const batch = Number(values.batch);
const window_ = Number(values.window);
if (![objects, batch, window_].every((n) => Number.isInteger(n) && n > 0)) {
  throw new Error("--objects, --batch and --window take positive integers");
}
if (window_ % batch !== 0 || objects % window_ !== 0) {
  throw new Error(
    "--window must be a multiple of --batch, --objects of --window",
  );
}

const lock = JSON.parse(readFileSync(LOCK, "utf8"));
const server = await startServer({ port: 0 });
const browser = await launch();
try {
  const page = await (await browser.newContext()).newPage();
  await page.goto(`${server.url}/`);
  await page.waitForFunction(() => window.ready === true);
  await page.exposeFunction("report", (objects, ms) =>
    console.log(
      `${String(objects).padStart(7)} objects: ${ms.toFixed(2)} ms a write of ${batch}`,
    ),
  );
  console.log(`${engine}, ${objects} objects, ${batch} to a write`);
  const total = await page.evaluate(
    async ({ lock, objects, batch, window_ }) => {
      const { nc } = window;
      const model = nc.model({
        tasks: nc.collection({ title: nc.string(), priority: nc.int() }),
      });
      const db = await nc.open({ model, lock, name: "measure-writes" });
      const project = await db.spaces.create();
      // The time spent writing, without the reports.
      let writing = 0;
      let start = performance.now();
      for (let i = 0; i < objects; i += batch) {
        await project.write((tx) => {
          for (let j = i; j < i + batch; j++) {
            tx.create(model.tasks, { title: `task ${j}`, priority: j % 5 });
          }
        });
        if ((i + batch) % window_ === 0) {
          const took = performance.now() - start;
          writing += took;
          await window.report(i + batch, took / (window_ / batch));
          start = performance.now();
        }
      }
      await db.close();
      return writing;
    },
    { lock, objects, batch, window_ },
  );
  console.log(`all ${objects} objects in ${(total / 1000).toFixed(0)} s`);
} finally {
  await browser.close();
  await server.close();
}
