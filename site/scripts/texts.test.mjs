// Checks the built site's texts against what release 0.1 ships (NOC-360) and
// against the glossary in docs/glossary.md: no page calls a shipped part
// "designed" or "not built", every page uses the glossary's terms, and the
// docs name the limits of 0.1.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const dist = join(dirname(fileURLToPath(import.meta.url)), "..", "dist");

/** The visible text of a built page: tags, scripts and styles removed. */
const text = (path) =>
  readFileSync(join(dist, path), "utf8")
    .replace(/<(script|style)[^>]*>[\s\S]*?<\/\1>/g, " ")
    .replace(/<[^>]*>/g, " ")
    .replace(/&#39;/g, "'")
    .replace(/&quot;/g, '"')
    .replace(/&amp;/g, "&")
    .replace(/\s+/g, " ");

/** Every built docs page, relative to dist. */
const docsPages = (dir = "docs") =>
  readdirSync(join(dist, dir), { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return docsPages(path);
    return entry.name === "index.html" ? [path] : [];
  });

test("no docs page says that 0.1's parts are only designed", () => {
  for (const page of docsPages()) {
    const words = text(page);
    for (const stale of [
      "does not exist yet",
      "Not yet a product",
      "designed, not built",
      "once the SDK exists",
      "None of it is released yet",
      "is designed to",
      "are designed to",
    ])
      assert.ok(!words.includes(stale), `${page}: "${stale}"`);
  }
});

test("getting started says what 0.1 is and links both guides", () => {
  const words = text("docs/index.html");
  for (const fact of [
    "0.1",
    "Chromium, Firefox and Safari",
    "storageUnavailable",
    "waitingForTab",
  ])
    assert.ok(words.includes(fact), fact);
  const html = readFileSync(join(dist, "docs/index.html"), "utf8");
  for (const guide of [
    "docs/getting-started.md",
    "docs/getting-started-browser.md",
  ])
    assert.ok(html.includes(`blob/main/${guide}"`), guide);
});

test("the RFC list holds every RFC in docs/rfc", () => {
  const rfcs = readdirSync(join(dist, "..", "..", "docs", "rfc")).filter((f) =>
    /^\d{4}-.*\.md$/.test(f),
  );
  const page = readFileSync(join(dist, "docs/rfcs/index.html"), "utf8");
  for (const rfc of rfcs) assert.ok(page.includes(rfc), rfc);
});

test("the relay page names the two relays 0.1 ships", () => {
  const words = text("docs/concepts/relay/index.html");
  for (const fact of ["ncdb-relay", "SQLite", "AWS", "DynamoDB"])
    assert.ok(words.includes(fact), fact);
});

test("the docs use the glossary's terms", () => {
  for (const page of docsPages()) {
    const words = text(page);
    for (const avoid of [
      /\breplicas?\b/i,
      /\bsync server\b/i,
      /\bsubagents?\b/i,
      /\bmutations?\b/i,
      /\boffline mode\b/i,
    ])
      assert.ok(!avoid.test(words), `${page}: ${avoid}`);
  }
});
