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
import { build, LOCK, prebuilt } from "./build.mjs";
import { startRelay } from "./relay.mjs";
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
  assert.deepEqual(reread.spaces, [
    { id: written.space, role: "admin", model: "ours" },
  ]);
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

// NOC-176 slice 2: queries and subscriptions (RFC 0009, "Queries and
// indexes", "Subscribe").
test("db.find and space.find page through an index", async () => {
  const { page: opened } = await page();
  const found = await opened.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "queries" });
      const project = await db.spaces.create();
      const other = await db.spaces.create();
      await project.write((tx) => {
        for (const [priority, status] of [
          [3, "open"],
          [1, "open"],
          [2, "done"],
        ]) {
          tx.create(model.tasks, { priority, status });
        }
      });
      await other.create(model.tasks, { priority: 4, status: "open" });
      const query = model.tasks.indexes.byPriority.take(2);
      const first = await db.find(query);
      const second = await db.find(query.after(first.cursor));
      const open = await db.find(
        model.tasks.indexes.byStatusPriority.equal("open"),
      );
      const inProject = await project.find(
        model.tasks.indexes.open.equal("open"),
      );
      const cursorText = String(first.cursor);
      await db.close();
      const priorities = (p) => p.items.map((item) => item.priority);
      return {
        first: priorities(first),
        firstMore: first.hasMore,
        second: priorities(second),
        secondMore: second.hasMore,
        open: priorities(open),
        inProject: priorities(inProject),
        spaces: new Set(open.items.map((item) => item.$space)).size,
        cursorText,
      };
    },
    { lock, MODEL },
  );
  assert.deepEqual(found, {
    first: [1, 2],
    firstMore: true,
    second: [3, 4],
    secondMore: false,
    open: [1, 3, 4],
    inProject: [1, 3],
    spaces: 2,
    cursorText: "[redacted]",
  });
});

test("subscriptions re-run after a write, until stopped", async () => {
  const { page: opened } = await page();
  const seen = await opened.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "subscriptions" });
      const project = await db.spaces.create();
      const pages = [];
      const objects = [];
      const until = (check) =>
        new Promise((resolve, reject) => {
          const start = Date.now();
          const poll = () => {
            if (check()) return resolve();
            if (Date.now() - start > 5000)
              return reject(new Error("no update"));
            setTimeout(poll, 10);
          };
          poll();
        });
      const stop = db.subscribe(model.tasks.indexes.byPriority, (p) =>
        pages.push(p.items.map((item) => item.title)),
      );
      await until(() => pages.length === 1);
      const { ids } = await project.create(model.tasks, {
        title: "a",
        priority: 1,
      });
      await until(() => pages.length === 2);
      const stopOne = project.subscribe(model.tasks, ids[0], (task) =>
        objects.push(task?.title),
      );
      await until(() => objects.length === 1);
      await project.update(model.tasks, ids[0], { title: "b" });
      await until(() => objects.length === 2 && pages.length === 3);
      stop();
      stopOne();
      await project.create(model.tasks, { title: "c", priority: 2 });
      await new Promise((resolve) => setTimeout(resolve, 200));
      await db.close();
      return { pages, objects };
    },
    { lock, MODEL },
  );
  assert.deepEqual(seen.pages, [[], ["a"], ["b"]]);
  assert.deepEqual(seen.objects, ["a", "b"]);
});

// NOC-176 slice 3: db.changes (RFC 0009, "Subscribe").
test("db.changes names the ids a write touched, as local", async () => {
  const { page: opened } = await page();
  const result = await opened.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "changes" });
      const project = await db.spaces.create();
      const changes = [];
      const stop = db.changes.subscribe((change) => changes.push(change));
      const { ids } = await project.write((tx) => {
        tx.create(model.tasks, { title: "a" });
        tx.create(model.tasks, { title: "b" });
      });
      const start = Date.now();
      while (changes.length === 0 && Date.now() - start < 5000) {
        await new Promise((resolve) => setTimeout(resolve, 10));
      }
      stop();
      await db.close();
      return { changes, ids, space: project.id };
    },
    { lock, MODEL },
  );
  assert.equal(result.changes.length, 1, JSON.stringify(result.changes));
  const [change] = result.changes;
  assert.equal(change.space, result.space);
  assert.equal(change.collection, "tasks");
  assert.equal(change.origin, "local");
  assert.deepEqual([...change.ids].sort(), [...result.ids].sort());
});

