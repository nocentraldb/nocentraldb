// Asserts that the texts a user reads first follow the owner's decisions in
// RFC 0003, RFC 0005 and RFC 0006 (NOC-35): the FAQ says what a lost device
// and a browser copy give away and keep, RFC 0001 says which fields can be
// personal, and every list of erasure limits names the app's duty for free
// text and that removing a member is not erasure. Each claim links its RFC
// section; whether the wording is right is the owner's review of the PR.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");

const RFC_0001 = "docs/rfc/0001-nocentraldb.md";
const FAQ = "docs/faq.md";
const USE_CASES = "docs/use-cases.md";
const GENESIS = "factory/GENESIS.md";

const PLAINTEXT = "0006-local-view.md#what-the-view-stores-in-plaintext";
const RFC_0006_DECISIONS = "0006-local-view.md#decisions-for-the-owner";
const RFC_0006_BROWSER = "0006-local-view.md#the-browser";
const REMOVING =
  "0003-keys-and-cryptography.md#removing-a-member-and-rotating-the-space-key";
const ACCOUNT_KEY = "0003-keys-and-cryptography.md#the-account-key";
const PERSONAL = "0005-schema-and-model-lock.md#personal-fields";
const BUILD_CHECKS = "0005-schema-and-model-lock.md#what-the-build-checks";
const FREE_TEXT = "0005-schema-and-model-lock.md#personal-data-in-free-text";
const NOT_ERASURE = "0005-schema-and-model-lock.md#about-whom-not-by-whom";

/** GitHub's heading anchor: lower case, punctuation dropped, spaces to dashes. */
const slug = (heading) =>
  heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s_-]/gu, "")
    .replace(/\s/g, "-");
