// NOC-332: a tab whose owner is frozen, as a phone freezes a hidden tab,
// or frozen and then closed. The owner's requirement: a write the app made
// is neither held until the frozen tab wakes nor left "may or may not have
// run" (RFC 0006, "The browser": "One tab owns the database").
//
// iOS first hides a tab when the person switches away, then freezes it.
// The test does the same: it hides the owning page (its visibilityState
// reads "hidden" and a visibilitychange event fires, as Playwright cannot
// hide a page itself), then freezes it.
//
// How the freeze is emulated: what matters is that the owning page stops
// handling messages while it keeps the Web Lock, so the others' calls wait
// in its queue. A busy loop on the owning page's main thread does that.
// It is not all of iOS's freeze: the owner's dedicated worker keeps
// running, its timers are not suspended, and no \`freeze\` or
// \`visibilitychange\` event fires. Neither Playwright's WebKit nor headless
// Chromium can freeze a page: Chromium's Page.setWebLifecycleState left a
// probe page's timers running and fired no \`freeze\` event.
//
// Not part of `pnpm gates`, which runs without browsers: the task
// `repo:sdk-browser` and the workflow browser-view.yml run it.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { build, LOCK, prebuilt } from "./build.mjs";
import { engine, launch } from "./engine.mjs";
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

/** Hides `page` as switching away from it does: its visibilityState reads "hidden", and visibilitychange fires. */
const hide = (page) =>
  page.evaluate(() => {
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      get: () => "hidden",
    });
    document.dispatchEvent(new Event("visibilitychange"));
  });

/** How long a frozen owner stays frozen, in milliseconds. */
const FROZEN = 20000;

/**
 * Whether the engine lets the visible tab take over while the owner is
 * still frozen. WebKit does not (NOC-352; docs/getting-started-browser.md,
 * "Safari"): a worker gives its OPFS handles back only while its page
 * runs, so the visible tab opens the database once the frozen one runs
 * again. Until then the write waits; it never fails.
 */
const takesOverWhileFrozen = engine !== "webkit";

/** Freezes `page`'s main thread for `ms`, as a phone freezes a hidden tab: it handles no message, and keeps the Web Lock. Does not wait. */
const freeze = (page, ms = FROZEN) => {
  page
    .evaluate((ms) => {
      const end = Date.now() + ms;
      while (Date.now() < end) {
        // Frozen.
      }
    }, ms)
    .catch(() => {
      // The page closed while frozen.
    });
};

/** Gets the space handle in `page` while the owner still answers, so the write itself is what waits. */
const handle = (page, space) =>
  page.evaluate(async (space) => {
    window.project = await window.db.spaces.get(space);
  }, space);

/** Starts a write of a task titled `title` in `page`, without waiting; `window.outcome` settles as "written" or the error's code. */
const startWrite = (page, title) =>
  page.evaluate((title) => {
    window.outcome = window.project
      .create(window.model.tasks, { title, status: "open", priority: 1 })
      .then(
        () => "written",
        (error) => error.code,
      );
  }, title);

/**
 * What `window.outcome` settled as within `ms`, or "pending". The time is
 * Node's, not a timer of the page: in WebKit the frozen owner can hold the
 * other pages' timers too, and a timer that fires only after the freeze
 * would let a write that waited for the owner pass (NOC-352).
 */
const outcomeWithin = async (page, ms) => {
  let timer;
  const late = new Promise((resolve) => {
    timer = setTimeout(() => resolve("pending"), ms);
  });
  const settled = page.evaluate(() => window.outcome);
  // Once late, the page may close before it answers.
  settled.catch(() => {});
  try {
    return await Promise.race([settled, late]);
  } finally {
    clearTimeout(timer);
  }
};

/**
 * Asserts that the write `page` started at `started` (Node's clock) is
 * written within `ms` of the owner's freeze, or, in an engine that takes
 * over only once the owner runs again, that it waits without failing until
 * then and is written within `ms` after it.
 */
const writtenAfterFreeze = async (page, started, ms) => {
  if (takesOverWhileFrozen) {
    assert.equal(await outcomeWithin(page, ms), "written");
    return;
  }
  const thawed = started + FROZEN;
  const early = await outcomeWithin(page, Math.max(0, thawed - Date.now()));
  assert.ok(early === "written" || early === "pending", early);
  assert.equal(await outcomeWithin(page, ms), "written");
};

