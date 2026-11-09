// Asserts the shape of the README (NOC-61): it opens with a disclaimer that
// says NoCentralDB is not yet a product and links the RFCs, then has the
// sections "What it is for", "Install", "Code examples" and "Contributing", in
// that order; and every code identifier in its examples appears in the code of
// an RFC, so the README shows no API name the RFCs do not define. The owner
// chose to keep RFC links and limits out of the README's feature list (see
// factory/decisions/), so no test asks for them there.
import assert from "node:assert/strict";
import { globSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");

const SECTIONS = ["What it is for", "Install", "Code examples", "Contributing"];

/** The body of the level-2 section with this heading, or undefined. */
function section(markdown, heading) {
  const start = markdown.indexOf(`\n## ${heading}\n`);
  if (start < 0) return undefined;
  const body = markdown.slice(start + heading.length + 5);
  const end = body.search(/\n## /);
  return end < 0 ? body : body.slice(0, end);
}

/** The text of every fenced code block and inline code span. */
function code(markdown) {
  const blocks = [...markdown.matchAll(/```[^\n]*\n([\s\S]*?)```/g)].map(
    (m) => m[1],
  );
  const prose = markdown.replace(/```[^\n]*\n[\s\S]*?```/g, "");
  const spans = [...prose.matchAll(/`([^`\n]+)`/g)].map((m) => m[1]);
  return [...blocks, ...spans].join("\n");
}

/** Strips comments and the contents of string literals from code. */
function stripped(source) {
  return source
    .replace(/\/\/[^\n]*/g, "")
    .replace(/#[^\n]*/g, "")
    .replace(/"[^"\n]*"/g, '""')
    .replace(/'[^'\n]*'/g, "''");
}

// Words of the languages themselves, not names the RFCs must define.
const KEYWORDS = new Set([
  "import",
  "from",
  "export",
  "const",
  "let",
  "await",
  "async",
  "type",
  "typeof",
  "true",
  "false",
]);

const IDENTIFIER = /[A-Za-z_$][\w$]*/g;
const CHAIN = /(?<![\w$.])[A-Za-z_$][\w$]*(?:\.[A-Za-z_$][\w$]*)+/g;

const rfcCode = globSync("docs/rfc/*.md", { cwd: root })
  .filter((path) => !path.endsWith("README.md"))
  .map((path) => stripped(code(read(path))))
  .join("\n");
const rfcWords = new Set(rfcCode.match(IDENTIFIER));

test("the README opens with a disclaimer that links the RFCs", () => {
  const readme = read("README.md");
  const top = readme.slice(0, readme.indexOf("\n## "));
  const disclaimer = top
    .split("\n")
    .filter((line) => line.startsWith(">"))
    .join("\n");
  assert.ok(
    disclaimer,
    "the README has no disclaimer block before its first section",
  );
  assert.match(disclaimer, /not (yet )?(released|a product)/i);
  assert.match(disclaimer, /\]\(docs\/rfc\/README\.md\)/);
});

test("the README has its sections in order", () => {
  const headings = [...read("README.md").matchAll(/^## (.+)$/gm)].map(
    (m) => m[1],
  );
  const found = SECTIONS.map((heading) => headings.indexOf(heading));
  for (const [i, heading] of SECTIONS.entries()) {
    assert.ok(found[i] >= 0, `the README has no section "${heading}"`);
  }
  assert.deepEqual(
    [...found].sort((a, b) => a - b),
    found,
    `the sections are not in the order ${SECTIONS.join(", ")}`,
  );
});

test("the examples are marked as planned API", () => {
  const body = section(read("README.md"), "Code examples");
  assert.ok(body, "the README has no section Code examples");
  const intro = body.slice(0, body.indexOf("```"));
  assert.match(intro, /planned/i);
});

test("every code identifier in the README's examples appears in an RFC", () => {
  const body = section(read("README.md"), "Code examples");
  assert.ok(body, "the README has no section Code examples");
  const examples = stripped(
    [...body.matchAll(/```[^\n]*\n([\s\S]*?)```/g)].map((m) => m[1]).join("\n"),
  );
  assert.ok(examples.trim(), "Code examples holds no code");
  const missing = new Set();
  for (const word of examples.match(IDENTIFIER) ?? []) {
    if (!KEYWORDS.has(word) && !rfcWords.has(word)) missing.add(word);
  }
  for (const chain of examples.match(CHAIN) ?? []) {
    if (!rfcCode.includes(chain)) missing.add(chain);
  }
  assert.deepEqual([...missing], [], "names no RFC defines");
});
