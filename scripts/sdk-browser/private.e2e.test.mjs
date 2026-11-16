// NOC-347: what a private window does with the database across an owner
// handover (NOC-177). Firefox and WebKit have no OPFS in a private window,
// so the owning tab's worker keeps the copy in memory (RFC 0006, "Private
// windows"). That memory goes with the worker: the tab that takes over
// opens an empty copy. Chromium has OPFS in a private window, so its copy
// outlives the owning tab. The test runs in each engine's private window
// (engine.mjs, `launchPrivate`) and asserts what that engine does.
//
// Not part of `pnpm gates`, which runs without browsers: the task
// `repo:sdk-browser` and the workflow browser-view.yml run it.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { build, LOCK, prebuilt } from "./build.mjs";
import { engine, launchPrivate } from "./engine.mjs";
import { startServer } from "./server.mjs";

let server;
let browser;

before(async () => {
  if (process.env.SDK_BROWSER_PREBUILT === "1") prebuilt();
  else build();
  server = await startServer({ port: 0 });
  browser = await launchPrivate();
});

after(async () => {
  await browser?.close();
  await server?.close();
});

const lock = JSON.parse(readFileSync(LOCK, "utf8"));

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

/** Whether this engine's private window has no OPFS, so the copy is in memory. */
const inMemory = engine !== "chromium";

/** A page of `context` with the database opened as `window.db`. */
const tab = async (context) => {
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error));
  await page.goto(`${server.url}/`);
  await page.waitForFunction(() => window.ready === true);
  await page.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      window.model = new Function("nc", `return ${MODEL}`)(nc);
      window.db = await nc.open({ model: window.model, lock, name: "private" });
    },
    { lock, MODEL },
  );
  return { page, errors };
};

/** Creates a space with one task in it; answers the space id. */
const createSpace = (page) =>
  page.evaluate(async () => {
    const space = await window.db.spaces.create();
    await space.create(window.model.tasks, {
      title: "before the handover",
      status: "open",
      priority: 1,
    });
    return space.id;
  });

/** What `page` reads of the database: its spaces, its open tasks, and its durability. */
const copyOf = (page) =>
  page.evaluate(async () => {
    const spaces = await window.db.spaces.list();
    const found = await window.db.find(
      window.model.tasks.indexes.byStatusPriority.equal("open"),
    );
    return {
      spaces: spaces.length,
      titles: found.items.map((task) => task.title),
      durability: window.db.durability.current(),
    };
  });

/** Waits until the tab of `page` owns the database again: a read answers. */
const settled = async (page) => {
  const start = Date.now();
  for (;;) {
    const read = await copyOf(page).catch((error) => error);
    if (!(read instanceof Error)) return read;
    if (Date.now() - start > 15000) assert.fail(`no read answered: ${read}`);
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
};

test("a private window's copy is temporary where OPFS is missing", async () => {
  const context = await browser.newContext();
  const { page, errors } = await tab(context);
  await createSpace(page);
  const copy = await copyOf(page);
  assert.equal(copy.durability.storage, inMemory ? "temporary" : "best-effort");
  assert.equal(copy.durability.unconfirmed > 0, true);
  assert.deepEqual(errors, []);
  await context.close();
});

test("closing the owning tab of a private window: the copy goes with it where it is in memory", async () => {
  const context = await browser.newContext();
  const first = await tab(context);
  const second = await tab(context);
  const space = await createSpace(first.page);
  assert.deepEqual((await copyOf(second.page)).titles, ["before the handover"]);
  await first.page.close();
  const after = await settled(second.page);
  if (inMemory) {
    // The new owner's worker opened a copy of its own, in its own memory.
    assert.equal(after.spaces, 0);
    assert.deepEqual(after.titles, []);
    const write = await second.page.evaluate(
      (space) =>
        window.db.spaces.get(space).then(
          () => "found",
          (error) => error.code,
        ),
      space,
    );
    assert.equal(write, "notFound");
  } else {
    assert.equal(after.spaces, 1);
    assert.deepEqual(after.titles, ["before the handover"]);
  }
  assert.deepEqual(second.errors, []);
  await context.close();
});

/** Hides `page` as switching away from it does (frozen.e2e.test.mjs does the same): its visibilityState reads "hidden", and visibilitychange fires. */
const hide = (page) =>
  page.evaluate(() => {
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      get: () => "hidden",
    });
    document.dispatchEvent(new Event("visibilitychange"));
  });

test("hiding the owning tab of a private window: the visible tab takes over, and the copy goes with the hidden one where it is in memory", async () => {
  const context = await browser.newContext();
  const first = await tab(context);
  const second = await tab(context);
  await createSpace(first.page);
  await hide(first.page);
  const after = await settled(second.page);
  // Both tabs are still open; only the owner changed (NOC-332).
  assert.equal(first.page.isClosed(), false);
  if (inMemory) {
    assert.equal(after.spaces, 0);
    assert.deepEqual(after.titles, []);
    assert.equal(after.durability.unconfirmed, 0);
  } else {
    assert.equal(after.spaces, 1);
    assert.deepEqual(after.titles, ["before the handover"]);
  }
  assert.deepEqual(first.errors, []);
  assert.deepEqual(second.errors, []);
  await context.close();
});
