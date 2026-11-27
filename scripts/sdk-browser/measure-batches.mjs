// Measures what writes of many objects cost in a browser (NOC-542): the
// TypeScript SDK in a normal window on a fresh profile (engine.mjs), with no
// relay, writes `--objects` tasks in writes of each size `--batches` names,
// once into a new space of a new database (small) and once into a space of
// a new database that already holds `--large` tasks (large), and prints the
// mean time of a write and the time of all of them. The tasks are those of
// measure-open.mjs, with its indexes, so the view does the work an app's
// would. Every measurement runs in a database of its own, so a small space
// never shares the view's tables with a large one.
//
//   node scripts/sdk-browser/build.mjs
//   node scripts/sdk-browser/measure-batches.mjs --batches 100,1000 \
//     --objects 10000 --large 100000
//
// `--profile <dir>` writes a Chromium CPU profile of each measured run into
// <dir>, one .cpuprofile per worker, which Chrome's DevTools open
// (Performance panel, "Load profile"). `--runs` repeats every measurement
// and prints each run and their mean.
//
// The script uses only what the SDK had at 0.1.0-beta.2, so it measures that
// tag as well: copy it into a checkout of the tag and run it there.
// SDK_BROWSER_ENGINE names the engine, as for the e2e files. Not a test.
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";
import { LOCK } from "./build.mjs";
import { engine, launch } from "./engine.mjs";
import { startServer } from "./server.mjs";

const { values } = parseArgs({
  options: {
    batches: { type: "string", default: "100,1000" },
    objects: { type: "string", default: "10000" },
    large: { type: "string", default: "100000" },
    runs: { type: "string", default: "1" },
    profile: { type: "string" },
  },
});
const batches = values.batches.split(",").map(Number);
const objects = Number(values.objects);
const large = Number(values.large);
const runs = Number(values.runs);
if (
  ![...batches, objects, runs].every((n) => Number.isInteger(n) && n > 0) ||
  !(Number.isInteger(large) && large >= 0)
) {
  throw new Error(
    "--batches, --objects and --runs take positive integers, --large a count",
  );
}
if (!batches.every((batch) => objects % batch === 0)) {
  throw new Error("--objects must be a multiple of every size in --batches");
}

/** The test lock's tasks, as measure-open.mjs declares them. */
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
const server = await startServer({ port: 0 });
const browser = await launch();

/** Loads the page and gives it the model and a function that writes tasks. */
const load = async (page) => {
  await page.goto(`${server.url}/`);
  await page.waitForFunction(() => window.ready === true);
  await page.evaluate((MODEL) => {
    const { nc } = window;
    window.model = new Function("nc", `return ${MODEL}`)(nc);
    /** Writes tasks `from` to `to` into `space`, `batch` to a write; gives each write's milliseconds. */
    window.fill = async (space, from, to, batch) => {
      const times = [];
      for (let start = from; start < to; start += batch) {
        const begin = performance.now();
        await space.write((tx) => {
          for (let i = start; i < Math.min(start + batch, to); i++) {
            // The records of examples/bulk_import of the crate nocentraldb.
            tx.create(window.model.tasks, {
              title: `Imported task ${String(i).padStart(6, "0")}: follow up on the old record`,
              status: i % 3 === 0 ? "done" : "open",
              priority: i % 5,
              estimate: 0.5 * (i % 8),
              urgent: i % 10 === 0,
              tags: nc.add("imported", i % 2 === 0 ? "even" : "odd"),
            });
          }
        });
        times.push(performance.now() - begin);
      }
      return times;
    };
  }, MODEL);
};

/**
 * Starts the CPU profiler of the page's workers, where the core runs. A
 * worker is no target Playwright opens a CDP session on, so the page's
 * session attaches to it and passes the profiler's messages through.
 */
