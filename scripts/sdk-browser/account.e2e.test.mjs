// Accounts in Chromium through the local relay (NOC-316, slice 3; RFC
// 0009, "Accounts and devices"; RFC 0010, "Losing a device"): a laptop
// pairs a phone into its account, both sync, and the laptop removes the
// lost phone with the recovery code. Removing needs the phone's records
// in the account space, which the phone writes and the laptop learns by
// syncing, so this test, and not the bridge's, shows a removal that
// succeeds.
//
// The relay lets the laptop's account create spaces
// (`--allow-invitation`), as rust.e2e.test.mjs lets the Rust device's.
//
// Not part of `pnpm gates`, which runs without browsers: the task
// `repo:sdk-browser` and the workflow browser-view.yml run it.
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

/** A fresh page with the package loaded and `window.model` set; a fresh context, so a fresh OPFS. */
const page = async () => {
  const opened = await (await browser.newContext()).newPage();
  const errors = [];
  opened.on("pageerror", (error) => errors.push(error));
  await opened.goto(`${server.url}/`);
  await opened.waitForFunction(() => window.ready === true);
  await opened.evaluate((MODEL) => {
    window.model = new Function("nc", `return ${MODEL}`)(window.nc);
    /** Polls `check` until it gives a value, for up to 20 seconds. */
    window.until = async (what, check) => {
      const start = Date.now();
      for (;;) {
        const got = await check();
        if (got) return got;
        if (Date.now() - start > 20000) throw new Error(`never: ${what}`);
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
    };
  }, MODEL);
  return { page: opened, errors };
};

test("a device removes a lost one from its spaces with the recovery code, which renews the account key", async () => {
  // The laptop's invitation, for the relay to let its account create spaces.
  const { page: laptop, errors } = await page();
  const { invitation, words } = await laptop.evaluate(async (lock) => {
    const { nc, model } = window;
    const db = await nc.open({ model, lock, name: "laptop" });
    const invitation = String(await db.invitation());
    const words = [...(await db.account.recoveryCode()).words()].join(" ");
    await db.close();
    return { invitation, words };
  }, lock);
  const relay = await startRelay(["--allow-invitation", invitation]);
  try {
    const { page: phone, errors: phoneErrors } = await page();
    const asked = await phone.evaluate(async (lock) => {
      const { nc, model } = window;
      window.db = await nc.open({
        model,
        lock,
        name: "phone",
        pairing: "request",
      });
      return String(await window.db.invitation());
    }, lock);
    const paired = await laptop.evaluate(
      async ({ lock, url, asked }) => {
        const { nc, model } = window;
        window.db = await nc.open({ model, lock, name: "laptop", relay: url });
        const project = await window.db.spaces.create();
        window.project = project.id;
        const { answer } = await window.db.account.pair({
          kind: "device",
          invitation: nc.invitation(asked),
        });
        return String(answer);
      },
      { lock, url: relay.url, asked },
    );
    // The phone completes the pairing, then opens again with the relay. A
    // phone that syncs while it waits to be paired does not yet pick up
    // the spaces the pairing adds it to (reported with NOC-316 slice 3).
    // Once it holds the project, it writes a task there; the laptop
    // reading that task shows that the phone's records, which it writes
    // into the account space as it takes the project, have reached the
    // laptop too.
    const project = await laptop.evaluate(() => window.project);
    const task = await phone.evaluate(
      async ({ answer, lock, url, project }) => {
        const { nc, model } = window;
        await window.db.account.completePairing(nc.pairingAnswer(answer));
        await window.db.close();
        window.db = await nc.open({ model, lock, name: "phone", relay: url });
        const space = await window.until("the phone holds the project", () =>
          window.db.spaces.get(project).catch(() => null),
        );
        const { ids } = await space.create(model.tasks, {
          title: "from the phone",
          status: "open",
          priority: 1,
        });
        return ids[0];
      },
      { answer: paired, lock, url: relay.url, project },
    );
    await laptop.evaluate(
      async ({ project, task }) => {
        const space = await window.db.spaces.get(project);
        await window.until("the laptop reads the phone's task", () =>
          space.get(window.model.tasks, task),
        );
      },
      { project, task },
    );

    const result = await laptop.evaluate(async (words) => {
      const { db, project } = window;
      const space = await db.spaces.get(project);
      const others = (await space.members.list()).filter((m) => !m.self);
      const phone = others[0].id;
      const removed = await db.account.removeMember(phone, { recover: words });
      const left = await space.members.list();
      return {
        others: others.length,
        removed: removed.removed.map(({ space, notice }) => [
          space === project,
          notice.code,
        ]),
        askAdmins: removed.askAdmins,
        notice: removed.notice.code,
        left: left.map((member) => member.self),
      };
    }, words);
    assert.equal(result.others, 1, "the phone is the one other member");
    assert.deepEqual(result.removed, [[true, "memberRemoved"]]);
    assert.deepEqual(result.askAdmins, []);
    assert.equal(result.notice, "accountKeyRenewed");
    assert.deepEqual(result.left, [true], "only the laptop is left");
    assert.deepEqual(errors, []);
    assert.deepEqual(phoneErrors, []);
  } finally {
    relay.stop();
  }
});
