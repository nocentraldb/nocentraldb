// End to end (NOC-439): a write of one object costs about the same at
// 20,000 objects as at 2,000, in a normal window. After every write the SDK
// reads the status of each space a handle follows and the durability
// report, and both read their log by a pass over it. In Chromium, a
// one-object write cost 6 ms at 2,000 objects and 13 ms at 20,000, and
// A consumer using NoCentralDB measured about 60 ms at 60,000 on 0.1.0-beta.1. The space
// status and the durability report now keep what they read, and this test
// keeps the cost flat.
//
// It writes 20,000 objects one write each, about two minutes in Chromium,
// and compares the mean cost of the writes from 18,000 to 20,000 with that
// of the writes from 2,000 to 4,000, after the first writes have warmed
// the core up. A pass over the log doubled it; the test allows half again.
// The window is a normal one on a fresh profile (engine.mjs): an
// off-the-record context keeps OPFS in memory and is about 13 times
// slower.
//
// Not part of `pnpm gates`, which runs without browsers: the task
// `repo:sdk-browser-long` and the long job of browser-view.yml run it
// (NOC-448).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { build, LOCK, prebuilt } from "../build.mjs";
import { launch } from "../engine.mjs";
import { startServer } from "../server.mjs";

let server;
let browser;

before(async () => {
  if (process.env.SDK_BROWSER_PREBUILT === "1") prebuilt();
  else build();
  server = await startServer({ port: 0 });
  browser = await launch();
});

after(async () => {
  await browser?.close();
  await server?.close();
});

const lock = JSON.parse(readFileSync(LOCK, "utf8"));

const MODEL = `nc.model({
  tasks: nc.collection({ title: nc.string(), priority: nc.int() }),
})`;

/** The objects written, one write each. */
const OBJECTS = 20_000;
/** The writes each mean is taken over. */
const WINDOW = 2_000;
/** How many times the late writes' mean may be the early writes'. */
const FACTOR = 1.5;

test("a one-object write costs about the same at 20,000 objects as at 2,000", async () => {
  const opened = await (await browser.newContext()).newPage();
  const errors = [];
  opened.on("pageerror", (error) => errors.push(error));
  await opened.goto(`${server.url}/`);
  await opened.waitForFunction(() => window.ready === true);

  const { early, late } = await opened.evaluate(
    async ({ lock, MODEL, OBJECTS, WINDOW }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "write-cost" });
      const project = await db.spaces.create();
      const means = [];
      let start = performance.now();
      for (let i = 1; i <= OBJECTS; i++) {
        await project.write((tx) => {
          tx.create(model.tasks, { title: `task ${i}`, priority: i % 5 });
        });
        if (i % WINDOW === 0) {
          const now = performance.now();
          means.push((now - start) / WINDOW);
          start = now;
        }
      }
      await db.close();
      return { early: means[1], late: means[means.length - 1] };
    },
    { lock, MODEL, OBJECTS, WINDOW },
  );

  assert.ok(
    late <= early * FACTOR,
    `a write took ${late.toFixed(2)} ms at ${OBJECTS} objects and ${early.toFixed(2)} ms at ${2 * WINDOW}`,
  );
  assert.deepEqual(errors, []);
});
