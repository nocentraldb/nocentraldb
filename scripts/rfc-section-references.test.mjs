// Holds NOC-105: every RFC is one complete text (NOC-100, NOC-101), so code,
// comments, tests, scripts and docs name the RFC section where a rule stands,
// never an RFC amendment by its number. Three places keep their history:
// factory/decisions/ (and the example in factory/DECISIONS.md, which quotes a
// decision file), the change fragments under changes/, which become release
// notes, and the History section of a Markdown file. docs/rfc/README.md
// quotes the old mark once, as the example of what an RFC no longer carries.
// rfc-readme.test.mjs holds the RFCs themselves to the stricter rule that the
// word appears nowhere in them.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");

/** An amendment named by its number, alone or as a list of several. */
const NUMBERED = /\bAmendments?\s+\d/;
/** That name, built from parts so that this file does not trip its own scan. */
const named = (n) => `${"Amend"}ment ${n}`;

const KEEPS_HISTORY = [/^factory\/decisions\//, /^changes\//];
const KEEPS_EXAMPLE = {
  "docs/rfc/README.md": `no mark such as "(${named(3)})"`,
};
const TEXT = /\.(rs|ts|tsx|js|mjs|cjs|py|md|yml|yaml|toml|json|sh)$/;

/** The text of `markdown` without its `## History` section. */
const withoutHistory = (markdown) =>
  markdown.replace(/^## History\n[\s\S]*?(?=^## |(?![\s\S]))/m, "");

const tracked = () =>
  execFileSync("git", ["ls-files", "-z"], { cwd: root, encoding: "utf8" })
    .split("\0")
    .filter((path) => TEXT.test(path))
    .filter((path) => !KEEPS_HISTORY.some((pattern) => pattern.test(path)))
    .filter((path) => existsSync(join(root, path)));

test("no file outside the decision log, the fragments and History sections names an RFC amendment by number", () => {
  const found = [];
  for (const path of tracked()) {
    let text = read(path);
    if (path.endsWith(".md")) text = withoutHistory(text);
    if (path === "factory/DECISIONS.md") {
      // Its example decision keeps the title of the file it quotes.
      text = text.replace(/^# 2026-11-08 · NOC-22 · .*$/m, "");
    }
    if (KEEPS_EXAMPLE[path]) text = text.replace(KEEPS_EXAMPLE[path], "");
    text.split("\n").forEach((line, index) => {
      if (NUMBERED.test(line))
        found.push(`${path}:${index + 1}: ${line.trim()}`);
    });
  }
  assert.deepEqual(
    found,
    [],
    `name the RFC section instead:\n${found.join("\n")}`,
  );
});

test("the History exemption covers only the History section", () => {
  const history = `## History\n\n- ${named(1)}\n`;
  assert.match(
    withoutHistory(`# T\n\n## Rules\n\n${named(2)}\n\n${history}`),
    NUMBERED,
  );
  assert.doesNotMatch(withoutHistory(`# T\n\n${history}\n## Next\n`), NUMBERED);
});

test("every test file the root moon.yml names exists", () => {
  const named = [
    ...read("moon.yml").matchAll(/scripts\/[\w./-]+\.test\.mjs/g),
  ].map(([path]) => path);
  assert.ok(named.length > 0, "moon.yml names no test file");
  const missing = named.filter((path) => !existsSync(join(root, path)));
  assert.deepEqual(
    missing,
    [],
    `moon.yml names removed test files: ${missing.join(", ")}`,
  );
});
