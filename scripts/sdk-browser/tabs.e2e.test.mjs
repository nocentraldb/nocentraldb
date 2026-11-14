// NOC-177: one tab owns the database and the others reach it (RFC 0006,
// "The browser": "One tab owns the database"; RFC 0009, "Open and close").
// Two or three pages of one browser context share one origin, so one OPFS
// pool: the first to open owns the database through a Web Lock, the
// others send their calls to it over a BroadcastChannel, and when the
// owner closes, the next tab takes the database over.
//
// Not part of `pnpm gates`, which runs without browsers: the task
// `repo:sdk-browser` and the workflow browser-view.yml run it.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { chromium } from "playwright";
import { build, LOCK, prebuilt } from "./build.mjs";
import { startServer } from "./server.mjs";

let server;
let browser;

before(async () => {
  if (process.env.SDK_BROWSER_PREBUILT === "1") prebuilt();
  else build();
  server = await startServer({ port: 0 });
  browser = await chromium.launch();
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

/** A page of `context` with the package loaded and the database `name` opened as `window.db`. */
const tab = async (context, name = "tabs") => {
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error));
  await page.goto(`${server.url}/`);
  await page.waitForFunction(() => window.ready === true);
  await page.evaluate(
    async ({ lock, MODEL, name }) => {
      const { nc } = window;
      window.model = new Function("nc", `return ${MODEL}`)(nc);
      window.db = await nc.open({ model: window.model, lock, name });
    },
    { lock, MODEL, name },
  );
  return { page, errors };
};

/** Creates a space and a task titled `title` in it; answers the space id. */
const createTask = (page, title, space) =>
  page.evaluate(
    async ({ title, space }) => {
      const project = space
        ? await window.db.spaces.get(space)
        : await window.db.spaces.create();
      await project.create(window.model.tasks, {
        title,
        status: "open",
        priority: 1,
      });
      return project.id;
    },
    { title, space },
  );

/** The titles of the open tasks of every space, as a query reads them. */
const titles = (page) =>
  page.evaluate(async () => {
    const page = await window.db.find(
      window.model.tasks.indexes.byStatusPriority.equal("open"),
    );
    return page.items.map((task) => task.title).sort();
  });

/** Waits until `check` holds in `page`, polling. */
const eventually = async (check, what) => {
  const start = Date.now();
  for (;;) {
    const got = await check();
    if (got === true) return;
    if (Date.now() - start > 15000) assert.fail(`${what}: ${got}`);
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
};

test("two tabs of one origin use one database: both write and read", async () => {
  const context = await browser.newContext();
  const first = await tab(context);
  const second = await tab(context);
  const space = await createTask(first.page, "from the first tab");
  await createTask(second.page, "from the second tab", space);
  const expected = ["from the first tab", "from the second tab"];
  assert.deepEqual(await titles(first.page), expected);
  assert.deepEqual(await titles(second.page), expected);
  assert.deepEqual(first.errors, []);
  assert.deepEqual(second.errors, []);
  await context.close();
});

test("a subscription in one tab fires after a write in the other", async () => {
  const context = await browser.newContext();
  const first = await tab(context);
  const second = await tab(context);
  for (const { page } of [first, second]) {
    await page.evaluate(() => {
      window.seen = [];
      window.changes = [];
      window.db.subscribe(
        window.model.tasks.indexes.byStatusPriority.equal("open"),
        (page) => window.seen.push(page.items.map((task) => task.title)),
      );
      window.db.changes.subscribe((change) =>
        window.changes.push(change.origin),
      );
    });
  }
  const space = await createTask(first.page, "a");
  await createTask(second.page, "b", space);
  for (const { page } of [first, second]) {
    await eventually(
      () =>
        page.evaluate(() =>
          window.seen.some((titles) => titles.length === 2)
            ? true
            : JSON.stringify(window.seen),
        ),
      "the subscription saw both writes",
    );
    // Both writes reached each tab's change listeners.
    await eventually(
      () =>
        page.evaluate(() =>
          window.changes.length >= 2 ? true : JSON.stringify(window.changes),
        ),
      "the change listeners saw both writes",
    );
  }
  await context.close();
});

test("closing the owning tab hands the database to another without losing a write", async () => {
  const context = await browser.newContext();
  const first = await tab(context);
  const second = await tab(context);
  const space = await createTask(first.page, "written by the owner");
  await createTask(second.page, "written through the owner", space);
  await first.page.close();
  await eventually(async () => {
    const got = await titles(second.page).catch((error) => String(error));
    return Array.isArray(got) && got.length === 2 ? true : JSON.stringify(got);
  }, "the second tab reads both writes after the owner closed");
  await createTask(second.page, "written by the new owner", space);
  assert.deepEqual(await titles(second.page), [
    "written by the new owner",
    "written by the owner",
    "written through the owner",
  ]);
  assert.deepEqual(second.errors, []);
  await context.close();
});

test("db.close() in the owning tab hands the database over, and a third tab follows the new owner", async () => {
  const context = await browser.newContext();
  const first = await tab(context);
  const second = await tab(context);
  const third = await tab(context);
  const space = await createTask(first.page, "one");
  await first.page.evaluate(() => window.db.close());
  await createTask(second.page, "two", space);
  await createTask(third.page, "three", space);
  for (const { page } of [second, third]) {
    assert.deepEqual(await titles(page), ["one", "three", "two"]);
  }
  const closed = await first.page.evaluate(() =>
    window.db.spaces.list().then(
      () => "answered",
      (error) => error.code,
    ),
  );
  assert.equal(closed, "closed");
  await context.close();
});

test("a subscription keeps firing in a tab whose owner went away", async () => {
  const context = await browser.newContext();
  const first = await tab(context);
  const second = await tab(context);
  const space = await createTask(first.page, "before");
  await second.page.evaluate(() => {
    window.seen = [];
    window.db.subscribe(
      window.model.tasks.indexes.byStatusPriority.equal("open"),
      (page) => window.seen.push(page.items.map((task) => task.title).sort()),
    );
  });
  await first.page.close();
  await createTask(second.page, "after", space);
  await eventually(
    () =>
      second.page.evaluate(() =>
        window.seen.some((titles) => titles.join() === "after,before")
          ? true
          : JSON.stringify(window.seen),
      ),
    "the subscription saw the write after the handover",
  );
  await context.close();
});

test("a write in flight while its owner goes away runs at most once", async () => {
  const context = await browser.newContext();
  const first = await tab(context);
  const second = await tab(context);
  const space = await createTask(first.page, "before");
  // The second tab starts a write through the owner and does not wait;
  // the owner's tab closes at once.
  await second.page.evaluate((space) => {
    window.inFlight = window.db.spaces
      .get(space)
      .then((project) =>
        project.create(window.model.tasks, {
          title: "in flight",
          status: "open",
          priority: 1,
        }),
      )
      .then(
        () => "written",
        (error) => error.code,
      );
  }, space);
  await first.page.close();
  const outcome = await second.page.evaluate(() => window.inFlight);
  assert.ok(
    ["written", "storageFailed"].includes(outcome),
    `the write ended with ${outcome}`,
  );
  const got = await titles(second.page);
  const count = got.filter((title) => title === "in flight").length;
  assert.ok(count <= 1, `the write ran ${count} times`);
  if (outcome === "written") assert.equal(count, 1);
  await context.close();
});
