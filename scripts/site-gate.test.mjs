// Asserts the gate task `repo:site` (NOC-213): `pnpm gates` installs the
// site's own pnpm project from its lockfile, builds it, and checks the built
// output with site/scripts/output.test.mjs, so a change that breaks the site,
// its llms.txt files or a docs page's Markdown copy fails the gate.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { parse } from "yaml";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const moon = parse(readFileSync(join(root, "moon.yml"), "utf8"));

test("repo:site installs the site from its lockfile, builds it and checks the output", () => {
  const site = moon.tasks?.site;
  assert.ok(site, "moon.yml has no task site");
  assert.equal(
    site.script,
    "pnpm --dir site install --frozen-lockfile && pnpm --dir site build && node --test 'site/scripts/*.test.mjs'",
  );
  assert.ok(site.inputs?.includes("/site/**/*"), "the site is an input");
});

test("pnpm gates runs repo:site", () => {
  assert.ok(moon.tasks.gates.deps.includes("~:site"), "gates deps");
});