// NOC-176 slice 4: db.sync (RFC 0009, "Sync and status"), with the local
// relay server. The relay allows no account to create spaces here, so the
// test follows the connection only; membership adds the rest.
test("nc.open with a relay connects, and db.sync pauses and resumes it", async () => {
  const relay = await startRelay();
  try {
    const { page: opened, errors } = await page();
    const seen = await opened.evaluate(
      async ({ lock, MODEL, url }) => {
        const { nc } = window;
        const model = new Function("nc", `return ${MODEL}`)(nc);
        const db = await nc.open({ model, lock, name: "sync", relay: url });
        const seen = [];
        const until = (connection) =>
          new Promise((resolve, reject) => {
            const timer = setTimeout(
              () =>
                reject(
                  new Error(
                    `not ${connection}: ${JSON.stringify(db.sync.current())}`,
                  ),
                ),
              20000,
            );
            const stop = db.sync.subscribe((status) => {
              if (status.connection !== connection) return;
              clearTimeout(timer);
              queueMicrotask(() => stop());
              resolve();
            });
          });
        seen.push(db.sync.current().relay);
        await until("connected");
        seen.push(db.sync.current().connection);
        await db.sync.pause();
        seen.push(db.sync.current().connection);
        await db.sync.resume();
        await until("connected");
        seen.push(db.sync.current().connection);
        await db.close();
        return seen;
      },
      { lock, MODEL, url: relay.url },
    );
    assert.deepEqual(seen, [relay.url, "connected", "paused", "connected"]);
    assert.deepEqual(errors, []);
  } finally {
    relay.stop();
  }
});

test("space.status gives a new space's status, and its handles share it", async () => {
  const { page: opened, errors } = await page();
  const result = await opened.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "status" });
      const project = await db.spaces.create();
      const again = await db.spaces.get(project.id);
      const seen = [];
      const stop = project.status.subscribe((status) => seen.push(status.role));
      stop();
      const status = project.status.current();
      await db.close();
      return { status, same: again.status.current(), seen };
    },
    { lock, MODEL },
  );
  assert.equal(result.status.role, "admin");
  assert.equal(result.status.initialSyncComplete, false);
  assert.deepEqual(result.status.provisionalErasures, []);
  assert.deepEqual(result.same, result.status);
  assert.deepEqual(result.seen, ["admin"]);
  assert.deepEqual(errors, []);
});

// NOC-315 slice 1: the space list as a report, and leaving a space (RFC
// 0009, "Spaces").
test("db.spaces.subscribe follows the list, and the last admin cannot leave", async () => {
  const { page: opened, errors } = await page();
  const result = await opened.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "spaces" });
      const seen = [];
      const first = new Promise((resolve) =>
        db.spaces.subscribe((all) => {
          seen.push(all);
          resolve();
        }),
      );
      await first;
      const project = await db.spaces.create();
      for (let i = 0; i < 100 && seen.length < 2; i += 1) {
        await new Promise((resolve) => setTimeout(resolve, 20));
      }
      const refused = await project.leave().catch((error) => error.code);
      await db.close();
      return { seen, id: project.id, refused };
    },
    { lock, MODEL },
  );
  assert.deepEqual(result.seen[0], []);
  assert.deepEqual(result.seen[1], [
    { id: result.id, role: "admin", model: "ours" },
  ]);
  assert.equal(result.refused, "lastAdmin");
  assert.deepEqual(errors, []);
});

// NOC-176 slice 4b: invitations and members (RFC 0009, "Invitations and
// members"), between two members, each in its own browser context: the
// databases of one origin share one OPFS pool, which one worker holds.
test("an admin adds a member by invitation once its fingerprint is confirmed, re-roles and removes it", async () => {
  const { page: other, errors: otherErrors } = await page();
  const own = await other.evaluate(
    async ({ lock, MODEL }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "other" });
      const invitation = await db.invitation();
      await db.close();
      return { text: String(invitation), fingerprint: invitation.fingerprint };
    },
    { lock, MODEL },
  );
  const { page: admin, errors } = await page();
  const seen = await admin.evaluate(
    async ({ lock, MODEL, text }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const db = await nc.open({ model, lock, name: "admin" });
      const project = await db.spaces.create();
      const invitation = nc.invitation(text);
      const seen = {};
      seen.unconfirmed = await project.members
        .add(invitation, { role: "writer" })
        .then(
          () => "added",
          (error) => ({ code: error.code, fingerprint: error.fingerprint }),
        );
      seen.added = (
        await project.members.add(invitation, {
          role: "writer",
          fingerprint: seen.unconfirmed.fingerprint,
        })
      ).notice.code;
      const them = (await project.members.list()).find((m) => !m.self);
      seen.listed = { kind: them.kind, role: them.role, id: them.id.length };
      seen.lowered = (
        await project.members.setRole(them.id, "reader")
      ).notice.code;
      seen.removed = (await project.members.remove(them.id)).notice.code;
      seen.left = (await project.members.list()).length;
      await db.close();
      return seen;
    },
    { lock, MODEL, text: own.text },
  );
  assert.equal(seen.unconfirmed.code, "unknownAccount");
  assert.equal(seen.unconfirmed.fingerprint, own.fingerprint);
  assert.equal(seen.added, "memberAdded");
  assert.deepEqual(seen.listed, { kind: "device", role: "writer", id: 64 });
  assert.equal(seen.lowered, "roleSet");
  assert.equal(seen.removed, "memberRemoved");
  assert.equal(seen.left, 1);
  assert.deepEqual(otherErrors, []);
  assert.deepEqual(errors, []);
});
