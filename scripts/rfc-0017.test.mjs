// RFC 0017 designs the ncdb MCP server (NOC-485): a Rust binary on the Rust
// SDK that is itself an agent's member, for clients that cannot link an SDK,
// such as a person's own Claude Code or Codex, and that serves the tools an
// app declares in its model (NOC-488). It is decided and states its design
// directly, as docs/rfc/README.md asks of a newly published RFC: each
// section says what the design does and why, then the options not taken.
// It names nobody who decided; the decision log does. The public names that
// declared tools add were decided a day later (NOC-504): they stand in the
// sections they belong to, with the other options under "Options not
// taken", a short entry each under "Decisions" and a second History line.
// The tests that asked for an "Open questions" section and for one History
// line were rewritten for the decided text then. These tests check that
// each part is there; whether the design is right was the owner's review.
import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import {
  assertRecord,
  decisionEntries,
  historyLines,
} from "./lib/rfc-record.mjs";
import { headings, read, root, section } from "./lib/rfc-text.mjs";

const file = () => {
  const found = readdirSync(join(root, "docs/rfc")).filter(
    (f) => f.startsWith("0017-") && f.endsWith(".md"),
  );
  assert.equal(found.length, 1, "docs/rfc holds one RFC 0017");
  return `docs/rfc/${found[0]}`;
};

const text = () => read(file());

/** The prose of the RFC: no fenced code. */
const prose = () => text().replace(/^```[\s\S]*?^```$/gm, "");

