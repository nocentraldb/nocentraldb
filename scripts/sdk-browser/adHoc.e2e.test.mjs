// End to end (NOC-318 S4): an ad-hoc space for a sub-agent, through the
// TypeScript SDK in a browser on the real core (RFC 0009, "Sub-agents and
// ad-hoc spaces"; the shapes the owner decided on NOC-351, Carsten,
// 2026-11-16). Run as sdk.e2e.test.mjs is: by the task `repo:sdk-browser`,
// not by `pnpm gates`.
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
  tasks: nc.collection(
    { title: nc.string(), priority: nc.int() },
    { indexes: { byPriority: nc.index(["priority"]) } },
  ),
})`;

test("db.spaces.createAdHoc copies the objects the SDK read, and the parent finds each copy", async () => {
  const opened = await (await browser.newContext()).newPage();
  const errors = [];
  opened.on("pageerror", (error) => errors.push(error));
  await opened.goto(`${server.url}/`);
  await opened.waitForFunction(() => window.ready === true);

  const result = await opened.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "adhoc" });
      const project = await db.spaces.create();
      const {
        ids: [a, b],
      } = await project.write((tx) => {
        tx.create(model.tasks, { title: "A", priority: 1 });
        tx.create(model.tasks, { title: "B", priority: 2 });
      });
      const taskA = await project.get(model.tasks, a);
      const {
        items: [first],
      } = await db.find(model.tasks.indexes.byPriority);

      const created = await db.spaces.createAdHoc({ copy: [taskA] });
      const copy = created.copies[0].copy;
      const read = await created.space.get(model.tasks, copy);
      const sourceOf = await created.space.sourceOf(copy);
      const copyOf = await created.space.copyOf(project.id, a);
      const notCopied = await created.space.copyOf(project.id, b);
      const spread = await db.spaces
        .createAdHoc({ copy: [{ ...first }] })
        .catch((error) => error.code);
      const local = await db.spaces
        .createAdHoc({ copy: [first], local: true })
        .catch((error) => error.code);
      await db.close();
      return {
        project: project.id,
        a,
        space: created.space.id,
        notice: created.notice.code,
        copies: created.copies,
        clearedLinks: created.clearedLinks,
        pluginFieldsNotCopied: created.pluginFieldsNotCopied,
        title: read?.title,
        readSpace: read?.$space,
        copy,
        sourceOf,
        copyOf,
        notCopied: notCopied ?? "none",
        spread,
        local,
      };
    },
    { lock, MODEL },
  );

  assert.notEqual(result.space, result.project);
  assert.equal(result.notice, "adHocSpaceCreated");
  assert.equal(result.copies.length, 1);
  assert.equal(result.copies[0].source, result.a);
  assert.notEqual(result.copy, result.a, "a copy gets a fresh id");
  assert.equal(result.clearedLinks, 0);
  assert.equal(result.pluginFieldsNotCopied, 0);
  assert.equal(result.title, "A");
  assert.equal(result.readSpace, result.space);
  assert.deepEqual(result.sourceOf, { space: result.project, id: result.a });
  assert.equal(result.copyOf, result.copy);
  assert.equal(result.notCopied, "none");
  assert.equal(result.spread, "invalidValue");
  assert.equal(result.local, "notAllowed");
  assert.deepEqual(errors, []);
});
