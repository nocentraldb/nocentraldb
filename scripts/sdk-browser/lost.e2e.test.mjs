// NOC-445: a database whose files the browser dropped is reported, not
// reopened empty in silence. In a Chromium private window the origin's
// OPFS goes after about ten minutes with no database open, while
// IndexedDB, with the member's keys, stays (measured in Chromium 157;
// docs/browser-storage.md, manual check log). The next `nc.open` started
// an empty copy, and nothing told the app. Now it fails with
// `storageLost`, until the app opens with `ifStorageLost: "open-empty"`.
//
// The test drops OPFS itself, by removing every entry of its root, as
// Chromium does; waiting the ten minutes is what idle.e2e.test.mjs does
// for an open database. It runs in this engine's normal window, and in
// Chromium's private window, the only private window with OPFS (Firefox
// and WebKit refuse to open there, private.e2e.test.mjs).
//
// Not part of `pnpm gates`, which runs without browsers: the task
// `repo:sdk-browser` and the workflow browser-view.yml run it.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { build, LOCK, prebuilt } from "./build.mjs";
import { engine, launch, launchPrivate } from "./engine.mjs";
import { startServer } from "./server.mjs";

let server;
let normal;
let privately;

const chromium = engine === "chromium";

before(async () => {
  if (process.env.SDK_BROWSER_PREBUILT === "1") prebuilt();
  else build();
  server = await startServer({ port: 0 });
  normal = await launch();
  if (chromium) privately = await launchPrivate();
});

after(async () => {
  await normal?.close();
  await privately?.close();
  await server?.close();
});

const lock = JSON.parse(readFileSync(LOCK, "utf8"));

const MODEL = `nc.model({
  tasks: nc.collection({ title: nc.string(), priority: nc.int() }),
})`;

/** Opens, writes and closes `name`; drops OPFS; then opens it again three times. */
const lostAndAccepted = async (browser) => {
  const page = await (await browser.newContext()).newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error));
  await page.goto(`${server.url}/`);
  await page.waitForFunction(() => window.ready === true);
  const result = await page.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const outcome = (promise) =>
        promise.then(
          async (db) => {
            const spaces = (await db.spaces.list()).length;
            await db.close();
            return `opened, ${spaces} spaces`;
          },
          (e) => `${e.code}: ${e.message}`,
        );
      const first = await nc.open({ model, lock, name: "lost" });
      const space = await first.spaces.create();
      await space.create(model.tasks, { title: "before", priority: 1 });
      await first.close();
      const kept = await outcome(nc.open({ model, lock, name: "lost" }));
      // What Chromium does to a private window's OPFS after ten quiet minutes.
      // The closed worker gives its handles back a moment later, so a
      // removal may have to wait for it.
      const root = await navigator.storage.getDirectory();
      for (let tried = 0; ; tried++) {
        try {
          for await (const [entry] of root.entries()) {
            await root.removeEntry(entry, { recursive: true });
          }
          break;
        } catch (error) {
          if (tried === 50) throw error;
          await new Promise((resolve) => setTimeout(resolve, 100));
        }
      }
      const refused = await outcome(nc.open({ model, lock, name: "lost" }));
      const again = await outcome(
        nc.open({ model, lock, name: "lost", ifStorageLost: "refuse" }),
      );
      const other = await outcome(nc.open({ model, lock, name: "other" }));
      const accepted = await outcome(
        nc.open({ model, lock, name: "lost", ifStorageLost: "open-empty" }),
      );
      const after = await outcome(nc.open({ model, lock, name: "lost" }));
      return { kept, refused, again, other, accepted, after };
    },
    { lock, MODEL },
  );
  assert.equal(result.kept, "opened, 1 spaces");
  assert.match(result.refused, /^storageLost: .*lost.*"open-empty"/);
  assert.match(result.again, /^storageLost: /);
  assert.equal(result.other, "opened, 0 spaces");
  assert.equal(result.accepted, "opened, 0 spaces");
  assert.equal(result.after, "opened, 0 spaces");
  assert.deepEqual(errors, []);
};

test("in a normal window, a database whose OPFS files are gone is refused with storageLost until the app opens it empty", async () => {
  await lostAndAccepted(normal);
});

test("in a Chromium private window, a database whose OPFS was dropped is refused with storageLost until the app opens it empty", {
  skip: chromium
    ? false
    : "Firefox and WebKit have no OPFS in a private window (private.e2e.test.mjs)",
}, async () => {
  await lostAndAccepted(privately);
});
