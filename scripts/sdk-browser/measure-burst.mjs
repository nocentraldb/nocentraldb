// Measures the memory of a burst of writes in a browser tab that syncs
// (NOC-519): the TypeScript SDK in a normal window
// on a fresh profile (engine.mjs) writes `--records` tasks of the bulk
// import measurement into one space, `--batch` to a write, online with the
// local relay, and waits until the relay holds them all. It prints the
// WebAssembly memory of the tab's core once the relay holds the burst,
// which never shrinks and so is its peak, and the peak resident memory of
// the tab's renderer process. To read the core's memory, the page is served
// a copy of the package whose worker.js keeps it, as
// scripts/memory-budget/browser.test.mjs does. The renderer peak is read
// from /proc, so this runs on Linux only.
//
//   node scripts/sdk-browser/build.mjs
//   node scripts/sdk-browser/measure-burst.mjs --records 300000 --batch 500
//
// SDK_BROWSER_ENGINE names the engine, as for the e2e files. Not a test.
import {
  cpSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { parseArgs } from "node:util";
import { DIST, LOCK } from "./build.mjs";
import { engine, launch } from "./engine.mjs";
import { startRelay } from "./relay.mjs";
import { startServer } from "./server.mjs";

const { values } = parseArgs({
  options: {
    records: { type: "string", default: "300000" },
    batch: { type: "string", default: "500" },
  },
});
const records = Number(values.records);
const batch = Number(values.batch);
for (const [name, n] of [
  ["--records", records],
  ["--batch", batch],
]) {
  if (!Number.isInteger(n) || n <= 0) {
    throw new Error(`${name} takes a positive integer`);
  }
}

/** Keeps the WebAssembly memory of the module the SDK's worker instantiates as `self.coreMemory`. */
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
const dist = mkdtempSync(join(tmpdir(), "measure-burst-sdk-"));
cpSync(DIST, dist, { recursive: true });
const workerFile = join(dist, "worker.js");
writeFileSync(workerFile, PROBE + readFileSync(workerFile, "utf8"));
const server = await startServer({ port: 0, dist });
const browser = await launch();
let relay;
try {
  const page = await (await browser.newContext()).newPage();
  await page.goto(`${server.url}/`);
  await page.waitForFunction(() => window.ready === true);
  const invitation = await page.evaluate(
    async ({ MODEL, lock }) => {
      window.model = new Function("nc", `return ${MODEL}`)(window.nc);
      const db = await window.nc.open({
        model: window.model,
        lock,
        name: "burst",
      });
      const invitation = String(await db.invitation());
      await db.close();
      return invitation;
    },
    { MODEL, lock },
  );
  // The relay lets the tab's account create spaces.
  relay = await startRelay(["--allow-invitation", invitation]);
  const outcome = await page.evaluate(
    async ({ lock, url, records, batch }) => {
      const { nc, model } = window;
      const until = async (what, check) => {
        const start = Date.now();
        while (!check()) {
          if (Date.now() - start > 30 * 60_000)
            throw new Error(`never: ${what}`);
          await new Promise((resolve) => setTimeout(resolve, 200));
        }
      };
      const db = await nc.open({ model, lock, name: "burst", relay: url });
      const space = await db.spaces.create();
      await until(
        "the relay holds the space",
        () => db.sync.current().unconfirmed === 0,
      );
      const start = performance.now();
      // The records of examples/bulk_import of the crate nocentraldb.
      for (let first = 0; first < records; first += batch) {
        await space.write((tx) => {
          for (let i = first; i < Math.min(first + batch, records); i++) {
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
      const written = performance.now() - start;
      await until(
        "the relay holds the burst",
        () => db.sync.current().unconfirmed === 0,
      );
      return {
        written,
        held: performance.now() - start,
        refused: db.sync.current().refused,
      };
    },
    { lock, url: relay.url, records, batch },
  );
  const worker = page.workers().find((w) => w.url().endsWith("/worker.js"));
  const core = await worker.evaluate(() => self.coreMemory?.buffer.byteLength);
  console.log(
    `${engine}, ${records} records, ${batch} to a write, with a relay`,
  );
  console.log(
    `written in:              ${(outcome.written / 1000).toFixed(1)} s`,
  );
  console.log(`held by the relay after: ${(outcome.held / 1000).toFixed(1)} s`);
  console.log(`refused by the relay:    ${outcome.refused}`);
  console.log(
    `core WebAssembly memory: ${(core / (1024 * 1024)).toFixed(0)} MiB`,
  );
  console.log(`renderer peak:           ${rendererPeak().toFixed(0)} MB`);
} finally {
  relay?.stop();
  await browser.close();
  await server.close();
  rmSync(dist, { recursive: true, force: true });
}
