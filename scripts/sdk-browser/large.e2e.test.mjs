// End to end (NOC-425): a value of more than 1 MiB through `space.write` in
// a browser, on the real core. The SDK's CBOR writer spread every byte as
// an argument and overflowed the stack from about 125 KiB, so such a write
// threw a RangeError instead of reaching the core. Run as
// sdk.e2e.test.mjs is: by the task `repo:sdk-browser`, not by `pnpm gates`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { build, LOCK, prebuilt } from "./build.mjs";
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

const lock = JSON.parse(readFileSync(LOCK, "utf8"));

const MODEL = `nc.model({
  tasks: nc.collection({ title: nc.string(), priority: nc.int() }),
})`;

test("space.write takes a value of more than 1 MiB, and a get reads it back", async () => {
  const opened = await (await browser.newContext()).newPage();
  const errors = [];
  opened.on("pageerror", (error) => errors.push(error));
  await opened.goto(`${server.url}/`);
  await opened.waitForFunction(() => window.ready === true);

  const result = await opened.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "large" });
      const project = await db.spaces.create();
      const title = "x".repeat(1536 * 1024);
      const written = await project
        .write((tx) => {
          tx.create(model.tasks, { title, priority: 1 });
        })
        .then(
          ({ ids }) => ({ id: ids[0] }),
          (e) => ({ error: `${e.name}: ${e.code ?? e.message}` }),
        );
      const read =
        written.id === undefined
          ? undefined
          : await project.get(model.tasks, written.id);
      await db.close();
      return {
        written: written.error ?? "ok",
        same: read?.title === title,
        length: read?.title.length,
      };
    },
    { lock, MODEL },
  );
  assert.deepEqual(result, {
    written: "ok",
    same: true,
    length: 1536 * 1024,
  });
  assert.deepEqual(errors, []);
});

// A write above the relay's change limit of 16 MiB (NOC-421; NOC-431, B′):
// many objects are split into several changes and the write goes through,
// every object readable at once; one value above the limit is refused with
// the nc.Error `tooLarge` before anything is written.
test("space.write splits many objects above 16 MiB, and refuses one value above it with tooLarge", async () => {
  const opened = await (await browser.newContext()).newPage();
  const errors = [];
  opened.on("pageerror", (error) => errors.push(error));
  await opened.goto(`${server.url}/`);
  await opened.waitForFunction(() => window.ready === true);

  const result = await opened.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "split" });
      const project = await db.spaces.create();
      const title = (n) => `${n}`.padStart(8, "0") + "x".repeat(256 * 1024);
      const { ids } = await project.write((tx) => {
        for (let n = 0; n < 72; n++)
          tx.create(model.tasks, { title: title(n) });
      });
      let same = 0;
      for (const [n, id] of ids.entries()) {
        const task = await project.get(model.tasks, id);
        if (task?.title === title(n)) same++;
      }
      const huge = "x".repeat(16 * 1024 * 1024 + 1);
      const refused = await project
        .write((tx) => {
          tx.create(model.tasks, { title: huge });
        })
        .then(
          () => "written",
          (e) => (e instanceof nc.Error ? e.code : `${e.name}: ${e.message}`),
        );
      await db.close();
      return { ids: ids.length, same, refused };
    },
    { lock, MODEL },
  );
  assert.deepEqual(result, { ids: 72, same: 72, refused: "tooLarge" });
  assert.deepEqual(errors, []);
});

// A value of 256 MiB (NOC-425): it never fails with anything but an
// nc.Error with a code. Since NOC-421 the SDK refuses it with `tooLarge`
// before the core would run out of memory; trap.e2e.test.mjs holds what
// follows a real trap of the core (NOC-499).
test("a write too large for the core fails with an nc.Error that has a code", async () => {
  const opened = await (await browser.newContext()).newPage();
  const errors = [];
  opened.on("pageerror", (error) => errors.push(error));
  await opened.goto(`${server.url}/`);
  await opened.waitForFunction(() => window.ready === true);

  const result = await opened.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "too-large" });
      const project = await db.spaces.create();
      const title = "x".repeat(256 * 1024 * 1024);
      const outcome = await project
        .write((tx) => {
          tx.create(model.tasks, { title, priority: 1 });
        })
        .then(
          () => "written",
          (e) =>
            e instanceof nc.Error && typeof e.code === "string" && e.code
              ? "nc.Error"
              : `${e.name}: ${e.message}`,
        );
      await db.close().catch(() => {});
      return outcome;
    },
    { lock, MODEL },
  );
  assert.ok(["written", "nc.Error"].includes(result), result);
  assert.deepEqual(errors, []);
});

// After a write the core could not hold (NOC-426): the core trapped and is
// broken for good, so every later call fails with `storageFailed` and a
// text that says to open the database again; `db.close()` succeeds, and the
// database opened again works and holds what was written before. Where the
// engine's core holds the value, the SDK refuses it with `tooLarge`, since
// one value of 256 MiB does not fit into one change under the relay's limit
// (NOC-421), or the browser's storage refuses it with `storageFull`
// (WebKit's, since NOC-422 made the core hold it); in each case the core
// runs on and the later call simply works. trap.e2e.test.mjs holds what
// follows a real trap of the core (NOC-499). In no case does anything
// reach the page as an uncaught error.
test("after a write too large for the core, later calls say to open the database again, and it opens again", async () => {
  const opened = await (await browser.newContext()).newPage();
  const errors = [];
  opened.on("pageerror", (error) => errors.push(error));
  await opened.goto(`${server.url}/`);
  await opened.waitForFunction(() => window.ready === true);

  const result = await opened.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const outcome = (promise) =>
        promise.then(
          () => "ok",
          (e) =>
            e instanceof nc.Error ? `${e.code}: ${e.message}` : String(e),
        );
      const db = await nc.open({ model, lock, name: "after-too-large" });
      const project = await db.spaces.create();
      const { ids } = await project.write((tx) => {
        tx.create(model.tasks, { title: "before", priority: 1 });
      });
      const written = await outcome(
        project.write((tx) => {
          tx.create(model.tasks, {
            title: "x".repeat(256 * 1024 * 1024),
            priority: 2,
          });
        }),
      );
      const later = await outcome(project.get(model.tasks, ids[0]));
      const closed = await outcome(db.close());
      const again = await nc.open({ model, lock, name: "after-too-large" });
      const [space] = await again.spaces.list();
      const read = await (await again.spaces.get(space.id)).get(
        model.tasks,
        ids[0],
      );
      await again.close();
      return { written, later, closed, title: read?.title };
    },
    { lock, MODEL },
  );
  assert.deepEqual(errors, []);
  assert.equal(result.closed, "ok");
  assert.equal(result.title, "before");
  if (
    result.written === "ok" ||
    result.written.startsWith("tooLarge: ") ||
    result.written.startsWith("storageFull: ")
  ) {
    assert.equal(result.later, "ok");
  } else {
    assert.match(result.written, /^storageFailed: /);
    assert.equal(
      result.later,
      "storageFailed: the database's core stopped after an earlier failure, such as a write too large for its memory: close the database and open it again",
    );
  }
});
