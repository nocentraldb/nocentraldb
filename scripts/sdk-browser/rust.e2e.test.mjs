// End to end with a browser (NOC-178; the last step of NOC-66, "the
// browser"): a Chromium tab with the TypeScript SDK and a Rust device with
// the Rust SDK share a space through the local relay server on SQLite, as
// the Rust SDK's own two-device test does (two_devices.rs). The Rust device
// creates the space and adds the tab with `members().add` and the tab's
// invitation, once its fingerprint is compared; both write, also
// concurrently into one object while the network is cut; both sync, and
// their reads compare equal (GENESIS.md, "Convergence").
//
// Both reach the relay through a proxy the test cuts. Each is of its own
// account; the relay lets the Rust device's account create spaces
// (`--allow-invitation`), and the tab only as a member.
//
// Chromium only, until NOC-174 shows the Rust port runs in Firefox and
// WebKit. Not part of `pnpm gates`, which runs without browsers: the task
// `repo:sdk-browser` and the workflow browser-view.yml run it.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { chromium } from "playwright";
import { build, LOCK, prebuilt } from "./build.mjs";
import { buildPeer, prebuiltPeer, startPeer, startProxy } from "./peer.mjs";
import { startRelay } from "./relay.mjs";
import { startServer } from "./server.mjs";

let server;
let browser;

before(async () => {
  if (process.env.SDK_BROWSER_PREBUILT === "1") {
    prebuilt();
    prebuiltPeer();
  } else {
    build();
    buildPeer();
  }
  server = await startServer({ port: 0 });
  browser = await chromium.launch();
});

after(async () => {
  await browser?.close();
  await server?.close();
});

const lock = JSON.parse(readFileSync(LOCK, "utf8"));

/** The tab's model: the test lock's tasks, as sdk.e2e.test.mjs declares them. */
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

/** Code the tab runs in the page, where `window.tab` holds its database. */
const TAB = `
  window.waitFor = async (what, done) => {
    const start = Date.now();
    for (;;) {
      const value = await done();
      if (value) return value;
      if (Date.now() - start > 30000) throw new Error(what + ": not in time");
      await new Promise((resolve) => setTimeout(resolve, 20));
    }
  };
  window.synced = () =>
    window.waitFor("synced", () => {
      const status = window.tab.db.sync.current();
      return status.connection === "connected" && status.unconfirmed === 0;
    });
  window.read = async (space, ids) => {
    const { db, model } = window.tab;
    const project = await db.spaces.get(space);
    const tasks = [];
    for (const id of ids) {
      const task = await project.get(model.tasks, id);
      tasks.push(
        task && {
          id: task.$id,
          deleted: task.$deleted,
          title: task.title ?? null,
          status: task.status ?? null,
          priority: task.priority ?? null,
          tags: [...(task.tags ?? [])].sort(),
        },
      );
    }
    return tasks;
  };
`;

test("a Chromium tab and a Rust device share a space through the local relay and read the same", async () => {
  const proxy = await startProxy();
  const rust = await startPeer(LOCK, proxy.url);
  const relay = await startRelay(["--allow-invitation", rust.invitation]);
  const context = await browser.newContext();
  const tab = await context.newPage();
  const errors = [];
  tab.on("pageerror", (error) => errors.push(error));
  try {
    proxy.lead(relay.url);
    await rust.ask("resume");

    await tab.goto(`${server.url}/`);
    await tab.waitForFunction(() => window.ready === true);
    await tab.addScriptTag({ content: TAB });
    const invited = await tab.evaluate(
      async ({ lock, MODEL, url }) => {
        const { nc } = window;
        const model = new Function("nc", `return ${MODEL}`)(nc);
        const db = await nc.open({ model, lock, name: "tab", relay: url });
        window.tab = { db, model };
        await window.waitFor(
          "connected",
          () => db.sync.current().connection === "connected",
        );
        const invitation = await db.invitation();
        return {
          text: String(invitation),
          fingerprint: invitation.fingerprint,
        };
      },
      { lock, MODEL, url: proxy.url },
    );

    // The Rust device creates the space and adds the tab, comparing the
    // fingerprint the tab shows.
    const [space, shared] = (await rust.ask("create")).split(" ");
    const added = await rust.ask(
      `add ${space} ${invited.text} ${invited.fingerprint}`,
    );
    assert.equal(added, "MemberAdded");
    await rust.ask("synced");

    // The tab stays connected: the relay tells it that it was added.
    const ofTab = await tab.evaluate(
      async ({ space, shared }) => {
        const { db, model } = window.tab;
        const project = await window.waitFor("the tab holds the space", () =>
          db.spaces.get(space).catch(() => null),
        );
        await window.waitFor("the tab holds the Rust device's task", () =>
          project.get(model.tasks, shared),
        );
        const { ids } = await project.create(model.tasks, {
          title: "from the tab",
          priority: 2,
        });
        await window.synced();
        return ids[0];
      },
      { space, shared },
    );
    await rust.ask(`holds ${space} ${ofTab}`);
    await rust.ask("synced");

    // Concurrent writes into one object: neither sees the other's change
    // before it writes its own.
    proxy.cut();
    await rust.ask(`update ${space} ${shared}`);
    await tab.evaluate(
      async ({ space, shared, ofTab }) => {
        const { db, model, nc } = { ...window.tab, nc: window.nc };
        const project = await db.spaces.get(space);
        await project.write((tx) => {
          tx.update(model.tasks, shared, {
            title: "the tab's title",
            tags: nc.add("y"),
            priority: 5,
          });
          tx.update(model.tasks, ofTab, { status: "done" });
        });
      },
      { space, shared, ofTab },
    );
    proxy.restore();

    await rust.ask("synced");
    await tab.evaluate(() => window.synced());

    // Both hold both writes once each has the other's change: the tags
    // of each, and the tab's status.
    const ids = [shared, ofTab];
    const both = (tasks) =>
      tasks[0]?.tags.join() === "rust,x,y" && tasks[1]?.status === "done";
    const start = Date.now();
    let onRust;
    let onTab;
    for (;;) {
      onRust = JSON.parse(await rust.ask(`read ${space} ${ids.join(" ")}`));
      onTab = await tab.evaluate(({ space, ids }) => window.read(space, ids), {
        space,
        ids,
      });
      if (both(onRust) && both(onTab)) break;
      assert.ok(Date.now() - start < 30000, "both hold both writes in time");
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    assert.deepEqual(onTab, onRust);
    assert.deepEqual(onRust[0].tags, ["rust", "x", "y"]);
    assert.equal(onRust[0].priority, 5);
    assert.ok(
      ["Rust's title", "the tab's title"].includes(onRust[0].title),
      onRust[0].title,
    );
    assert.deepEqual(onRust[1], {
      id: ofTab,
      deleted: false,
      title: "from the tab",
      status: "done",
      priority: 2,
      tags: [],
    });
    assert.deepEqual(errors, []);
  } finally {
    await context.close();
    rust.stop();
    relay.stop();
    await proxy.close();
  }
});
