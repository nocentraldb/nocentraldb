// NOC-347: a private window without OPFS does not open the database.
// Firefox and WebKit have no OPFS in a private window. A copy there could
// live only in the memory of the tab that owns it, and it went with that
// tab when it closed or was hidden: the next owner opened an empty copy,
// and unconfirmed changes were lost silently. So the owner decided that
// `nc.open` refuses there with `storageUnavailable`, and the app tells
// the user (Carsten, 2026-11-16, NOC-347). Chromium has OPFS in a private
// window: it opens there, and its copy outlives the owning tab. The test
// runs in each engine's private window (engine.mjs, `launchPrivate`).
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

/** Whether this engine's private window has OPFS, so the database opens there. */
const opens = engine === "chromium";

/** A page of `context` that opened the database: answers the page and how the open ended, `"opened"` or the error's code. */
const tab = async (context) => {
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error));
  await page.goto(`${server.url}/`);
  await page.waitForFunction(() => window.ready === true);
  const open = await page.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      window.model = new Function("nc", `return ${MODEL}`)(nc);
      try {
        window.db = await nc.open({
          model: window.model,
          lock,
          name: "private",
        });
        return { outcome: "opened" };
      } catch (error) {
        return {
          outcome: error.code,
          name: error.name,
          message: error.message,
        };
      }
    },
    { lock, MODEL },
  );
  return { page, errors, open };
};

/** Creates a space with one task in it. */
const createSpace = (page) =>
  page.evaluate(async () => {
    const space = await window.db.spaces.create();
    await space.create(window.model.tasks, {
      title: "before the handover",
      status: "open",
      priority: 1,
    });
  });

/** The titles of the open tasks `page` reads, once a read answers. */
const titlesOf = async (page) => {
  const start = Date.now();
  for (;;) {
    const read = await page
      .evaluate(async () => {
        const found = await window.db.find(
          window.model.tasks.indexes.byStatusPriority.equal("open"),
        );
        return found.items.map((task) => task.title);
      })
      .catch((error) => error);
    if (!(read instanceof Error)) return read;
    if (Date.now() - start > 15000) assert.fail(`no read answered: ${read}`);
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
};

/** Hides `page` as switching away from it does (frozen.e2e.test.mjs does the same): its visibilityState reads "hidden", and visibilitychange fires. */
const hide = (page) =>
  page.evaluate(() => {
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      get: () => "hidden",
    });
    document.dispatchEvent(new Event("visibilitychange"));
  });

test("nc.open in a private window refuses with storageUnavailable where OPFS is missing, and opens where it is not", async () => {
  const context = await browser.newContext();
  const { page, errors, open } = await tab(context);
  if (opens) {
    assert.equal(open.outcome, "opened");
    const storage = await page.evaluate(
      () => window.db.durability.current().storage,
    );
    assert.equal(storage, "best-effort");
  } else {
    assert.equal(open.outcome, "storageUnavailable", JSON.stringify(open));
    assert.equal(open.name, "nc.Error");
    assert.match(open.message, /private window/);
  }
  assert.deepEqual(errors, []);
  await context.close();
});

test("a second tab of a private window holds no copy of its own where OPFS is missing", async () => {
  const context = await browser.newContext();
  const first = await tab(context);
  const second = await tab(context);
  if (opens) {
    assert.equal(second.open.outcome, "opened");
  } else {
    // Neither tab owns a database, so no tab can take one over empty.
    assert.equal(first.open.outcome, "storageUnavailable");
    assert.equal(second.open.outcome, "storageUnavailable");
  }
  await context.close();
});

test("in a private window that opens, the copy outlives its owning tab, closed or hidden", async () => {
  const context = await browser.newContext();
  const first = await tab(context);
  if (!opens) {
    assert.equal(first.open.outcome, "storageUnavailable");
    await context.close();
    return;
  }
  const second = await tab(context);
  const third = await tab(context);
  await createSpace(first.page);
  await hide(first.page);
  assert.deepEqual(await titlesOf(second.page), ["before the handover"]);
  await second.page.close();
  assert.deepEqual(await titlesOf(third.page), ["before the handover"]);
  assert.deepEqual(third.errors, []);
  await context.close();
});
