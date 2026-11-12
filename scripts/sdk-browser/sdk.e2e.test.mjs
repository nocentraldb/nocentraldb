// End to end (NOC-176 slice 1): the TypeScript SDK in Chromium through
// Playwright, as an app uses it. The page imports the package's build
// output (packages/sdk/dist/, built by `pnpm --filter @nocentraldb/sdk
// build`); `nc.open` starts the worker the package ships, which loads the
// core's WebAssembly module next to it with `new URL(..., import.meta.url)`
// and keeps the database on OPFS (RFC 0009, "Open and close"; Carsten,
// 2026-11-12, NOC-274).
//
// Not part of `pnpm gates`, which runs without browsers: the task
// `repo:sdk-browser` and the workflow browser-view.yml run it, the workflow
// in Playwright's image on the package built before (SDK_BROWSER_PREBUILT=1).
// A missing Chromium fails it: install it with
// `pnpm exec playwright install chromium`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { chromium } from "playwright";
import { build, LOCK } from "./build.mjs";
import { startServer } from "./server.mjs";

let server;
let browser;

before(async () => {
  if (process.env.SDK_BROWSER_PREBUILT !== "1") build();
  server = await startServer({ port: 0 });
  browser = await chromium.launch();
});

after(async () => {
  await browser?.close();
  await server?.close();
});

const lock = JSON.parse(readFileSync(LOCK, "utf8"));

/** A fresh page with the package loaded; a fresh context, so a fresh OPFS. */
const page = async (context) => {
  const opened = await (context ?? (await browser.newContext())).newPage();
  const errors = [];
  opened.on("pageerror", (error) => errors.push(error));
  await opened.goto(`${server.url}/`);
  await opened.waitForFunction(() => window.ready === true);
  return { page: opened, errors };
};

/** The page's model: the test lock's tasks, as the app declares them. */
const MODEL = `nc.model({
  tasks: nc.collection({
    title: nc.string(),
    priority: nc.int(),
    estimate: nc.float(),
    urgent: nc.boolean(),
    tags: nc.set(nc.string()),
  }),
})`;

test("opens, writes, reads, closes and reopens the database on OPFS", async () => {
  const context = await browser.newContext();
  const { page: first } = await page(context);
  const written = await first.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "e2e" });
      const project = await db.spaces.create();
      let id;
      const result = await project.write((tx) => {
        id = tx.create(model.tasks, {
          title: "Ship the SDK",
          priority: 2,
          estimate: 3,
          urgent: true,
          tags: nc.add("sdk", "browser"),
        });
      });
      const task = await project.get(model.tasks, id);
      await db.close();
      const closed = await project.get(model.tasks, id).catch((e) => e.code);
      return { space: project.id, id, result, task, closed };
    },
    { lock, MODEL },
  );
  assert.match(written.space, /^[0-9a-f]{32}$/);
  assert.match(written.id, /^[0-9a-f]{32}$/);
  assert.deepEqual(written.result.ids, [written.id]);
  assert.match(written.result.change, /^[0-9a-f]{64}$/);
  assert.equal(written.task.$id, written.id);
  assert.equal(written.task.$space, written.space);
  assert.equal(written.task.$deleted, false);
  assert.equal(written.task.title, "Ship the SDK");
  assert.equal(written.task.priority, 2);
  assert.equal(written.task.estimate, 3);
  assert.equal(written.task.urgent, true);
  assert.deepEqual([...written.task.tags].sort(), ["browser", "sdk"]);
  assert.equal(written.closed, "closed");

  // A new page of the same origin opens the same database again.
  const { page: second } = await page(context);
  const reread = await second.evaluate(
    async ({ lock, MODEL, space, id }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "e2e" });
      const spaces = await db.spaces.list();
      const project = await db.spaces.get(space);
      const task = await project.get(model.tasks, id);
      await db.close();
      return { spaces, title: task?.title };
    },
    { lock, MODEL, space: written.space, id: written.id },
  );
  assert.deepEqual(reread.spaces, [{ id: written.space, role: "admin" }]);
  assert.equal(reread.title, "Ship the SDK");
  await context.close();
});

test("an update, a delete and a restore change what a get reads", async () => {
  const { page: opened } = await page();
  const read = await opened.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock });
      const project = await db.spaces.create();
      const { ids } = await project.create(model.tasks, { title: "a" });
      const id = ids[0];
      await project.update(model.tasks, id, { title: "b" });
      const updated = (await project.get(model.tasks, id)).title;
      await project.delete(model.tasks, id);
      const deleted = (await project.get(model.tasks, id)).$deleted;
      await project.restore(model.tasks, id);
      const restored = (await project.get(model.tasks, id)).$deleted;
      const missing = await project.get(model.tasks, "00".repeat(16));
      await db.close();
      return { updated, deleted, restored, missing: missing === undefined };
    },
    { lock, MODEL },
  );
  assert.deepEqual(read, {
    updated: "b",
    deleted: true,
    restored: false,
    missing: true,
  });
});

test("errors are nc.Error with RFC 0009's codes", async () => {
  const { page: opened } = await page();
  const codes = await opened.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const badName = await nc
        .open({ model, lock, name: "Not OK" })
        .catch((e) => e);
      const db = await nc.open({ model, lock, name: "errors" });
      const project = await db.spaces.create();
      const notFound = await project
        .update(model.tasks, "00".repeat(16), { title: "x" })
        .catch((e) => e);
      const invalid = await project
        .create(model.tasks, { title: 3 })
        .catch((e) => e);
      await db.close();
      return [badName, notFound, invalid].map((e) => ({
        isError: e instanceof nc.Error,
        code: e.code,
        name: e.name,
      }));
    },
    { lock, MODEL },
  );
  assert.deepEqual(codes, [
    { isError: true, code: "invalidValue", name: "nc.Error" },
    { isError: true, code: "notFound", name: "nc.Error" },
    { isError: true, code: "invalidValue", name: "nc.Error" },
  ]);
});
