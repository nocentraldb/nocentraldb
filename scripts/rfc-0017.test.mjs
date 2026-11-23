// RFC 0017 drafts the ncdb MCP server (NOC-485): a Rust binary on the Rust
// SDK that is itself an agent's member, for clients that cannot link an SDK,
// such as a person's own Claude Code or Codex. It is published as a draft
// and states its design directly, as docs/rfc/README.md asks of a newly
// published RFC: each section says what the design does and why, then the
// options not taken. It names nobody who decides; the questions for the
// decision stand in its pull request. These tests check that each part is
// there; whether the design is right is the review of the draft.
import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { historyLines } from "./lib/rfc-record.mjs";
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

test("RFC 0017 is a draft about the MCP server", () => {
  const markdown = text();
  assert.match(markdown.split("\n")[0], /^# RFC 0017: .*MCP server/);
  assert.equal(
    markdown.split("\n")[2],
    "REQUEST FOR COMMENTS · NOVEMBER 2026 · Ticket NOC-485",
  );
  assert.match(markdown, /^\*\*Status\.\*\* Draft\./m);
  assert.match(markdown, /^\*\*At 0\.1\.\*\* Not built\./m);
});

test("RFC 0017 states its design directly and names nobody who decides", () => {
  const found = headings(text());
  for (const heading of ["Questions to the owner", "Decisions"]) {
    assert.ok(!found.includes(heading), `it has a section "${heading}"`);
  }
  const body = prose();
  assert.doesNotMatch(body, /\bRecommended\b/);
  assert.doesNotMatch(body, /\bCarsten\b|\bthe owner\b|\bpm\b/);
  const label = body.match(/\boption [A-Z]\b/);
  assert.ok(!label, `an option label the reader must decode: ${label?.[0]}`);
});

test("RFC 0017's History is its one publication line", () => {
  assert.deepEqual(historyLines(text()), [
    "- 2026-11-23 · First published (NOC-485)",
  ]);
});

// Each section that makes a choice states it, then the options not taken.
const CHOICES = [
  ["what-the-server-is", "what the server is"],
  ["becoming-a-member", "how it becomes a member"],
  ["transports", "the transports"],
  ["which-process-holds-the-copy", "the lifecycle"],
  ["how-current-the-copy-is", "how current the copy stays"],
  ["the-tools", "the tool surface"],
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

test("RFC 0017 names every tool, the clients' limits and the token", () => {
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

test("the RFC lists name RFC 0017 as a draft", () => {
  const index = read("docs/rfc/README.md");
  const row = index.split("\n").find((l) => l.startsWith("| [0017]"));
  assert.ok(row, "the RFC index has no row for RFC 0017");
  assert.match(row, /draft/);
  const site = read("site/src/content/docs/docs/rfcs.md");
  const item = site.split("\n").find((l) => l.includes("[RFC 0017: "));
  assert.ok(item, "the site's RFC list has no item for RFC 0017");
  assert.match(item, /\/docs\/rfc\/0017-/);
  assert.match(item, /draft/);
});
