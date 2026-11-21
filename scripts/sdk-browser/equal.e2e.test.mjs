// `.equal()` on every field type an index may fix (RFC 0005, "Indexes":
// nc.string, nc.int, nc.float, nc.boolean, nc.oneOf, nc.date,
// nc.timestamp, nc.link and nc.ref), in the browser, as an app writes and
// queries them (NOC-423, NOC-412): a link is the target's `$id`, a
// reference `{ space, id }`, a whole float a whole number, a day its
// "YYYY-MM-DD" text and a point in time a Date. Each query finds the one task that holds the value. The
// bridge's own test is crates/nocentraldb/src/bridge/tests/find_equal.rs.
//
// Not part of `pnpm gates`, which runs without browsers: the task
// `repo:sdk-browser` and the workflow browser-view.yml run it.
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

/** The fields an index here fixes, one index `eq<field>` each. */
const FIELDS = [
  "title",
  "status",
  "priority",
  "estimate",
  "urgent",
  "project",
  "customer",
  "doneOn",
  "dueAt",
];

/**
 * The test lock whose tasks have, instead of their own indexes, an index
 * `eq<field>` per field of FIELDS with that field as its equality prefix.
 * The core checks that a query runs through its index, so no other index
 * starts with the same column.
 */
const lock = JSON.parse(readFileSync(LOCK, "utf8"));
lock.collections.tasks.indexes = Object.fromEntries(
  FIELDS.map((field) => [
    `eq${field}`,
    { equal: 1, fields: [{ direction: "asc", field }], since: 1 },
  ]),
);

/** The page's model: the test lock's fields, with the indexes above. */
const MODEL = `nc.model({
  tasks: nc.collection(
    {
      title: nc.string(),
      status: nc.oneOf(["open", "done"]),
      priority: nc.int(),
      estimate: nc.float(),
      urgent: nc.boolean(),
      project: nc.link("projects"),
      customer: nc.ref("projects"),
      doneOn: nc.date(),
      dueAt: nc.timestamp(),
    },
    {
      indexes: Object.fromEntries(
        ${JSON.stringify(FIELDS)}.map((f) => ["eq" + f, nc.index([f], { equal: 1 })]),
      ),
    },
  ),
  projects: nc.collection({ name: nc.string() }),
})`;

test(".equal() finds the object on every field type an index may fix", async () => {
  const page = await (await browser.newContext()).newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error));
  await page.goto(`${server.url}/`);
  await page.waitForFunction(() => window.ready === true);
  const found = await page.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "equal" });
      const space = await db.spaces.create();
      const created = async (collection, values) =>
        (await space.create(collection, values)).ids[0];
      const p = await created(model.projects, { name: "p" });
      const q = await created(model.projects, { name: "q" });
      const ref = (project) => ({ space: space.id, id: project });
      const a = await created(model.tasks, {
        title: "a",
        status: "open",
        priority: 1,
        estimate: 2,
        urgent: true,
        project: p,
        customer: ref(p),
        doneOn: "1969-12-31",
        dueAt: new Date(-1),
      });
      const b = await created(model.tasks, {
        title: "b",
        status: "done",
        priority: 2,
        estimate: 2.5,
        urgent: false,
        project: q,
        customer: ref(q),
        doneOn: "2026-11-21",
        dueAt: new Date(1795288124636),
      });
      const name = (task) =>
        task.$id === a ? "a" : task.$id === b ? "b" : "?";
      const cases = [
        ["title", "a"],
        ["status", "done"],
        ["priority", 1],
        ["estimate", 2],
        ["estimate", 2.5],
        ["urgent", false],
        ["project", p],
        ["project", q],
        ["customer", ref(p)],
        ["customer", ref(q)],
        ["doneOn", "1969-12-31"],
        ["doneOn", "2026-11-21"],
        ["dueAt", new Date(-1)],
        ["dueAt", new Date(1795288124636)],
      ];
      const results = [];
      for (const [field, value] of cases) {
        const index = model.tasks.indexes[`eq${field}`];
        try {
          const page = await db.find(index.equal(value));
          results.push([field, page.items.map(name)]);
        } catch (error) {
          results.push([field, `${error.code}: ${error.message}`]);
        }
      }
      await db.close();
      return results;
    },
    { lock, MODEL },
  );
  assert.deepEqual(found, [
    ["title", ["a"]],
    ["status", ["b"]],
    ["priority", ["a"]],
    ["estimate", ["a"]],
    ["estimate", ["b"]],
    ["urgent", ["b"]],
    ["project", ["a"]],
    ["project", ["b"]],
    ["customer", ["a"]],
    ["customer", ["b"]],
    ["doneOn", ["a"]],
    ["doneOn", ["b"]],
    ["dueAt", ["a"]],
    ["dueAt", ["b"]],
  ]);
  assert.deepEqual(errors, []);
});