test("RFC 0017 is decided and is about the MCP server", () => {
  const markdown = text();
  assert.match(markdown.split("\n")[0], /^# RFC 0017: .*MCP server/);
  assert.equal(
    markdown.split("\n")[2],
    "REQUEST FOR COMMENTS · NOVEMBER 2026 · Ticket NOC-485",
  );
  assert.match(markdown, /^\*\*Status\.\*\* Decided, 2026-11-23\./m);
  assert.match(markdown, /^\*\*At 0\.1\.\*\* Not built\./m);
});

test("RFC 0017 states its design directly and names nobody who decides", () => {
  const found = headings(text());
  for (const heading of ["Questions to the owner", "Open questions"]) {
    assert.ok(!found.includes(heading), `it has a section "${heading}"`);
  }
  const body = prose();
  assert.doesNotMatch(body, /\bRecommended\b/);
  assert.doesNotMatch(body, /\bCarsten\b|\bthe owner\b|\bpm\b/);
  const label = body.match(/\boption [A-Z]\b/);
  assert.ok(!label, `an option label the reader must decode: ${label?.[0]}`);
});

test("RFC 0017's History is its publication and the decision on declared tools' names", () => {
  const lines = historyLines(text());
  assert.equal(lines[0], "- 2026-11-23 · First published (NOC-485)");
  assert.equal(lines.length, 2);
  assert.match(lines[1], /^- 2026-11-24 · .*\(NOC-504, #\d+\)$/);
  assertRecord(text(), "2026-11-23 · First published (NOC-485)");
});

test("RFC 0017's Decisions name the four answers on declared tools, each with its decision file", () => {
  const entries = decisionEntries(text());
  assert.equal(entries.length, 4, entries.join("\n"));
  for (const entry of entries) {
    assert.match(entry, /^- \*\*2026-11-24 · /);
    assert.match(entry, /factory\/decisions\/2026-11-24-NOC-504-/);
  }
});

test("RFC 0017 states the decided names in their sections, with the others not taken", () => {
  const status = text()
    .split("\n")
    .find((l) => l.startsWith("**Status.**"));
  assert.doesNotMatch(status, /open|proposed/i);
  const declared = section(text(), "declared-tools");
  assert.match(declared, /`format` 2/);
  assert.match(declared, /`toolsRefused`/);
  assert.doesNotMatch(declared, /proposed/);
  for (const notTaken of [
    "model.tools.json",
    "format 1",
    "nc.agent",
    "snake case",
  ]) {
    assert.ok(
      declared.includes(notTaken),
      `declared tools do not name ${notTaken} as not taken`,
    );
  }
  const tools = section(text(), "the-tools");
  assert.match(tools, /`--dev`/);
  assert.match(tools, /`list_spaces`[^.]*behind/);
});

// Each section that makes a choice states it, then the options not taken.
const CHOICES = [
  ["what-the-server-is", "what the server is"],
  ["becoming-a-member", "how it becomes a member"],
  ["transports", "the transports"],
  ["which-process-holds-the-copy", "the lifecycle"],
  ["how-current-the-copy-is", "how current the copy stays"],
  ["the-tools", "the tool surface"],
  ["declared-tools", "the tools an app declares"],
  ["identity-rules", "the identity rules"],
  ["how-it-ships", "distribution"],
];

for (const [anchor, name] of CHOICES) {
  test(`RFC 0017 states its design on ${name}, then the options not taken`, () => {
    const part = section(text(), anchor);
    const at = part.indexOf("### Options not taken");
    assert.ok(at > 0, 'no "### Options not taken" after the design');
    const options = part
      .slice(at)
      .split("\n")
      .filter((l) => l.startsWith("- **"));
    assert.ok(options.length >= 1, "no option not taken is listed");
  });
}

test("RFC 0017 names the declared tools' schema, lock and build check", () => {
  const part = section(text(), "declared-tools");
  for (const name of [
    "nc.tools",
    "nc.tool.find",
    "nc.tool.get",
    "nc.tool.action",
  ]) {
    assert.ok(part.includes(`\`${name}`), `no ${name}`);
  }
  assert.match(part, /"tools"/, "the lock's key");
  assert.match(part, /`ncdb model build`/);
  assert.match(part, /one change/, "an action is one write");
});

test("RFC 0017 keeps the generic tools behind a developer flag", () => {
  assert.match(prose(), /`--generic-tools`/);
});

test("RFC 0017 names every generic tool, the clients' limits and the token", () => {
  const body = prose();
  for (const tool of [
    "list_spaces",
    "describe_model",
    "find",
    "get",
    "inspect",
    "write",
    "read_rich_text",
    "edit_rich_text",
  ]) {
    assert.ok(body.includes(`\`${tool}\``), `no tool ${tool}`);
  }
  assert.match(body, /10 s to start and 60 s per tool call/);
  assert.match(body, /`syncedAt`/);
  assert.match(body, /`Origin`/);
  assert.match(body, /Bearer/);
  assert.match(body, /loopback/);
});

test("RFC 0017 says plainly what it does not do", () => {
  const part = section(text(), "what-this-does-not-do");
  assert.match(part, /model's provider/);
  assert.match(part, /prompt injection/);
  assert.match(part, /replace the SDKs/);
});

test("the decision log holds one file per decision of RFC 0017", () => {
  const files = readdirSync(join(root, "factory/decisions")).filter((f) =>
    /^2026-11-2(3-NOC-485|4-NOC-504)-/.test(f),
  );
  assert.equal(files.length, 11, files.join("\n"));
  for (const f of files) {
    const entry = read(`factory/decisions/${f}`);
    assert.match(
      entry,
      /^- Decided by: Carsten, 2026-11-2[34]\b.*NOC-(488|504)/m,
    );
    assert.match(entry, /^- Topics: .*\brfc-0017\b/m);
  }
});

test("the RFC lists name RFC 0017 without calling it a draft", () => {
  const index = read("docs/rfc/README.md");
  const row = index.split("\n").find((l) => l.startsWith("| [0017]"));
  assert.ok(row, "the RFC index has no row for RFC 0017");
  assert.doesNotMatch(row, /draft/);
  const site = read("site/src/content/docs/docs/rfcs.md");
  const item = site.split("\n").find((l) => l.includes("[RFC 0017: "));
  assert.ok(item, "the site's RFC list has no item for RFC 0017");
  assert.match(item, /\/docs\/rfc\/0017-/);
  assert.doesNotMatch(item, /draft/);
});