const startProfiles = async (page) => {
  const session = await page.context().newCDPSession(page);
  const workers = [];
  const pending = new Map();
  let next = 1;
  session.on("Target.attachedToTarget", ({ sessionId, targetInfo }) => {
    if (targetInfo.type === "worker") workers.push(sessionId);
  });
  session.on("Target.receivedMessageFromTarget", ({ message }) => {
    const reply = JSON.parse(message);
    pending.get(reply.id)?.(reply.result);
    pending.delete(reply.id);
  });
  const send = (sessionId, method, params = {}) =>
    new Promise((resolve) => {
      const id = next++;
      pending.set(id, resolve);
      session.send("Target.sendMessageToTarget", {
        sessionId,
        message: JSON.stringify({ id, method, params }),
      });
    });
  await session.send("Target.setAutoAttach", {
    autoAttach: true,
    waitForDebuggerOnStart: false,
    flatten: false,
  });
  for (const worker of workers) {
    await send(worker, "Profiler.enable");
    await send(worker, "Profiler.setSamplingInterval", { interval: 100 });
    await send(worker, "Profiler.start");
  }
  return { session, workers, send };
};

/** Stops the profilers and writes their profiles as `<name>-<n>.cpuprofile`. */
const stopProfiles = async ({ session, workers, send }, name) => {
  mkdirSync(values.profile, { recursive: true });
  for (const [n, worker] of workers.entries()) {
    const { profile } = await send(worker, "Profiler.stop");
    writeFileSync(
      join(values.profile, `${name}-${n}.cpuprofile`),
      JSON.stringify(profile),
    );
  }
  await session.detach();
};

/**
 * One measurement: a new database `name`, a space that first gets `before`
 * tasks in writes of 1,000, then `objects` tasks in writes of `batch`, which
 * are timed. Gives each timed write's milliseconds.
 */
const measure = async (name, before, batch) => {
  const page = await (await browser.newContext()).newPage();
  try {
    await load(page);
    await page.evaluate(
      async ({ lock, name, before }) => {
        const db = await window.nc.open({ model: window.model, lock, name });
        window.db = db;
        window.space = await db.spaces.create();
        await window.fill(window.space, 0, before, 1000);
      },
      { lock, name, before },
    );
    const profiles =
      values.profile && engine === "chromium"
        ? await startProfiles(page)
        : undefined;
    const times = await page.evaluate(
      ({ before, objects, batch }) =>
        window.fill(window.space, before, before + objects, batch),
      { before, objects, batch },
    );
    if (profiles) await stopProfiles(profiles, name);
    await page.evaluate(() => window.db.close());
    return times;
  } finally {
    await page.context().close();
  }
};

const sum = (numbers) => numbers.reduce((a, b) => a + b, 0);
const ms = (n) => `${n.toFixed(1)} ms`.padStart(10);
const s = (n) => `${(n / 1000).toFixed(2)} s`.padStart(8);

try {
  console.log(`${engine}, ${objects} objects in each measurement`);
  for (const batch of batches) {
    for (const [label, before] of [
      ["small", 0],
      ["large", large],
    ]) {
      const totals = [];
      const means = [];
      for (let run = 1; run <= runs; run++) {
        const times = await measure(
          `batches-${batch}-${label}-${run}`,
          before,
          batch,
        );
        totals.push(sum(times));
        means.push(sum(times) / times.length);
        const space = `${label} space (${before} before)`.padEnd(28);
        console.log(
          `writes of ${String(batch).padStart(5)}, ${space} run ${run}: ${ms(means.at(-1))} a write, ${s(totals.at(-1))} in all`,
        );
      }
      if (runs > 1) {
        const space = `${label} space (${before} before)`.padEnd(28);
        console.log(
          `writes of ${String(batch).padStart(5)}, ${space} mean:  ${ms(sum(means) / runs)} a write, ${s(sum(totals) / runs)} in all`,
        );
      }
    }
  }
} finally {
  await browser.close();
  await server.close();
}
