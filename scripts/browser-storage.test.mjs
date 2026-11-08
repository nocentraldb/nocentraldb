// Asserts the browser storage findings of NOC-33: the rows that a probe run
// renders (scripts/browser-storage/report.mjs), and the table in
// docs/browser-storage.md, which may call a row "measured" only when the
// committed run holds it. Runs in `pnpm gates` without a browser.
import assert from "node:assert/strict";
import { test } from "node:test";
import { measuredRows, tableRows } from "./browser-storage/report.mjs";

const engine = (overrides = {}) => ({
  version: "1.0",
  persist: { answered: true, granted: false },
  persisted: false,
  estimate: { usage: 2048, quota: 10 * 1024 ** 3 },
  opfs: "ok",
  reload: { sqlite: "kept", idb: "kept" },
  restart: { sqlite: "kept", idb: "kept" },
  clearCache: { sqlite: "kept", idb: "kept" },
  private: {
    opfs: "ok",
    idb: "ok",
    afterClose: { sqlite: "lost", idb: "lost" },
  },
  ...overrides,
});

const run = (engines) => ({
  date: "2026-11-08",
  platform: "linux x64",
  playwright: "1.63.0",
  engines,
});

const find = (rows, browser, behaviour) =>
  rows.find(
    (row) => row.browser === browser && row.behaviour.startsWith(behaviour),
  );

test("each engine gets a row per behaviour, labelled as Playwright's build", () => {
  const rows = measuredRows(
    run({ chromium: engine(), firefox: engine(), webkit: engine() }),
  );
  for (const browser of [
    "Chromium (Playwright)",
    "Firefox (Playwright)",
    "WebKit (Playwright)",
  ]) {
    for (const behaviour of [
      "`persist()`",
      "`persisted()`",
      "`estimate()`",
      "SQLite on OPFS",
      "Survives a reload",
      "Survives a browser restart",
      'Survives `Clear-Site-Data: "cache"`',
      "Private window: storage",
      "Private window: after close",
    ]) {
      assert.ok(find(rows, browser, behaviour), `${browser}: ${behaviour}`);
    }
  }
  assert.ok(rows.every((row) => !row.browser.startsWith("Safari")));
});

test("a measured row names the run it comes from", () => {
  const rows = measuredRows(run({ webkit: engine({ version: "26.4" }) }));
  for (const row of rows) {
    assert.equal(
      row.source,
      "measured: Playwright 1.63.0, WebKit 26.4, linux x64, 2026-11-08",
    );
  }
});

test("persist() says whether it answered without the user", () => {
  const rows = measuredRows(
    run({
      chromium: engine({ persist: { answered: true, granted: true } }),
      firefox: engine({ persist: { answered: false, granted: null } }),
      webkit: engine(),
    }),
  );
  assert.equal(
    find(rows, "Chromium (Playwright)", "`persist()`").finding,
    "granted, without asking the user",
  );
  assert.equal(
    find(rows, "Firefox (Playwright)", "`persist()`").finding,
    "no answer within 5 s: the browser waits for the user",
  );
  assert.equal(
    find(rows, "WebKit (Playwright)", "`persist()`").finding,
    "denied, without asking the user",
  );
});

test("survival rows report SQLite on OPFS and the IndexedDB control", () => {
  const rows = measuredRows(
    run({
      chromium: engine({
        restart: { sqlite: "lost", idb: "kept" },
        estimate: { usage: 1536, quota: 1.5 * 1024 ** 3 },
      }),
    }),
  );
  assert.equal(
    find(rows, "Chromium (Playwright)", "Survives a browser restart").finding,
    "SQLite lost, IndexedDB kept",
  );
  assert.equal(
    find(rows, "Chromium (Playwright)", "`estimate()`").finding,
    "usage 1.5 KiB, quota 1.5 GiB",
  );
});

test("a private window without OPFS shows the browser's error", () => {
  const rows = measuredRows(
    run({
      firefox: engine({
        private: {
          opfs: "SecurityError: The operation is insecure.",
          idb: "ok",
          afterClose: { sqlite: "lost", idb: "lost" },
        },
      }),
    }),
  );
  assert.equal(
    find(rows, "Firefox (Playwright)", "Private window: storage").finding,
    "no SQLite on OPFS (SecurityError: The operation is insecure.); IndexedDB works",
  );
  assert.equal(
    find(rows, "Firefox (Playwright)", "Private window: after close").finding,
    "SQLite lost, IndexedDB lost",
  );
});

test("tableRows reads the first table under a heading", () => {
  const markdown = [
    "# Doc",
    "## Other",
    "| A | B |",
    "| - | - |",
    "| x | y |",
    "## Findings",
    "Text.",
    "",
    "| Browser | Behaviour | Finding | Source |",
    "| --- | --- | --- | --- |",
    "| Firefox | `persist()` | a \\| b | unknown |",
    "",
    "| Not | This |",
  ].join("\n");
  assert.deepEqual(tableRows(markdown, "Findings"), [
    {
      browser: "Firefox",
      behaviour: "`persist()`",
      finding: "a | b",
      source: "unknown",
    },
  ]);
});
