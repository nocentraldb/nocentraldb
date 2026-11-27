// End to end (NOC-571): `nc.open({ relay: () => Promise<string> })` in a
// browser, with the real core in its worker (RFC 0009, "Sync and status";
// NOC-496). The core asks for the relay URL with its first `relay` event
// while the database opens, before `connect` listens: that event must not
// be lost, or the core waits for the URL and the page stays `connecting`.
// The function fails first, so the core reports `relay_url_failed`, asks
// again after a backoff and then connects with the URL it gets.
//
// Not part of `pnpm gates`, which runs without browsers: the task
// `repo:sdk-browser` and the workflow browser-view.yml run it, as
// sdk.e2e.test.mjs says.
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

const MODEL = `nc.model({
  tasks: nc.collection({ title: nc.string() }),
})`;

test("a relay function that fails first is called again and connects", async () => {
  const relay = await startRelay();
  try {
    const opened = await (await browser.newContext()).newPage();
    const errors = [];
    opened.on("pageerror", (error) => errors.push(error));
    await opened.goto(`${server.url}/`);
    await opened.waitForFunction(() => window.ready === true);
    const seen = await opened.evaluate(
      async ({ lock, MODEL, url }) => {
        const { nc } = window;
        const model = new Function("nc", `return ${MODEL}`)(nc);
        let calls = 0;
        const db = await nc.open({
          model,
          lock,
          name: "relay-function",
          relay: async () => {
            calls += 1;
            if (calls === 1) throw new Error("no token yet");
            return url;
          },
        });
        const errorsSeen = new Set();
        const connected = await new Promise((resolve) => {
          const timer = setTimeout(() => resolve(false), 30000);
          const check = (status) => {
            if (status.error) errorsSeen.add(status.error);
            if (status.connection !== "connected") return false;
            clearTimeout(timer);
            resolve(true);
            return true;
          };
          if (check(db.sync.current())) return;
          const stop = db.sync.subscribe((status) => {
            if (check(status)) queueMicrotask(() => stop());
          });
        });
        const status = db.sync.current();
        await db.close();
        return {
          connected,
          calls,
          errors: [...errorsSeen],
          connection: status.connection,
          relay: status.relay,
        };
      },
      { lock, MODEL, url: relay.url },
    );
    assert.equal(seen.connected, true, JSON.stringify(seen));
    assert.ok(seen.calls >= 2, `the function was called ${seen.calls} times`);
    assert.deepEqual(seen.errors, ["relay_url_failed"]);
    assert.equal(seen.relay, relay.url);
    assert.deepEqual(errors, []);
  } finally {
    relay.stop();
  }
});
