// Asserts the browser storage findings of NOC-33: the rows that a probe run
// renders (scripts/browser-storage/report.mjs), and the table in
// docs/browser-storage.md, which may call a row "measured" only when the
// committed run holds it. Runs in `pnpm gates` without a browser.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { build } from "./browser-storage/build.mjs";
import { measuredRows, tableRows } from "./browser-storage/report.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

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
    "SQLite: no OPFS, IndexedDB lost",
  );
});

test("after closing a private window without OPFS, SQLite reads as no OPFS", () => {
  const rows = measuredRows(
    run({
      webkit: engine({
        private: {
          opfs: "UnknownError: failed",
          idb: "ok",
          afterClose: { sqlite: "error: UnknownError: failed", idb: "lost" },
        },
      }),
    }),
  );
  assert.equal(
    find(rows, "WebKit (Playwright)", "Private window: after close").finding,
    "SQLite: no OPFS, IndexedDB lost",
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

// docs/browser-storage.md: one table, every row sourced, and a row is
// "measured" by Playwright only when the committed run holds it.

const DOC = "docs/browser-storage.md";
const RESULTS = "docs/browser-storage.results.json";
const doc = () => readFileSync(join(root, DOC), "utf8");
const findings = () => tableRows(doc(), "Findings");
const results = () => JSON.parse(readFileSync(join(root, RESULTS), "utf8"));
const key = (row) => `${row.browser} | ${row.behaviour}`;

test("the findings table has a source for every row", () => {
  const rows = findings();
  assert.ok(rows.length > 0, `${DOC} has no rows under "Findings"`);
  for (const row of rows) {
    assert.deepEqual(
      Object.keys(row),
      ["browser", "behaviour", "finding", "source"],
      key(row),
    );
    assert.match(
      row.source,
      /^(measured: |documented: |unknown$)/,
      `${key(row)}: the source is measured, documented or unknown`,
    );
    if (row.source.startsWith("documented: ")) {
      assert.match(
        row.source,
        /\]\(https:\/\/[^)]+\)/,
        `${key(row)}: a documented row links to the vendor's documentation`,
      );
    }
    if (row.source === "unknown") {
      assert.equal(row.finding, "unknown", `${key(row)}: no guess`);
    }
  }
});

test("every Playwright row in the doc is the committed run, and all of it", () => {
  const expected = measuredRows(results()).map(
    (row) => `${key(row)} | ${row.finding} | ${row.source}`,
  );
  const actual = findings()
    .filter((row) => row.source.startsWith("measured: Playwright"))
    .map((row) => `${key(row)} | ${row.finding} | ${row.source}`);
  assert.deepEqual(actual.toSorted(), expected.toSorted());
});

test("a measured row that Playwright did not measure names a manual check", () => {
  const log = doc();
  for (const row of findings()) {
    if (!row.source.startsWith("measured: ")) continue;
    if (row.source.startsWith("measured: Playwright")) continue;
    const match = row.source.match(
      /^measured: manual check (\d{4}-\d{2}-\d{2})/,
    );
    assert.ok(match, `${key(row)}: "measured: manual check <date>"`);
    assert.ok(
      log.includes(`### ${match[1]}`),
      `${key(row)}: the manual check log has an entry for ${match[1]}`,
    );
  }
});

test("Playwright's WebKit is never reported as Safari", () => {
  for (const row of findings()) {
    if (/safari/i.test(row.browser)) {
      assert.doesNotMatch(row.source, /Playwright/, key(row));
    }
    if (/webkit/i.test(row.browser)) {
      assert.equal(row.browser, "WebKit (Playwright)", key(row));
    }
  }
});

test("the doc has a row for each check that needs a person", () => {
  const keys = findings().map(key);
  for (const wanted of [
    "Safari (macOS) | Seven-day deletion of script-writable storage",
    "Safari (macOS) | `persist()` prevents the seven-day deletion",
    "Safari (iOS) | Seven-day deletion of script-writable storage",
    "Safari (iOS) | `persist()` prevents the seven-day deletion",
    "Safari (iOS) | Add to Home Screen prevents the seven-day deletion",
    "Chrome (Android) | Eviction under storage pressure",
    "Edge (Android) | Eviction under storage pressure",
    "Firefox | `persist()`",
  ]) {
    assert.ok(keys.includes(wanted), `${DOC} has no row "${wanted}"`);
  }
});

test("the doc says how to run each check by hand", () => {
  const sections = doc()
    .split("\n")
    .filter((line) => /^#{2,3} /.test(line))
    .map((line) => line.replace(/^#+ /, "").toLowerCase());
  for (const wanted of [
    "findings",
    "how it was measured",
    "manual checks",
    "manual check log",
  ]) {
    assert.ok(sections.includes(wanted), `${DOC} has no section "${wanted}"`);
  }
});

// The static bundle: what the owner puts on any HTTPS host for the manual
// checks on a phone, without a local setup.

test("the static bundle holds the page and the SQLite build, nothing else", async () => {
  const out = await mkdtemp(join(tmpdir(), "ncdb-probe-build-"));
  try {
    const files = await build(out);
    assert.deepEqual(files.toSorted(), [
      "index.html",
      "probe.js",
      "sqlite/index.mjs",
      "sqlite/sqlite3.wasm",
      "worker.js",
    ]);
    for (const file of ["index.html", "probe.js", "worker.js"]) {
      assert.deepEqual(
        await readFile(join(out, file)),
        await readFile(join(root, "scripts/browser-storage/page", file)),
        file,
      );
    }
    assert.deepEqual(
      await readFile(join(out, "sqlite/sqlite3.wasm")),
      await readFile(
        join(root, "node_modules/@sqlite.org/sqlite-wasm/dist/sqlite3.wasm"),
      ),
    );
  } finally {
    await rm(out, { recursive: true, force: true });
  }
});

test("the checklist covers every device the owner checks", () => {
  const checklist = readFileSync(
    join(root, "docs/browser-storage-checklist.md"),
    "utf8",
  );
  const headings = checklist
    .split("\n")
    .filter((line) => /^## /.test(line))
    .map((line) => line.slice(3));
  for (const wanted of [
    /^Safari on macOS/,
    /^Safari on iOS, in the browser/,
    /^Safari on iOS, as a home-screen app/,
    /^Chrome on Android/,
    /^Firefox/,
  ]) {
    assert.ok(
      headings.some((heading) => wanted.test(heading)),
      `the checklist has no section matching ${wanted}`,
    );
  }
  assert.match(checklist, /Day 0/);
  assert.match(checklist, /Day 8/);
  assert.match(checklist, /takes about \d+ min/);
});
