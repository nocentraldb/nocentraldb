// The memory budget of one change in a browser tab (NOC-479; Carsten,
// 2026-11-23, NOC-451): a change at the relay's 16 MiB limit fits in 256 MiB
// of the core's WebAssembly memory. The TypeScript SDK runs in Chromium as
// an app uses it, its core compiled to wasm32 in the SDK's worker. A laptop
// tab writes one change just below the limit and syncs it through the local
// relay; then it pairs a phone tab into its account, which receives the
// change as it catches up. The WebAssembly memory of each tab's core must
// stay within the budget. It never shrinks, so its size once the work is
// done is its peak. The native side of the budget is
// crates/nocentraldb/tests/memory_budget.rs.
//
// To read that memory, the test serves the page a copy of the package whose
// worker.js is instrumented to keep the core's WebAssembly memory where
// Playwright can read it; the package itself is unchanged.
//
// It runs only for releases: the `verify` job of
// .github/workflows/release.yml runs it in Playwright's image, for beta and
// stable tags and in the publish dry run, on the package and the relay that
// scripts/sdk-browser/build.mjs built (SDK_BROWSER_PREBUILT=1). It is not a
// file of `pnpm gates` or of browser-view.yml. By hand:
//
//   node scripts/sdk-browser/build.mjs
//   node --test scripts/memory-budget/browser.test.mjs
import assert from "node:assert/strict";
import {
  cpSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { build, DIST, LOCK, prebuilt } from "../sdk-browser/build.mjs";
import { engine, launch } from "../sdk-browser/engine.mjs";
import { startRelay } from "../sdk-browser/relay.mjs";
import { startServer } from "../sdk-browser/server.mjs";

/** The budget: 256 MiB. */
const BUDGET = 256 * 1024 * 1024;

/** Creates in the one change: the same as the native guard, whose change of them it checks is just below 16 MiB. */
const RECORDS = 42000;

/** How long a step may take: a 16 MiB change goes up and down through an upload. */
const PATIENCE = 180_000;

/**
 * Prepended to the package's worker.js: it keeps the WebAssembly memory of
 * the module the worker instantiates as `self.coreMemory`. Imports are
 * hoisted, so this runs before the worker's own code calls `init`.
 */
const PROBE = `{
  const keep = (result) => {
    const instance = result instanceof WebAssembly.Instance ? result : result.instance;
    self.coreMemory = instance.exports.memory;
    return result;
  };
  const instantiate = WebAssembly.instantiate;
  const streaming = WebAssembly.instantiateStreaming;
  WebAssembly.instantiate = (...args) => instantiate.apply(WebAssembly, args).then(keep);
  WebAssembly.instantiateStreaming = (...args) => streaming.apply(WebAssembly, args).then(keep);
}
`;

let server;
let browser;
let dist;

before(async () => {
  if (process.env.SDK_BROWSER_PREBUILT === "1") prebuilt();
  else build();
  assert.equal(engine, "chromium", "the budget is held in Chromium");
  // The package's build output, its worker.js with the probe in front.
  dist = mkdtempSync(join(tmpdir(), "memory-budget-sdk-"));
  cpSync(DIST, dist, { recursive: true });
  const worker = join(dist, "worker.js");
  writeFileSync(worker, PROBE + readFileSync(worker, "utf8"));
  server = await startServer({ port: 0, dist });
  // Normal windows on a profile on disk, as docs/bulk-import.md measured.
  browser = await launch();
});

after(async () => {
  await browser?.close();
  await server?.close();
  if (dist) rmSync(dist, { recursive: true, force: true });
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

/** A fresh tab, in a window of its own (its own OPFS), whose SDK worker keeps its core's memory. */
const tab = async () => {
  const page = await (await browser.newContext()).newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error));
  await page.goto(`${server.url}/`);
  await page.waitForFunction(() => window.ready === true);
  await page.evaluate(
    ({ MODEL, PATIENCE }) => {
      window.model = new Function("nc", `return ${MODEL}`)(window.nc);
      /** Polls `check` until it gives a value. */
      window.until = async (what, check) => {
        const start = Date.now();
        for (;;) {
          const got = await check();
          if (got) return got;
          if (Date.now() - start > PATIENCE) {
            const sync = JSON.stringify(window.db?.sync.current());
            throw new Error(`never: ${what} (sync: ${sync})`);
          }
          await new Promise((resolve) => setTimeout(resolve, 200));
        }
      };
    },
    { MODEL, PATIENCE },
  );
  return { page, errors };
};

/** The size of the WebAssembly memory of the core in `page`'s SDK worker, in bytes. */
const coreMemory = async (page) => {
  const worker = page.workers().find((w) => w.url().endsWith("/worker.js"));
  assert.ok(worker, "the SDK's worker runs");
  const size = await worker.evaluate(() => self.coreMemory?.buffer.byteLength);
  assert.ok(size > 0, "the probe found the core's WebAssembly memory");
  return size;
};

const mib = (bytes) => (bytes / (1024 * 1024)).toFixed(0);

test("a change at the 16 MiB limit fits in 256 MiB of WebAssembly memory when written and when received in Chromium", async () => {
  const { page: laptop, errors } = await tab();
  const invitation = await laptop.evaluate(async (lock) => {
    const { nc, model } = window;
    const db = await nc.open({ model, lock, name: "laptop" });
    const invitation = String(await db.invitation());
    await db.close();
    return invitation;
  }, lock);
  // The relay lets the laptop's account create spaces.
  const relay = await startRelay(["--allow-invitation", invitation]);
  try {
    const written = await laptop.evaluate(
      async ({ lock, url, records }) => {
        const { nc, model } = window;
        window.db = await nc.open({ model, lock, name: "laptop", relay: url });
        const space = await window.db.spaces.create();
        // Connected first, with the space at the relay, so that the
        // change's upload is all the tab waits for.
        await window.until(
          "the relay holds the space",
          () => window.db.sync.current().unconfirmed === 0,
        );
        // The records of examples/bulk_import of the crate nocentraldb.
        const { ids } = await space.write((tx) => {
          for (let i = 0; i < records; i++) {
            tx.create(model.tasks, {
              title: `Imported task ${String(i).padStart(6, "0")}: follow up on the old record`,
              status: i % 3 === 0 ? "done" : "open",
              priority: i % 5,
              estimate: 0.5 * (i % 8),
              urgent: i % 10 === 0,
              tags: nc.add("imported", i % 2 === 0 ? "even" : "odd"),
            });
          }
        });
        await window.until(
          "the relay holds the change",
          () => window.db.sync.current().unconfirmed === 0,
        );
        return {
          space: space.id,
          last: ids[ids.length - 1],
          refused: window.db.sync.current().refused,
        };
      },
      { lock, url: relay.url, records: RECORDS },
    );
    assert.equal(written.refused, 0, "the relay took the change");
    const writer = await coreMemory(laptop);

    const { page: phone, errors: phoneErrors } = await tab();
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
        return String(await window.db.invitation());
      },
      { lock, url: relay.url },
    );
    const answer = await laptop.evaluate(async (asked) => {
      const { nc } = window;
      const { answer } = await window.db.account.pair({
        kind: "device",
        invitation: nc.invitation(asked),
      });
      return String(answer);
    }, asked);
    await phone.evaluate(
      async ({ answer, space, last }) => {
        const { nc, model } = window;
        await window.db.account.completePairing(nc.pairingAnswer(answer));
        const held = await window.until("the phone holds the space", () =>
          window.db.spaces.get(space).catch(() => null),
        );
        await window.until("the phone received the change", () =>
          held.get(model.tasks, last),
        );
      },
      { answer, ...written },
    );
    const receiver = await coreMemory(phone);

    console.log(
      `a change of ${RECORDS} creates: writer ${mib(writer)} MiB, receiver ${mib(receiver)} MiB of WebAssembly memory, budget ${mib(BUDGET)} MiB`,
    );
    assert.ok(
      writer <= BUDGET,
      `writing took ${mib(writer)} MiB of WebAssembly memory, above the budget of ${mib(BUDGET)} MiB`,
    );
    assert.ok(
      receiver <= BUDGET,
      `receiving took ${mib(receiver)} MiB of WebAssembly memory, above the budget of ${mib(BUDGET)} MiB`,
    );
    assert.deepEqual(errors, []);
    assert.deepEqual(phoneErrors, []);
  } finally {
    relay.stop();
  }
});