const anchors = (markdown) =>
  new Set(
    markdown
      .split("\n")
      .filter((line) => /^#{1,6} /.test(line))
      .map((line) => slug(line.replace(/^#+ /, ""))),
  );

/** The text under the heading `heading`, up to the next heading of the same or a higher level. */
const section = (markdown, heading) => {
  const lines = markdown.split("\n");
  const start = lines.findIndex((line) => line.replace(/^#+ /, "") === heading);
  assert.ok(start >= 0, `no heading "${heading}"`);
  const level = lines[start].match(/^#+/)[0].length;
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((line) => {
    const m = line.match(/^(#+) /);
    return m && m[1].length <= level;
  });
  return (end < 0 ? rest : rest.slice(0, end)).join("\n");
};

/** The sentences of `text` that link a target ending with `target`. */
const sentencesLinking = (text, target) =>
  text
    .split(/(?<=[.!?])\s+(?=[A-Z[])/)
    .filter((s) =>
      [...s.matchAll(/\]\(([^)\s]+)\)/g)].some(([, t]) => t.endsWith(target)),
    );

const linkingSentence = (text, target, pattern, what) => {
  const found = sentencesLinking(text, target).find((s) => pattern.test(s));
  assert.ok(
    found,
    `${what}: no sentence linking ${target} that matches ${pattern}`,
  );
  return found;
};

/** The paragraph that starts with "**Status.**". */
const status = (markdown) =>
  markdown.split("\n").find((line) => line.startsWith("**Status.**")) ?? "";

const LOST_DEVICE = "What happens if I lose my device?";
const STORAGE = "Does every device have to store all the data?";
const GDPR = "Does destroying a key satisfy a GDPR erasure request?";

test("the FAQ says a lost device holds every space in plaintext, with RFC 0006's section", () => {
  const answer = section(read(FAQ), LOST_DEVICE);
  linkingSentence(answer, PLAINTEXT, /plaintext/i, "plaintext");
  linkingSentence(
    answer,
    PLAINTEXT,
    /unlocked.*(disk|storage)|(disk|storage).*unlocked/i,
    "unlocked device or its disk",
  );
});

test("the FAQ says v1 relies on the device's disk encryption, with RFC 0006's decision 6", () => {
  const answer = section(read(FAQ), LOST_DEVICE);
  linkingSentence(answer, RFC_0006_DECISIONS, /disk encryption/i, "decision 6");
  assert.match(answer, /decision 6/, "the FAQ does not name decision 6");
});

test("the FAQ says removing the member does not delete the lost device's copy, with RFC 0003's section", () => {
  const answer = section(read(FAQ), LOST_DEVICE);
  linkingSentence(answer, REMOVING, /does not delete/i, "removing a member");
});

test("the FAQ follows RFC 0003 on recovering the account after losing every device", () => {
  const answer = section(read(FAQ), LOST_DEVICE);
  assert.ok(
    !/passkey/i.test(answer),
    "RFC 0003 decided against a passkey in v1",
  );
  assert.ok(
    !/does not yet specify how a person regains access/.test(answer),
    "RFC 0003 specifies the recovery code",
  );
  linkingSentence(answer, ACCOUNT_KEY, /recovery code/i, "recovery code");
});

test("the FAQ says a browser may evict its copy, Safari after seven days, with RFC 0006", () => {
  const answer = section(read(FAQ), STORAGE);
  linkingSentence(
    answer,
    RFC_0006_BROWSER,
    /seven days/i,
    "Safari's seven-day rule",
  );
  linkingSentence(answer, "browser-durability.md", /./, "the durability page");
  linkingSentence(answer, "browser-storage.md", /./, "the measurements");
});

test("the FAQ says the SDK asks for persistent storage, with RFC 0006's decision 8", () => {
  const answer = section(read(FAQ), STORAGE);
  linkingSentence(
    answer,
    RFC_0006_DECISIONS,
    /persistent storage/i,
    "decision 8",
  );
  assert.match(answer, /decision 8/, "the FAQ does not name decision 8");
});

test("RFC 0001 says only basic values can be personal, and only when the field is added", () => {
  const line = read(RFC_0001)
    .split("\n")
    .find((l) => l.startsWith("- **Personal fields.**"));
  assert.ok(line, "RFC 0001 has no bullet Personal fields");
  assert.ok(
    !/Any field can be marked/i.test(line),
    "RFC 0001 still says any field",
  );
  const claim = linkingSentence(line, BUILD_CHECKS, /basic value/i, "D9");
  assert.match(
    claim,
    /when the field is added/,
    "D9: from the version that adds it",
  );
});

/** Each list of erasure limits, with the text that holds it. */
const erasureLists = () => [
  {
    what: "RFC 0001's team lead",
    text: section(
      read(RFC_0001),
      "The team lead who has to pass the data-protection review",
    ),
  },
  {
    what: "use case 1",
    text: section(read(USE_CASES), "1. Erasure that reaches every copy"),
  },
  { what: "the FAQ's GDPR answer", text: section(read(FAQ), GDPR) },
];

test("every list of erasure limits names the app's duty for free text, with RFC 0005's pattern", () => {
  for (const { what, text } of erasureLists()) {
    linkingSentence(text, FREE_TEXT, /reference/i, what);
  }
});

test("every list of erasure limits says removing a member is not erasure, with RFC 0005", () => {
  for (const { what, text } of erasureLists()) {
    linkingSentence(
      text,
      NOT_ERASURE,
      /removing a member is not (an )?erasure/i,
      what,
    );
  }
});

test("GENESIS.md's Shred and audit points to RFC 0005's pattern for free text", () => {
  const line = read(GENESIS)
    .split("\n")
    .find((l) => l.startsWith("- **Shred and audit.**"));
  assert.ok(line, "GENESIS.md has no bullet Shred and audit");
  assert.match(line, /unmarked free text/, "the limit is no longer named");
  linkingSentence(line, FREE_TEXT, /free text/i, "GENESIS.md");
});

test("RFC 0001 announces Amendment 4 in a dated NOC-35 changelog line, and its section decides nothing", () => {
  const rfc = read(RFC_0001);
  const top = rfc.split(/^## /m)[0];
  assert.match(
    top,
    /^- \*\*\d{4}-\d{2}-\d{2}\*\* · NOC-35 · \[Amendment 4: [^\]]+\]\(#amendment-4-[^)]+\)/m,
    "no changelog line for Amendment 4",
  );
  const heading = rfc.split("\n").find((l) => l.startsWith("## Amendment 4: "));
  assert.ok(heading, "no section Amendment 4");
  const body = section(rfc, heading.slice(3));
  assert.match(body, /decides nothing new/, "Amendment 4 must decide nothing");
  assert.ok(
    sentencesLinking(body, PERSONAL).length +
      sentencesLinking(body, FREE_TEXT).length >
      0,
  );
  assert.ok(
    (rfc.match(/\*\(Amendment 4\.\)\*/g) ?? []).length >= 3,
    "the changed sentences are not marked",
  );
});

test("every relative link in the changed documents resolves, anchors included", () => {
  for (const path of [
    FAQ,
    RFC_0001,
    USE_CASES,
    GENESIS,
    "docs/rfc/0002-change-and-log-format.md",
    "docs/rfc/0003-keys-and-cryptography.md",
    "docs/rfc/0006-local-view.md",
    "docs/rfc/0007-relay.md",
    "docs/rfc/0008-membership.md",
  ]) {
    const markdown = read(path).replace(/```[\s\S]*?```/g, "");
    for (const [, target] of markdown.matchAll(/\]\(([^)\s]+)\)/g)) {
      if (/^[a-z]+:/.test(target)) continue;
      const [file, anchor] = target.split("#");
      const resolved = file ? normalize(join(dirname(path), file)) : path;
      assert.ok(
        existsSync(join(root, resolved)),
        `${path}: ${target} has no file`,
      );
      if (anchor && resolved.endsWith(".md")) {
        assert.ok(
          anchors(read(resolved)).has(anchor),
          `${path}: ${target} names no heading`,
        );
      }
    }
  }
});

// pm's additions to NOC-35 (2026-11-09): use case 1's sample uses RFC 0005's
// schema, RFC 0005 quotes RFC 0001's new wording (its Amendment 2), and RFC
// 0004 Amendment 2 parks a create that carries key 5, as the owner decided.
const RFC_0004 = "docs/rfc/0004-merge-rules-basic-types.md";
const RFC_0005 = "docs/rfc/0005-schema-and-model-lock.md";

test("use case 1's sample marks each personal field with .personal({ person }), as RFC 0005 writes it", () => {
  const uc1 = section(read(USE_CASES), "1. Erasure that reaches every copy");
  const sample = uc1.match(/```ts\n([\s\S]*?)```/)?.[1];
  assert.ok(sample, "use case 1 has no TypeScript sample");
  assert.ok(
    !/personal: true/.test(sample),
    "the sample still uses { personal: true }",
  );
  assert.match(
    sample,
    /\.personal\(\{ person: "self" \}\)/,
    "no .personal({ person })",
  );
});

/** The changelog line and section of amendment `n` of `markdown`, with its marked sentences. */
const amendment = (markdown, n, ticket) => {
  const top = markdown.split(/^## /m)[0];
  assert.match(
    top,
    new RegExp(
      `^- \\*\\*\\d{4}-\\d{2}-\\d{2}\\*\\* · ${ticket} · \\[Amendment ${n}: [^\\]]+\\]\\(#amendment-${n}-[^)]+\\)`,
      "m",
    ),
    `no changelog line for Amendment ${n}`,
  );
  const heading = markdown
    .split("\n")
    .find((l) => l.startsWith(`## Amendment ${n}: `));
  assert.ok(heading, `no section Amendment ${n}`);
  const marked = (
    markdown.match(new RegExp(`\\*\\(Amendment ${n}\\.\\)\\*`, "g")) ?? []
  ).length;
  assert.ok(marked >= 1, `no sentence marked *(Amendment ${n}.)*`);
  return section(markdown, heading.slice(3));
};

test("RFC 0005 Amendment 2 quotes RFC 0001's new wording on personal fields and decides nothing", () => {
  const rfc = read(RFC_0005);
  const body = amendment(rfc, 2, "NOC-35");
  assert.match(body, /decides nothing/, "Amendment 2 must decide nothing");
  const personal = section(rfc, "Personal fields");
  assert.ok(
    !/says any field can be marked as personal/.test(personal),
    "RFC 0005 still quotes the old wording",
  );
  assert.match(
    personal.split("\n").find((l) => l.includes("0001-nocentraldb.md")) ?? "",
    /basic value/,
  );
});

test("RFC 0004 Amendment 2 parks a create that carries key 5, decided by the owner", () => {
  const rfc = read(RFC_0004);
  const body = amendment(rfc, 2, "NOC-35");
  assert.match(
    body,
    /\(Carsten, \d{4}-\d{2}-\d{2}\)/,
    "the owner's decision is not recorded",
  );
  assert.ok(
    sentencesLinking(
      body,
      "2026-11-09-NOC-39-a-create-with-arguments-is-malformed.md",
    ).length > 0,
    "Amendment 2 does not link NOC-39's decision",
  );
  const rule = rfc
    .split("\n")
    .find((l) =>
      l.startsWith("- **A create with key 5 present is malformed.**"),
    );
  assert.ok(rule, "no rule for a create with key 5");
  assert.match(rule, /"malformed arguments".*\*\(Amendment 2\.\)\*/);
  assert.match(
    status(rfc),
    /Amendment 2/,
    "the status does not name Amendment 2",
  );
});

// pm, after #74 merged: the owner closed the first private window, so RFC 0006
// says Firefox's private data is gone after the window closes and cites the
// rows of docs/browser-storage.md, not #74's log headings (Amendment 3).
const RFC_0006 = "docs/rfc/0006-local-view.md";

test("RFC 0006 Amendment 3 says a closed Firefox private window loses its data, citing the findings rows", () => {
  const rfc = read(RFC_0006);
  const body = amendment(rfc, 3, "NOC-35");
  assert.match(body, /decides nothing/, "Amendment 3 must decide nothing");
  assert.ok(
    !/whether the first window was closed/.test(rfc),
    "RFC 0006 still doubts the window was closed",
  );
  assert.ok(
    !/#74[:)]|#74, NOC-60\)|by #74/.test(rfc),
    "RFC 0006 still cites #74 by its log headings",
  );
  const privateWindows = rfc
    .split("\n")
    .find((l) =>
      l.startsWith(
        "- **Private windows.** Firefox and WebKit have no OPFS in a private window (`getDirectory()` fails)",
      ),
    );
  assert.ok(privateWindows, "no Private windows finding in Amendment 2");
  assert.match(
    privateWindows,
    /measured: Firefox · Private window/,
    "the finding cites no row",
  );
  assert.match(privateWindows, /NOC-58/, "the open question names no ticket");
  assert.match(privateWindows, /\*\(Amendment 3\.\)\*/);
  for (const row of [
    "Safari (macOS) · SQLite on OPFS (`opfs-sahpool`)",
    "Safari (iOS) · SQLite on OPFS (`opfs-sahpool`)",
    "Firefox (macOS) · `persist()`",
  ]) {
    assert.ok(
      rfc.includes(`measured: ${row}`),
      `RFC 0006 does not cite the row ${row}`,
    );
  }
  const findings = read("docs/browser-storage.md");
  for (const [, browser, behaviour] of rfc.matchAll(
    /(?:measured|documented|unknown): ([^·()\n]+(?:\([^)]*\))?) · ([^;)\n]+?(?:\([^)]*\))?)(?=[;)])/g,
  )) {
    assert.ok(
      findings.includes(`| ${browser.trim()} | ${behaviour.trim()} |`),
      `RFC 0006 cites a row the findings lack: ${browser} · ${behaviour}`,
    );
  }
});
