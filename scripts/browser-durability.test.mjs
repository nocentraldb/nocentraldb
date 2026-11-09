// Asserts the proposal of NOC-34: the developer page "Browser apps and
// durability" says nothing NOC-33 did not find. Every row of its tables names,
// in its Source cell, the rows of the findings table in
// docs/browser-storage.md it rests on, with the same kind of source
// (measured, documented or unknown), so a statement cannot claim more than
// its row. Runs in `pnpm gates` without a browser.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { tableRows } from "./browser-storage/report.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const PAGE = "docs/browser-durability.md";
const FINDINGS = "docs/browser-storage.md";

const read = (path) => readFileSync(join(root, path), "utf8");

/** The text of the level-2 section titled `title`, without its heading. */
const section = (markdown, title) => {
  const part = markdown
    .split(/^(?=## )/m)
    .find((chunk) => chunk.split("\n")[0].trim() === `## ${title}`);
  return part?.split("\n").slice(1).join("\n");
};

/** The kind of a findings row's source: measured, documented or unknown. */
const kind = (source) => source.match(/^(measured|documented|unknown)/)?.[1];

/**
 * The references in a Source cell: "kind: Browser · Behaviour", several
 * separated by "; ". Anything else in the cell is not a reference.
 */
const references = (cell) =>
  cell
    .split(/;\s*/)
    .map((part) => part.trim())
    .filter(Boolean)
    .map((part) => {
      const match = part.match(/^(measured|documented|unknown): (.+?) · (.+)$/);
      return match
        ? { kind: match[1], browser: match[2], behaviour: match[3] }
        : { invalid: part };
    });

/** Every traced table on the page: those with a Source column. */
const tracedTables = () => {
  const page = read(PAGE);
  const titles = [...page.matchAll(/^## (.+)$/gm)].map((match) => match[1]);
  return titles
    .map((title) => {
      const body = section(page, title);
      if (!/^\| .* \| Source \|$/m.test(body ?? "")) return undefined;
      return { title, rows: tableRows(page, title) };
    })
    .filter(Boolean);
};

test("the page Browser apps and durability exists", () => {
  assert.ok(existsSync(join(root, PAGE)), `${PAGE} is missing`);
  assert.match(read(PAGE), /^# Browser apps and durability$/m);
});

test("the page has the sections the ticket asks for", () => {
  const page = read(PAGE);
  for (const title of [
    "Each browser in plain words",
    "What is lost, and when",
    "How to reduce the risk",
    "Private windows",
    "What the SDK tells you",
    "Text for your users",
  ]) {
    assert.ok(section(page, title) !== undefined, `${PAGE}: no "## ${title}"`);
  }
});

test("the page's tables trace every row to the findings of NOC-33", () => {
  const tables = tracedTables();
  for (const title of [
    "Each browser in plain words",
    "What is lost, and when",
    "How to reduce the risk",
  ]) {
    assert.ok(
      tables.some((table) => table.title === title),
      `${PAGE}: "${title}" has no table with a Source column`,
    );
  }
  const findings = tableRows(read(FINDINGS), "Findings");
  for (const { title, rows } of tables) {
    assert.ok(rows.length > 0, `${PAGE}: "${title}" has no rows`);
    for (const row of rows) {
      const refs = references(row.source);
      const label = `${PAGE}, "${title}": ${Object.values(row)[0]}`;
      assert.ok(refs.length > 0, `${label}: the Source cell names no row`);
      for (const ref of refs) {
        assert.ok(!ref.invalid, `${label}: "${ref.invalid}" is no reference`);
        const found = findings.find(
          (finding) =>
            finding.browser === ref.browser &&
            finding.behaviour === ref.behaviour,
        );
        assert.ok(
          found,
          `${label}: ${FINDINGS} has no row "${ref.browser} · ${ref.behaviour}"`,
        );
        assert.equal(
          kind(found.source),
          ref.kind,
          `${label}: "${ref.browser} · ${ref.behaviour}" is ${kind(found.source)}`,
        );
      }
    }
  }
});

test("a row that rests only on unknown findings says it is unknown", () => {
  for (const { title, rows } of tracedTables()) {
    for (const row of rows) {
      const refs = references(row.source);
      if (refs.length === 0 || refs.some((ref) => ref.kind !== "unknown")) {
        continue;
      }
      const statement = Object.entries(row)
        .filter(([column]) => column !== "source")
        .map(([, value]) => value)
        .join(" ");
      assert.match(
        statement,
        /\bunknown\b/i,
        `${PAGE}, "${title}": ${Object.values(row)[0]} rests only on unknown rows`,
      );
    }
  }
});

// "Private windows" is prose: its references stand in parentheses, as
// "(measured: Browser · Behaviour; …)", and resolve like a Source cell's.
const proseReferences = (text) =>
  [
    ...text.matchAll(
      /\b(measured|documented|unknown): ([^·;]+?) · (.+?)(?=; (?:measured|documented|unknown): |\)(?:[.,:]|\s|$))/gm,
    ),
  ].map((match) => ({
    kind: match[1],
    browser: match[2],
    behaviour: match[3],
  }));

test("Private windows cites the measured reload in each engine, and each reference resolves", () => {
  const text = section(read(PAGE), "Private windows") ?? "";
  const refs = proseReferences(text);
  const findings = tableRows(read(FINDINGS), "Findings");
  for (const ref of refs) {
    const found = findings.find(
      (finding) =>
        finding.browser === ref.browser && finding.behaviour === ref.behaviour,
    );
    assert.ok(
      found,
      `${PAGE}, "Private windows": ${FINDINGS} has no row "${ref.browser} · ${ref.behaviour}"`,
    );
    assert.equal(
      kind(found.source),
      ref.kind,
      `${PAGE}, "Private windows": "${ref.browser} · ${ref.behaviour}" is ${kind(found.source)}`,
    );
  }
  for (const [engine, mode] of [
    ["Chromium", "off-the-record context"],
    ["Firefox", "private browsing mode"],
    ["WebKit", "ephemeral context"],
  ]) {
    const behaviour = `Private window: survives a reload (${mode})`;
    assert.ok(
      refs.some(
        (ref) =>
          ref.kind === "measured" &&
          ref.browser === `${engine} (Playwright)` &&
          ref.behaviour === behaviour,
      ),
      `${PAGE}, "Private windows" does not cite "${engine} (Playwright) · ${behaviour}"`,
    );
  }
  assert.doesNotMatch(
    text,
    /lasts within one open private session is unknown/,
    `${PAGE}, "Private windows": a reload in one private session is measured now`,
  );
});

// The text for end users: short, with placeholders the developer fills, and
// no word that promises more than the findings. "Tested" here means checked by
// this gate; no user has read the text yet, and the page says so.
const PROMISES =
  /\b(never|always|guarantee[sd]?|safe(ly)?|secure(ly)?|forever|permanent(ly)?|cannot be lost)\b/i;
const MAX_WORDS = 60;

/** The ```text blocks of a level-3 section under "Text for your users". */
const userTexts = (title) => {
  const part = (section(read(PAGE), "Text for your users") ?? "")
    .split(/^(?=### )/m)
    .find((chunk) => chunk.split("\n")[0].trim() === `### ${title}`);
  return [...(part ?? "").matchAll(/```text\n([\s\S]*?)```/g)].map((match) =>
    match[1].trim(),
  );
};

const TEXTS = ["Onboarding", "Changes not yet saved", "Private window"];

test("the page offers each text for users", () => {
  for (const title of TEXTS) {
    assert.ok(userTexts(title).length > 0, `${PAGE}: no text "${title}"`);
  }
});

test("the text for unsaved changes has a form for one change and for more", () => {
  const texts = userTexts("Changes not yet saved");
  assert.ok(
    texts.some((text) => /^1 change\b/.test(text) && !text.includes("{count}")),
    `${PAGE}: no text that starts "1 change"`,
  );
  assert.ok(
    texts.some((text) => /^\{count\} changes\b/.test(text)),
    `${PAGE}: no text that starts "{count} changes"`,
  );
});

test("every text for users is short, names the app and promises nothing", () => {
  for (const title of TEXTS) {
    for (const text of userTexts(title)) {
      const label = `${PAGE}, "${title}": ${text.slice(0, 40)}…`;
      assert.ok(text.includes("{appName}"), `${label}: no {appName}`);
      const words = text.split(/\s+/).length;
      assert.ok(words <= MAX_WORDS, `${label}: ${words} words`);
      assert.doesNotMatch(text, PROMISES, `${label}: promises too much`);
      for (const [placeholder] of text.matchAll(/\{[^}]*\}/g)) {
        assert.ok(
          ["{appName}", "{count}"].includes(placeholder),
          `${label}: unknown placeholder ${placeholder}`,
        );
      }
    }
  }
});

// Where the developer meets the proposal: RFC 0006 carries it as Amendment 2,
// with a decision per layer, and RFC 0001 links the page. The README does not:
// the owner keeps limits out of it (NOC-61).
const RFC_0006 = "docs/rfc/0006-local-view.md";
const RFC_0001 = "docs/rfc/0001-nocentraldb.md";
const AMENDMENT = "Amendment 2: durability of a browser copy";
const ANCHOR = "amendment-2-durability-of-a-browser-copy";

/** GitHub's heading anchor: lower case, punctuation dropped, spaces to dashes. */
const slug = (heading) =>
  heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s_-]/gu, "")
    .replace(/\s/g, "-");

/** The relative links of a Markdown file outside code blocks, resolved. */
const relativeLinks = (path) =>
  [
    ...read(path)
      .replace(/```[\s\S]*?```/g, "")
      .matchAll(/\]\(([^)\s]+)\)/g),
  ]
    .map(([, target]) => target)
    .filter((target) => !/^[a-z]+:/.test(target))
    .map((target) => {
      const [file, anchor] = target.split("#");
      const resolved = file ? join(dirname(path), file) : path;
      return { target, resolved, anchor };
    });

const headingsOf = (path) =>
  new Set(
    read(path)
      .replace(/```[\s\S]*?```/g, "")
      .split("\n")
      .filter((line) => /^#{1,6} /.test(line))
      .map((line) => slug(line.replace(/^#+ /, ""))),
  );

test("every relative link on the page resolves, to its heading too", () => {
  for (const { target, resolved, anchor } of relativeLinks(PAGE)) {
    assert.ok(
      existsSync(join(root, resolved)),
      `${PAGE}: ${target} is missing`,
    );
    if (anchor && resolved.endsWith(".md")) {
      assert.ok(
        headingsOf(resolved).has(anchor),
        `${PAGE}: ${target} names no heading`,
      );
    }
  }
});

test("RFC 0006 announces Amendment 2 in a dated changelog line at the top", () => {
  const top = read(RFC_0006).split(/^## /m)[0];
  assert.match(
    top,
    new RegExp(
      `^- \\*\\*\\d{4}-\\d{2}-\\d{2}\\*\\* · NOC-34 · \\[Amendment 2[^\\]]*\\]\\(#${ANCHOR}\\)`,
      "m",
    ),
  );
});

test("RFC 0006 says how to give feedback, above its first section", () => {
  const top = read(RFC_0006).split(/^## /m)[0];
  const line = top
    .split("\n")
    .find((l) => l.startsWith("**How to give feedback.**"));
  assert.ok(line, `${RFC_0006}: no "**How to give feedback.**" line`);
  assert.match(line, /\]\(README\.md#giving-feedback\)/);
  assert.match(line, /discussions\/categories\/rfc-feedback/);
});

test("Amendment 2 decides each layer with options and a recommendation", () => {
  const amendment = section(read(RFC_0006), AMENDMENT);
  assert.ok(amendment, `${RFC_0006}: no "## ${AMENDMENT}"`);
  const decisions = amendment
    .split(/^(?=\d+\. \*\*)/m)
    .filter((part) => /^\d+\. \*\*/.test(part));
  assert.ok(
    decisions.length >= 5,
    `${RFC_0006}: ${decisions.length} decisions`,
  );
  for (const decision of decisions) {
    const name = decision.split("\n")[0];
    assert.match(decision, /^\s+- A\. /m, `${name}: no option A`);
    assert.match(decision, /^\s+- B\. /m, `${name}: no option B`);
    assert.match(decision, /\*\(recommended\)\*/, `${name}: none recommended`);
    assert.match(
      decision,
      /^\s+Recommendation: /m,
      `${name}: no recommendation`,
    );
  }
  for (const layer of [
    /db\.durability/,
    /browser-durability\.md/,
    /\{appName\}/,
    /warning/i,
    /IndexedDB/,
  ]) {
    assert.match(amendment, layer, `${RFC_0006}: Amendment 2 lacks ${layer}`);
  }
});

test("RFC 0001 and RFC 0006 link the page", () => {
  for (const path of [RFC_0001, RFC_0006]) {
    assert.ok(
      relativeLinks(path).some(({ resolved }) => resolved === PAGE),
      `${path} does not link ${PAGE}`,
    );
  }
});

test("each decision of Amendment 2 records the owner's decision", () => {
  const amendment = section(read(RFC_0006), AMENDMENT) ?? "";
  const decisions = amendment
    .split(/^(?=\d+\. \*\*)/m)
    .filter((part) => /^\d+\. \*\*/.test(part));
  for (const decision of decisions) {
    assert.match(
      decision,
      /^\s+Decided \(Carsten, \d{4}-\d{2}-\d{2}\): the recommendation, \([A-C]\)\./m,
      `${decision.split("\n")[0]}: not decided`,
    );
  }
  assert.doesNotMatch(read(RFC_0006), /wait for the owner/);
});