test("a write from a tab whose owner is frozen is written, and that tab sees it", async () => {
  const context = await browser.newContext();
  const owner = await tab(context);
  const writer = await tab(context);
  const space = await createTask(owner.page, "before");
  await handle(writer.page, space);
  await hide(owner.page);
  const started = Date.now();
  freeze(owner.page);
  await startWrite(writer.page, "while frozen");
  await writtenAfterFreeze(writer.page, started, 5000);
  await eventually(
    async () =>
      (await titles(writer.page)).includes("while frozen") ||
      (await titles(writer.page)).join(","),
    "the writing tab sees its own write",
  );
  await context.close();
});

test("a write from a tab whose owner froze and then closed is written exactly once", async () => {
  const context = await browser.newContext();
  const owner = await tab(context);
  const writer = await tab(context);
  const space = await createTask(owner.page, "before");
  await handle(writer.page, space);
  await hide(owner.page);
  const started = Date.now();
  freeze(owner.page);
  await startWrite(writer.page, "frozen, then closed");
  await new Promise((resolve) => setTimeout(resolve, 500));
  // In WebKit the page closes only once its freeze ends.
  const closing = owner.page.close();
  await writtenAfterFreeze(writer.page, started, 10000);
  await closing;
  const got = await titles(writer.page);
  assert.equal(
    got.filter((title) => title === "frozen, then closed").length,
    1,
    got.join(","),
  );
  await context.close();
});

/** Records in `page` each `waitingForTab` its durability reports, with the page's clock, and when the write it starts settles. */
const recordWaiting = (page) =>
  page.evaluate(() => {
    window.waits = [];
    window.db.durability.subscribe(({ waitingForTab }) => {
      const last = window.waits.at(-1);
      if (last?.waiting !== waitingForTab)
        window.waits.push({ waiting: waitingForTab, at: Date.now() });
    });
  });

/**
 * Hides `page` and freezes it for `ms` right after, with only microtasks
 * between, as a phone that freezes a tab at once: the hide handler gives
 * the database up and releases the tabs' lock, but the owner's worker has
 * no time to give the storage back before the page stops. With the two in
 * separate steps, WebKit's worker sometimes gave it back in between, and
 * nothing waited. Does not wait.
 */
const hideAndFreeze = (page, ms = FROZEN) => {
  page
    .evaluate(async (ms) => {
      Object.defineProperty(document, "visibilityState", {
        configurable: true,
        get: () => "hidden",
      });
      document.dispatchEvent(new Event("visibilitychange"));
      await new Promise((resolve) => setTimeout(resolve, 0));
      const end = Date.now() + ms;
      while (Date.now() < end) {
        // Frozen.
      }
    }, ms)
    .catch(() => {
      // The page closed while frozen.
    });
};

test("while a write waits for a frozen owner, the tab reports waitingForTab, and clears it once the write goes through (NOC-355)", async () => {
  const context = await browser.newContext();
  const owner = await tab(context);
  const writer = await tab(context);
  const space = await createTask(owner.page, "before");
  await handle(writer.page, space);
  await recordWaiting(writer.page);
  const started = Date.now();
  hideAndFreeze(owner.page);
  await startWrite(writer.page, "waits for the owner");
  await writtenAfterFreeze(writer.page, started, 10000);
  const waits = await writer.page.evaluate(() => window.waits);
  const states = waits.map(({ waiting }) => waiting);
  if (takesOverWhileFrozen) {
    // The hidden owner gave the storage back before it froze: nothing waits.
    assert.deepEqual(states, [false], JSON.stringify(waits));
  } else {
    assert.deepEqual(states, [false, true, false], JSON.stringify(waits));
    // It appeared while the owner was frozen, not only once it ran again.
    assert.ok(waits[1].at < started + FROZEN, JSON.stringify(waits));
    assert.ok(waits[2].at >= waits[1].at, JSON.stringify(waits));
  }
  assert.equal(
    await writer.page.evaluate(
      () => window.db.durability.current().waitingForTab,
    ),
    false,
  );
  await context.close();
});
