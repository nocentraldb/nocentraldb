// A personal field after a close and a reopen in the browser, with no
// relay (NOC-547): the device keeps the person keys it holds, wrapped as
// the relay stores them, in its database on OPFS, so a value written
// before the close reads the same after the reopen, and `space.status`
// counts no missing person key. The Rust SDK's
// crates/nocentraldb/src/personal_reopen_tests.rs tests the same natively,
// and that the status counts a key the device does not hold.
//
// The model is erase/model.ts, with its lock beside it: one personal
// field, about the object itself.
//
// Not part of `pnpm gates`, which runs without browsers: the task
// `repo:sdk-browser` and the workflow browser-view.yml run it, in each
// engine.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { build, prebuilt, ROOT } from "./build.mjs";
import { launch } from "./engine.mjs";
import { startServer } from "./server.mjs";

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

const lock = JSON.parse(
  readFileSync(
    join(ROOT, "scripts", "sdk-browser", "erase", "model.lock.json"),
    "utf8",
  ),
);

/** erase/model.ts, as an app declares it. */
const MODEL = `nc.model({
  people: nc.collection({ name: nc.string().personal({ person: "self" }) }),
})`;

test("a personal field reads the same after a close and a reopen without a relay", async () => {
  const context = await browser.newContext();
  const opened = await context.newPage();
  const errors = [];
  opened.on("pageerror", (error) => errors.push(error));
  await opened.goto(`${server.url}/`);
  await opened.waitForFunction(() => window.ready === true);

  const read = await opened.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      let db = await nc.open({ model, lock, name: "personal" });
      let space = await db.spaces.create();
      const { ids } = await space.create(model.people, { name: "Anna" });
      const before = (await space.get(model.people, ids[0])).name;
      const spaceId = space.id;
      await db.close();

      db = await nc.open({ model, lock, name: "personal" });
      space = await db.spaces.get(spaceId);
      const after = (await space.get(model.people, ids[0])).name;
      const missing = space.status.current().missingPersonKeys;
      // A person drawn after the reopen is kept as well.
      const bert = (await space.create(model.people, { name: "Bert" })).ids[0];
      await db.close();

      db = await nc.open({ model, lock, name: "personal" });
      space = await db.spaces.get(spaceId);
      const again = (await space.get(model.people, ids[0])).name;
      const bertAgain = (await space.get(model.people, bert)).name;
      const missingAgain = space.status.current().missingPersonKeys;
      await db.close();
      return { before, after, missing, again, bertAgain, missingAgain };
    },
    { lock, MODEL },
  );

  assert.deepEqual(read, {
    before: "Anna",
    after: "Anna",
    missing: 0,
    again: "Anna",
    bertAgain: "Bert",
    missingAgain: 0,
  });
  assert.deepEqual(errors, []);
});
