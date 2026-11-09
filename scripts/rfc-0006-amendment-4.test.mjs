// Asserts RFC 0006 Amendment 4 (NOC-58): NOC-58 measured that IndexedDB, and
// the member key in it, survives a reload in a private window in Playwright's
// Chromium, Firefox and WebKit. The amendment updates the facts in the
// "Private windows" finding and in decision 12's recommendation, records the
// owner's confirmation that decision 12 stays A, and decides nothing new.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { tableRows } from "./browser-storage/report.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0006-local-view.md";
const read = (path) => readFileSync(join(root, path), "utf8");

const CONFIRMED =
  "Confirmed (Carsten, 2026-11-09) after NOC-58's measurement: A stays.";

const RELOAD_ROWS = [
  ["Chromium (Playwright)", "off-the-record context"],
  ["Firefox (Playwright)", "private browsing mode"],
  ["WebKit (Playwright)", "ephemeral context"],
].map(([browser, mode]) => ({
  browser,
  behaviour: `Private window: survives a reload (${mode})`,
}));

/** The text of decision 12, up to the next numbered decision or heading. */
const decision12 = (rfc) => {
  const start = rfc.indexOf(
    "12. **The fallback in a private window, where OPFS is missing.**",
  );
  assert.ok(start >= 0, "no decision 12");
  const rest = rfc.slice(start);
  const end = rest.search(/\n(?:\d+\. \*\*|## )/);
  return end < 0 ? rest : rest.slice(0, end);
};

test("RFC 0006 announces Amendment 4 in its changelog, and its section decides nothing", () => {
  const rfc = read(RFC);
  const top = rfc.split(/^## /m)[0];
  assert.match(
    top,
    /^- \*\*2026-11-09\*\* · NOC-58 · \[Amendment 4: [^\]]+\]\(#amendment-4-[^)]+\)/m,
    "no changelog line for Amendment 4",
  );
  const section = rfc
    .split(/^(?=## )/m)
    .find((s) => s.startsWith("## Amendment 4: "));
  assert.ok(section, "no section Amendment 4");
  assert.match(section, /decides nothing/, "Amendment 4 must decide nothing");
  assert.ok(
    section.includes(CONFIRMED),
    "Amendment 4 does not record the owner's confirmation",
  );
  assert.match(
    rfc,
    /\*\(Amendment 4\.\)\*/,
    "no sentence marked *(Amendment 4.)*",
  );
});

test("the Private windows finding cites the measured reload in each engine", () => {
  const rfc = read(RFC);
  const finding = rfc
    .split("\n")
    .find((line) =>
      line.startsWith(
        "- **Private windows.** Firefox and WebKit have no OPFS in a private window (`getDirectory()` fails)",
      ),
    );
  assert.ok(finding, "no Private windows finding in Amendment 2");
  assert.ok(
    !/lasts within one open private session in Firefox is unknown/.test(
      finding,
    ),
    "the finding still calls a reload in one private session unknown",
  );
  assert.match(finding, /\*\(Amendment 4\.\)\*/);
  const findings = tableRows(read("docs/browser-storage.md"), "Findings");
  for (const { browser, behaviour } of RELOAD_ROWS) {
    assert.ok(
      finding.includes(`measured: ${browser} · ${behaviour}`),
      `the finding does not cite ${browser} · ${behaviour}`,
    );
    const row = findings.find(
      (r) => r.browser === browser && r.behaviour === behaviour,
    );
    assert.ok(row, `the findings have no row ${browser} · ${behaviour}`);
    assert.match(
      row.source,
      /^measured: /,
      `${browser} · ${behaviour} is not measured`,
    );
  }
});

test("decision 12 states the measured facts and stays decided as A, confirmed by the owner", () => {
  const text = decision12(read(RFC));
  assert.ok(
    !/whether IndexedDB survives a reload in a private window is unknown/.test(
      text,
    ),
    "decision 12 still calls a reload in a private window unknown",
  );
  assert.ok(
    !/if IndexedDB does not survive a reload in a private window, each reload is a new device/.test(
      text,
    ),
    "decision 12 still doubts that the member key survives a reload",
  );
  assert.match(
    text,
    /Recommendation: A for v1\./,
    "the recommendation's conclusion changed",
  );
  assert.match(
    text,
    /Decided \(Carsten, 2026-11-09\): the recommendation, \(A\)\./,
    "the decision line changed",
  );
  assert.ok(
    text.includes(CONFIRMED),
    "decision 12 does not record the owner's confirmation",
  );
  assert.match(text, /NOC-58/, "decision 12 does not name the measurement");
});
