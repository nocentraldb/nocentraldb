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
  for (const guide of ["/docs/guides/rust/", "/docs/guides/browser/"])
    assert.ok(html.includes(`href="${guide}"`), guide);
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
  const found = [];
  // The glossary itself names the words to avoid.
  for (const page of docsPages().filter(
    (p) => !p.startsWith("docs/glossary/"),
  )) {
    const words = text(page);
    for (const avoid of [
      /\breplicas?\b/i,
      /\bsync server\b/i,
      /\bsubagents?\b/i,
      /\bmutations?\b/i,
      /\boffline mode\b/i,
    ])
      if (avoid.test(words)) found.push(`${page}: ${avoid}`);
  }
  assert.deepEqual(found, []);
});

test("getting started says what work orders are, that 0.1 runs none, and that erasure has its own path", () => {
  const words = text("docs/index.html");
  assert.match(
    words,
    /Work orders\. Clean-up and maintenance jobs that one member does and another member checks/,
  );
  assert.match(
    words,
    /compaction, which moves old history into a signed, encrypted archive/,
  );
  assert.match(words, /0\.1 runs none of them/);
  assert.match(
    words,
    /Erasing a person, which also needs a second account['’]s sign-off, works in 0\.1 through its own path/,
  );
  const html = readFileSync(join(dist, "docs/index.html"), "utf8");
  assert.ok(
    html.includes('href="/docs/concepts/erasure/"'),
    "a link to the erasure page",
  );
});

test("How NoCentralDB works explains the core concepts on one page, with the RFC diagrams", () => {
  const html = readFileSync(join(dist, "docs/how-it-works/index.html"), "utf8");
  assert.match(html, /<h1[^>]*>How NoCentralDB works<\/h1>/);
  const words = text("docs/how-it-works/index.html");
  for (const concept of [
    "space",
    "member",
    "change",
    "relay",
    "view",
    "model.lock.json",
    "member key",
    "recovery code",
  ])
    assert.ok(words.includes(concept), concept);
  for (const page of [
    "/docs/concepts/spaces/",
    "/docs/concepts/relay/",
    "/docs/concepts/convergence/",
    "/docs/concepts/erasure/",
  ])
    assert.ok(html.includes(`href="${page}"`), page);
  const images = [
    ...html.matchAll(/<img[^>]*src="\/diagrams\/([^"]+)"[^>]*>/g),
  ];
  assert.ok(images.length >= 3, "the diagrams");
  for (const [img] of images) {
    const alt = img.match(/alt="([^"]*)"/)?.[1] ?? "";
    assert.ok(alt.length > 80, `an alt text that tells what it shows: ${img}`);
  }
});

test("the sidebar lists Overview right after Getting started, then the two guides", () => {
  const html = readFileSync(join(dist, "docs/index.html"), "utf8");
  const sidebar = html.slice(html.indexOf('class="sidebar-content'));
  const links = [
    ...sidebar.matchAll(
      /<a href="(\/docs\/[^"]*)"[^>]*>\s*<span[^>]*>([^<]*)</g,
    ),
  ].map((m) => m[2].trim());
  // Then the two guides, rendered from docs/ (the owner, 2026-11-19, NOC-361).
  assert.deepEqual(links.slice(0, 5), [
    "Getting started",
    "Overview",
    "Getting started in Rust",
    "Getting started in the browser",
    "Spaces and members",
  ]);
});

test("How NoCentralDB works runs one example, projects, tasks and people, through the decisions", () => {
  const words = text("docs/how-it-works/index.html");
  for (const decision of [
    "people",
    "projects",
    "tasks",
    "nc.link",
    "nc.ref",
    "Reference",
    "notFound",
    "reader",
    "writer",
    "admin",
    ".personal",
    "unset",
  ])
    assert.ok(words.includes(decision), decision);
  // what 0.1 cannot do yet is said on the page, not hidden
  assert.match(words, /0\.1 has no call that reports a reference['’]s state/);
  const md = readFileSync(
    join(dist, "..", "src", "content", "docs", "docs", "how-it-works.md"),
    "utf8",
  );
  assert.ok(
    (md.match(/^```rust no_run$/gm) ?? []).length >= 2,
    "Rust snippets",
  );
  const lib = readFileSync(
    join(dist, "..", "..", "crates", "nocentraldb", "src", "lib.rs"),
    "utf8",
  );
  assert.ok(
    lib.includes(
      'include_str!("../../../site/src/content/docs/docs/how-it-works.md")',
    ),
    "the doc tests compile the page's Rust snippets",
  );
});

test("the erasure page is at /docs/concepts/erasure/, and the old address redirects to it", () => {
  const page = readFileSync(
    join(dist, "docs/concepts/erasure/index.html"),
    "utf8",
  );
  assert.match(page, /<h1[^>]*>Erasing a person<\/h1>/);
  const old = readFileSync(
    join(dist, "docs/concepts/shredding/index.html"),
    "utf8",
  );
  assert.match(
    old,
    /http-equiv="refresh"[^>]*url=\/docs\/concepts\/erasure\//i,
  );
  for (const p of docsPages())
    if (!p.includes("shredding"))
      assert.ok(
        !readFileSync(join(dist, p), "utf8").includes(
          "/docs/concepts/shredding/",
        ),
        `${p} links the old address`,
      );
});
