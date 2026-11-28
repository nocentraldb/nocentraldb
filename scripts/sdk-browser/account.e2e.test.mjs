// Accounts in the browser through the local relay (NOC-316, slice 3; RFC
// 0009, "Accounts and devices"; RFC 0010, "Losing a device"): a laptop
// reads the phone's fingerprint and pairs the phone into its account, both sync, and the laptop removes the
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
import { build, LOCK, prebuilt } from "./build.mjs";
import { launch } from "./engine.mjs";
import { startRelay } from "./relay.mjs";
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
    // The phone opens with the relay as it waits to be paired, as an app
    // opens a new device.
    const asked = await phone.evaluate(
      async ({ lock, url }) => {
        const { nc, model } = window;
        window.db = await nc.open({
          model,
          lock,
          name: "phone",
          pairing: "request",
          relay: url,
        });
        const invitation = await window.db.invitation();
        return { asked: String(invitation), shown: invitation.fingerprint };
      },
      { lock, url: relay.url },
    );
    // The laptop reads the digits a person compares with the ones the
    // phone shows, then pairs it (RFC 0010, step 2; NOC-583).
    const { paired, read } = await laptop.evaluate(
      async ({ lock, url, asked }) => {
        const { nc, model } = window;
        window.db = await nc.open({ model, lock, name: "laptop", relay: url });
        const project = await window.db.spaces.create();
        window.project = project.id;
        const invitation = nc.invitation(asked);
        const read = await window.db.account.pairingFingerprint(invitation);
        const { answer } = await window.db.account.pair({
          kind: "device",
          invitation,
        });
        return { paired: String(answer), read };
      },
      { lock, url: relay.url, asked: asked.asked },
    );
    assert.match(asked.shown, /^\d{5}( \d{5}){11}$/);
    assert.equal(read, asked.shown, "the laptop reads the phone's digits");
    // The phone completes the pairing while it syncs (NOC-337). It writes
    // its records into the account space as it completes the pairing and
    // as it takes the project. Once the relay has confirmed them, it
    // writes a task in the project, and the laptop waits until it reads
    // that task. The laptop may still learn the account space's changes a
    // moment later than the project's, so it tries again while the phone
    // is not found, which writes nothing.
    const project = await laptop.evaluate(() => window.project);
    const task = await phone.evaluate(
      async ({ answer, project }) => {
        const { nc, model } = window;
        await window.db.account.completePairing(nc.pairingAnswer(answer));
        const space = await window.until("the phone holds the project", () =>
          window.db.spaces.get(project).catch(() => null),
        );
        await window.until(
          "the relay confirmed the phone's records",
          () => window.db.sync.current().unconfirmed === 0,
        );
        const { ids } = await space.create(model.tasks, {
          title: "from the phone",
          status: "open",
          priority: 1,
        });
        return ids[0];
      },
      { answer: paired, project },
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
      const removed = await window.until("the laptop knows the phone", () =>
        db.account.removeMember(phone, { recover: words }).catch((error) => {
          if (error.code === "notFound") return null;
          throw error;
        }),
      );
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
