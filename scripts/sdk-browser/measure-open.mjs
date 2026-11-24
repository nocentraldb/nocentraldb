// Measures what an open space keeps in a browser tab's core (NOC-452), for
// docs/bulk-import.md: the TypeScript SDK in a normal window on a fresh
// profile (engine.mjs), with no relay, writes `--records` tasks of the bulk
// import measurement into one space, `--batch` to a write, and closes the
// database. The tab then loads the page again, which starts a new worker
// and a new core, opens the database, and writes one more task. The
// WebAssembly memory of that core, which never shrinks, is then the most it
// held while it opened the database and wrote. A database with one space
// and one task, opened the same way, gives the memory the core takes
// without the log.
//
// As scripts/memory-budget/browser.test.mjs does, it serves a copy of the
// package whose worker.js keeps the core's WebAssembly memory where
// Playwright can read it; the package itself is unchanged.
//
//   node scripts/sdk-browser/build.mjs
//   node scripts/sdk-browser/measure-open.mjs --records 81000 --batch 500
//
// SDK_BROWSER_ENGINE names the engine, as for the e2e files. Not a test.
import {
  cpSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { parseArgs } from "node:util";
import { DIST, LOCK } from "./build.mjs";
import { engine, launch } from "./engine.mjs";
import { startServer } from "./server.mjs";

const { values } = parseArgs({
  options: {
    records: { type: "string", default: "81000" },
    batch: { type: "string", default: "500" },
  },
});
const records = Number(values.records);
const batch = Number(values.batch);
for (const [name, value] of [
  ["--records", records],
  ["--batch", batch],
]) {
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`${name} takes a positive integer`);
  }
}

/** Prepended to worker.js: keeps the core's WebAssembly memory as `self.coreMemory`. */
const PROBE = `{
  const keep = (result) => {
    const instance = result instanceof WebAssembly.Instance ? result : result.instance;
    self.coreMemory = instance.exports.memory;
    return result;
  };
  const instantiate = WebAssembly.instantiate;
  const streaming = WebAssembly.instantiateStreaming;
  WebAssembly.instantiate = (...args) => instantiate.apply(WebAssembly, args).then(keep);
  WebAssembly.instantiateStreaming = (...args) => streaming.apply(WebAssembly, args).then(keep);
}
`;

/** The test lock's tasks, as an app declares them. */
const MODEL = `nc.model({
  tasks: nc.collection(
    {
      title: nc.string(),
      status: nc.oneOf(["open", "done"]),
      priority: nc.int(),
      estimate: nc.float(),
      urgent: nc.boolean(),
      tags: nc.set(nc.string()),
    },
    {
      indexes: {
        byPriority: nc.index(["priority"]),
        byStatusPriority: nc.index(["status", "priority"], { equal: 1 }),
        open: nc.index(["status", "priority"], { equal: 1, scope: "space" }),
      },
    },
  ),
})`;

const lock = JSON.parse(readFileSync(LOCK, "utf8"));
const dist = mkdtempSync(join(tmpdir(), "measure-open-sdk-"));
cpSync(DIST, dist, { recursive: true });
const worker = join(dist, "worker.js");
writeFileSync(worker, PROBE + readFileSync(worker, "utf8"));
const server = await startServer({ port: 0, dist });
const browser = await launch();

/** Loads the page in `page` and gives it the model. */
const load = async (page) => {
  await page.goto(`${server.url}/`);
  await page.waitForFunction(() => window.ready === true);
  await page.evaluate((MODEL) => {
    window.model = new Function("nc", `return ${MODEL}`)(window.nc);
  }, MODEL);
};

/** Writes `count` tasks into a new space of the database `name`, `batch` to a write, and closes it. */
const fill = (page, name, count) =>
  page.evaluate(
    async ({ lock, name, count, batch }) => {
      const { nc, model } = window;
      const db = await nc.open({ model, lock, name });
      const space = await db.spaces.create();
      for (let start = 0; start < count; start += batch) {
        await space.write((tx) => {
          for (let i = start; i < Math.min(start + batch, count); i++) {
            // The records of examples/bulk_import of the crate nocentraldb.
            tx.create(model.tasks, {
              title: `Imported task ${String(i).padStart(6, "0")}: follow up on the old record`,
              status: i % 3 === 0 ? "done" : "open",
              priority: i % 5,
              estimate: 0.5 * (i % 8),
              urgent: i % 10 === 0,
              tags: nc.add("imported", i % 2 === 0 ? "even" : "odd"),
            });
          }
        });
      }
      await db.close();
      return space.id;
    },
    { lock, name, count, batch },
  );

/** Opens the database `name` again in a fresh core, writes one task into `space`, and gives the core's WebAssembly memory in bytes. */
const reopen = async (page, name, space) => {
  await load(page);
  await page.evaluate(
    async ({ lock, name, space }) => {
      const { nc, model } = window;
      const db = await nc.open({ model, lock, name });
      const held = await db.spaces.get(space);
      await held.write((tx) => {
        tx.create(model.tasks, {
          title: "One more",
          status: "open",
          priority: 1,
          estimate: 1,
          urgent: false,
          tags: nc.add("after"),
        });
      });
    },
    { lock, name, space },
  );
  const core = page.workers().find((w) => w.url().endsWith("/worker.js"));
  return core.evaluate(() => self.coreMemory.buffer.byteLength);
};

const mib = (bytes) => `${(bytes / (1024 * 1024)).toFixed(0)} MiB`;

try {
  const page = await (await browser.newContext()).newPage();
  await load(page);
  const small = await fill(page, "one-task", 1);
  const start = Date.now();
  const space = await fill(page, "measured", records);
  const written = (Date.now() - start) / 1000;
  const empty = await reopen(page, "one-task", small);
  const full = await reopen(page, "measured", space);
  console.log(`${engine}, ${records} records, ${batch} to a write`);
  console.log(`written in:                          ${written.toFixed(0)} s`);
  console.log(`core memory, one task, reopened:     ${mib(empty)}`);
  console.log(`core memory, all records, reopened:  ${mib(full)}`);
} finally {
  await browser.close();
  await server.close();
  rmSync(dist, { recursive: true, force: true });
}
