// NOC-344: the TypeScript SDK's browser tests (scripts/sdk-browser/) run in
// the engine SDK_BROWSER_ENGINE names, Chromium when it names none, and
// fail at once on an engine Playwright does not have, so that a typo in the
// workflow cannot quietly run Chromium a second time.
import assert from "node:assert/strict";
import { test } from "node:test";
import { engineFrom } from "./sdk-browser/engine.mjs";

test("no SDK_BROWSER_ENGINE means Chromium", () => {
  assert.equal(engineFrom({}), "chromium");
});

test("SDK_BROWSER_ENGINE picks Firefox or WebKit", () => {
  assert.equal(engineFrom({ SDK_BROWSER_ENGINE: "firefox" }), "firefox");
  assert.equal(engineFrom({ SDK_BROWSER_ENGINE: "webkit" }), "webkit");
});

test("an engine Playwright does not have fails, naming the ones it has", () => {
  assert.throws(
    () => engineFrom({ SDK_BROWSER_ENGINE: "safari" }),
    /safari.*chromium, firefox, webkit/,
  );
});
