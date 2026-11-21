// Erasing a person in the browser (NOC-406; RFC 0009, "The Rust SDK",
// "Erasing a person"; Carsten, 2026-11-15, NOC-326): the TypeScript SDK's
// `db.erasureRequest`, `db.account.signOff`, `db.erase` and
// `space.erasures.takeBack`, as an app uses them.
//
// The first test erases for good: two databases of two accounts sync
// through the local relay; the performer writes Anna, a person, adds the
// signer as an admin, asks for the request, the signer signs it off from
// its text, and the performer erases with the sign-offs' text. Anna's
// name reads unset on both, and the audit lists the shred as final.
//
// The second test erases provisionally and takes it back: in a space
// whose members all belong to one account, the database signs off its
// own request, the erasure is provisional, and an admin of the account
// takes it back, so Anna's name reads again.
//
// The model is erase/model.ts, with its lock beside it. The relay lets
// the performer's account create spaces (`--allow-invitation`), as
// account.e2e.test.mjs does.
//
// Not part of `pnpm gates`, which runs without browsers: the task
// `repo:sdk-browser` and the workflow browser-view.yml run it, in each
// engine.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { build, prebuilt, ROOT } from "./build.mjs";
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

const lock = JSON.parse(
  readFileSync(
    join(ROOT, "scripts", "sdk-browser", "erase", "model.lock.json"),
    "utf8",
  ),
);

/** erase/model.ts, as an app declares it. */
const MODEL = `nc.model({
  people: nc.collection({ name: nc.string().personal({ person: "self" }) }),
})`;

/** A fresh page with the package loaded, `window.model` and `window.until` set; a fresh context, so a fresh OPFS. */
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

test("a second account signs off, and the person is erased for good on both databases", async () => {
  const { page: performer, errors } = await page();
  const { page: signer, errors: signerErrors } = await page();
  // The performer's invitation, for the relay to let its account create spaces.
  const invitation = await performer.evaluate(async (lock) => {
    const { nc, model } = window;
    const db = await nc.open({ model, lock, name: "performer" });
    const text = String(await db.invitation());
    await db.close();
    return text;
  }, lock);
  const relay = await startRelay(["--allow-invitation", invitation]);
  try {
    const signerInvitation = await signer.evaluate(
      async ({ lock, url }) => {
        const { nc, model } = window;
        window.db = await nc.open({ model, lock, name: "signer", relay: url });
        return String(await window.db.invitation());
      },
      { lock, url: relay.url },
    );
    const { space, anna } = await performer.evaluate(
      async ({ lock, url, signerInvitation }) => {
        const { nc, model } = window;
        window.db = await nc.open({ model, lock, name: "performer", relay: url });
        const space = await window.db.spaces.create();
        const invitation = nc.invitation(signerInvitation);
        const fingerprint = await space.members
          .add(invitation, { role: "admin" })
          .catch((error) => error.fingerprint);
        await space.members.add(invitation, { role: "admin", fingerprint });
        const { ids } = await space.create(model.people, {
          name: "Anna Schmidt",
        });
        await window.until(
          "the relay confirmed the performer's changes",
          () => window.db.sync.current().unconfirmed === 0,
        );
        return { space: space.id, anna: ids[0] };
      },
      { lock, url: relay.url, signerInvitation },
    );
    const request = await performer.evaluate(
      async ({ space, anna }) => {
        const read = await (await window.db.spaces.get(space)).get(
          window.model.people,
          anna,
        );
        window.request = await window.db.erasureRequest(read);
        return String(window.request);
      },
      { space, anna },
    );
    assert.match(request, /^ncdb-erase-1:/);

    // The signer signs off once it holds the space and the person key.
    const signOffs = await signer.evaluate(async (request) => {
      const { nc } = window;
      return window.until("the signer signs the space off", async () => {
        const signed = await window.db.account.signOff(
          nc.erasureRequest(request),
        );
        return signed.notSigned.length === 0 && String(signed.signOffs);
      });
    }, request);
    assert.match(signOffs, /^ncdb-sign-off-1:/);

    const erased = await performer.evaluate(
      async ({ space, anna, signOffs }) => {
        const { nc, db, model } = window;
        const erased = await db.erase(window.request, nc.signOffs(signOffs));
        const project = await db.spaces.get(space);
        const read = await project.get(model.people, anna);
        const audit = await project.erasures.list();
        await window.until(
          "the relay confirmed the shred",
          () => db.sync.current().unconfirmed === 0,
        );
        return {
          shreds: erased.shreds.map((shred) => ({
            space: shred.space === space,
            provisional: shred.provisional,
            audited: audit.some(
              (erasure) =>
                erasure.shred === shred.shred && erasure.state === "final",
            ),
          })),
          skipped: erased.skipped,
          notice: erased.notice.code,
          name: read.name ?? null,
        };
      },
      { space, anna, signOffs },
    );
    assert.deepEqual(erased.shreds, [
      { space: true, provisional: false, audited: true },
    ]);
    assert.deepEqual(erased.skipped, []);
    assert.equal(erased.notice, "erased");
    assert.equal(erased.name, null, "Anna's name reads unset");

    const nameOnSigner = await signer.evaluate(
      async ({ space, anna }) => {
        const project = await window.db.spaces.get(space);
        return window.until("the signer reads Anna erased", async () => {
          const read = await project.get(window.model.people, anna);
          return read !== undefined && read.name === undefined && "unset";
        });
      },
      { space, anna },
    );
    assert.equal(nameOnSigner, "unset");
    assert.deepEqual(errors, []);
    assert.deepEqual(signerErrors, []);
  } finally {
    relay.stop();
  }
});

test("a database signs off its own erasure in a space of its account, which is provisional, and takes it back", async () => {
  const { page: alone, errors } = await page();
  const result = await alone.evaluate(async (lock) => {
    const { nc, model } = window;
    const db = await nc.open({ model, lock, name: "alone" });
    const space = await db.spaces.create();
    const { ids } = await space.create(model.people, { name: "Anna Schmidt" });
    const read = await space.get(model.people, ids[0]);
    const request = await db.erasureRequest(read);
    const { signOffs, notSigned } = await db.account.signOff(request);
    const erased = await db.erase(request, signOffs);
    const nameErased = (await space.get(model.people, ids[0])).name ?? null;
    const shred = erased.shreds[0].shred;
    const state = (await space.erasures.list()).find(
      (erasure) => erasure.shred === shred,
    ).state;
    const notice = await space.erasures.takeBack(shred);
    const nameAgain = (await space.get(model.people, ids[0])).name;
    const refused = await space.erasures.takeBack(shred).then(
      () => "taken back twice",
      (error) => error.code,
    );
    await db.close();
    return {
      notSigned,
      provisional: erased.shreds.map((shred) => shred.provisional),
      nameErased,
      state,
      notice: notice.code,
      nameAgain,
      refused,
    };
  }, lock);
  assert.deepEqual(result, {
    notSigned: [],
    provisional: [true],
    nameErased: null,
    state: "provisional",
    notice: "takenBack",
    nameAgain: "Anna Schmidt",
    refused: "invalidValue",
  });
  assert.deepEqual(errors, []);
});
