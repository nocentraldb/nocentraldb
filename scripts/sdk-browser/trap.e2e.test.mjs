// End to end (NOC-499): what follows a real trap of the core in a browser,
// as NOC-426 decided it. The page loads a copy of the package whose core,
// built with the feature `test-trap` (trap.mjs), traps on a marker value
// while it answers a write, as a write too large for its memory traps
// while the core folds it. The trap does not rest on how much memory the
// core uses: a write large enough to make it run out of memory runs the
// page out of its own memory first on some machines (NOC-522), and one of
// 2 GiB got no answer for minutes in Chromium (NOC-523).
// coreGuard.test.ts holds the same with a fake core. Run as
// sdk.e2e.test.mjs is: by the task `repo:sdk-browser`, not by `pnpm gates`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { TEST_TRAP } from "../../packages/sdk/testTrap.mjs";
import { build, LOCK, prebuilt } from "./build.mjs";
import { launch } from "./engine.mjs";
import { startServer } from "./server.mjs";
import { buildTrap, prebuiltTrap, TRAP_DIST } from "./trap.mjs";

let server;
let browser;

before(async () => {
  if (process.env.SDK_BROWSER_PREBUILT === "1") {
    prebuilt();
    prebuiltTrap();
  } else {
    build();
    buildTrap();
  }
  server = await startServer({ port: 0, dist: TRAP_DIST });
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

// After the core trapped (NOC-426): it is broken for good, so the call it
// was answering fails with `storageFailed`, and the next call fails at once
// with `storageFailed` and a text that says to open the database again;
// `db.close()` succeeds, and the database opened again works and holds
// what was written before. Nothing reaches the page as an uncaught error.
test("after the core trapped, the next call says to open the database again, and it opens again with what was written before", async () => {
  const opened = await (await browser.newContext()).newPage();
  const errors = [];
  opened.on("pageerror", (error) => errors.push(error));
  await opened.goto(`${server.url}/`);
  await opened.waitForFunction(() => window.ready === true);

  const result = await opened.evaluate(
    async ({ lock, MODEL, TEST_TRAP }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const outcome = (promise) =>
        promise.then(
          () => "ok",
          (e) =>
            e instanceof nc.Error ? `${e.code}: ${e.message}` : String(e),
        );
      const db = await nc.open({ model, lock, name: "trapped" });
      const project = await db.spaces.create();
      const { ids } = await project.write((tx) => {
        tx.create(model.tasks, { title: "before", priority: 1 });
      });
      const written = await outcome(
        project.write((tx) => {
          tx.create(model.tasks, { title: TEST_TRAP, priority: 2 });
        }),
      );
      const later = await outcome(project.get(model.tasks, ids[0]));
      const closed = await outcome(db.close());
      const again = await nc.open({ model, lock, name: "trapped" });
      const [space] = await again.spaces.list();
      const read = await (await again.spaces.get(space.id)).get(
        model.tasks,
        ids[0],
      );
      await again.close();
      return { written, later, closed, title: read?.title };
    },
    { lock, MODEL, TEST_TRAP },
  );
  assert.deepEqual(errors, []);
  assert.match(result.written, /^storageFailed: the database's core failed: /);
  assert.equal(
    result.later,
    "storageFailed: the database's core stopped after an earlier failure, such as a write too large for its memory: close the database and open it again",
  );
  assert.equal(result.closed, "ok");
  assert.equal(result.title, "before");
});
