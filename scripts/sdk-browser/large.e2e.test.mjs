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
