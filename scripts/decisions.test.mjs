// Tests of the decision log tooling (NOC-27): scripts/decisions.mjs reads the
// one-file-per-decision folder factory/decisions/, prints it (`pnpm
// decisions`) and checks it in the gate (`repo:decisions`).
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { parse as parseYaml } from "yaml";

import { checkDecisions } from "./decisions.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");
const SCRIPT = join(root, "scripts/decisions.mjs");

/** Runs scripts/decisions.mjs with `args`; returns status, stdout, stderr. */
const run = (...args) =>
  spawnSync(process.execPath, [SCRIPT, ...args], { encoding: "utf8" });

/** A decision file's text, with the fields a test may override or drop. */
const entry = ({
  date = "2026-11-08",
  ticket = "NOC-22",
  title = "A decision",
  decidedBy = "Carsten",
  why = "one line",
  insteadOf = "the option not taken",
  topics = "factory",
  supersedes,
} = {}) =>
  [
    `# ${date} · ${ticket} · ${title}`,
    "",
    decidedBy === null ? null : `- Decided by: ${decidedBy}`,
    why === null ? null : `- Why: ${why}`,
    insteadOf === null ? null : `- Instead of: ${insteadOf}`,
    topics === null ? null : `- Topics: ${topics}`,
    supersedes === undefined ? null : `- Supersedes: ${supersedes}`,
    "",
  ]
    .filter((line) => line !== null)
    .join("\n");

/** A temporary decisions folder holding `files` ({ name: text }). */
const folder = (t, files) => {
  const dir = mkdtempSync(join(tmpdir(), "decisions-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  for (const [name, text] of Object.entries(files)) {
    writeFileSync(join(dir, name), text);
  }
  return dir;
};

const GOOD = "2026-11-08-NOC-22-a-decision.md";

test("check accepts well-formed files, with and without a ticket", (t) => {
  const dir = folder(t, {
    [GOOD]: entry(),
    "2026-11-07-no-ticket-tickets-live-in-linear.md": entry({
      date: "2026-11-07",
      ticket: "no ticket",
      supersedes: GOOD,
    }),
    "2026-11-07-NOC-6-a-range.md": entry({
      date: "2026-11-07",
      ticket: "NOC-6 to NOC-13",
      topics: "rfc, process",
    }),
    "2026-11-07-NOC-9-a-list.md": entry({
      date: "2026-11-07",
      ticket: "NOC-9, NOC-10",
    }),
  });
  assert.deepEqual(checkDecisions(dir), []);
});

test("check rejects a badly named file", (t) => {
  for (const name of [
    "2026-11-08-a-decision.md", // no ticket
    "2026-11-08-noc-22-a-decision.md", // ticket in lower case
    "2026-11-08-NOC-22-A-Decision.md", // slug not lower case
    "2026-11-08-NOC-22-.md", // no slug
    "2026-13-40-NOC-22-a-decision.md", // no such date
    "26-11-08-NOC-22-a-decision.md", // short year
    "2026-11-08-NOC-22-a-decision.txt", // not Markdown
    "README.md",
  ]) {
    const dir = folder(t, { [name]: entry() });
    const problems = checkDecisions(dir);
    assert.equal(problems.length, 1, `${name}: ${problems.join("; ")}`);
    assert.match(problems[0], new RegExp(`^${name.replace(/\./g, "\\.")}: `));
    assert.match(problems[0], /name/);
  }
});

test("check rejects a file that lacks a required field", (t) => {
  for (const [field, override] of [
    ["Decided by", { decidedBy: null }],
    ["Why", { why: null }],
    ["Instead of", { insteadOf: null }],
    ["Topics", { topics: null }],
  ]) {
    const dir = folder(t, { [GOOD]: entry(override) });
    const problems = checkDecisions(dir);
    assert.equal(problems.length, 1, `${field}: ${problems.join("; ")}`);
    assert.match(problems[0], new RegExp(`^${GOOD}: .*${field}`));
  }
});

test("check rejects an empty field and topics that are not lowercase tags", (t) => {
  for (const override of [
    { why: "" },
    { topics: "" },
    { topics: "Crypto" },
    { topics: "rfc 0002" },
    { topics: "rfc,,log" },
  ]) {
    const dir = folder(t, { [GOOD]: entry(override) });
    assert.equal(checkDecisions(dir).length, 1, JSON.stringify(override));
  }
});

test("check rejects a file without a heading of date, ticket and title", (t) => {
  for (const text of [
    entry().replace(/^# /, "### "),
    entry().replace(/^# .*\n/, ""),
    entry({ title: "" }),
    entry({ ticket: "NOC 22" }),
  ]) {
    const dir = folder(t, { [GOOD]: text });
    const problems = checkDecisions(dir);
    assert.equal(problems.length, 1, `${text}\n${problems.join("; ")}`);
    assert.match(problems[0], /heading/);
  }
});

test("check rejects a heading whose date or ticket differs from its file name", (t) => {
  for (const [name, override] of [
    [GOOD, { date: "2026-11-07" }],
    [GOOD, { ticket: "NOC-23" }],
    [GOOD, { ticket: "no ticket" }],
    [GOOD, { ticket: "NOC-21, NOC-22" }],
    ["2026-11-08-no-ticket-a-decision.md", { ticket: "NOC-22" }],
  ]) {
    const dir = folder(t, { [name]: entry(override) });
    const problems = checkDecisions(dir);
    assert.equal(problems.length, 1, `${name}: ${problems.join("; ")}`);
    assert.match(problems[0], /heading/);
    assert.match(problems[0], /file name/);
  }
});

test("check rejects a Supersedes: that names no file in the folder", (t) => {
  const dir = folder(t, {
    [GOOD]: entry(),
    "2026-11-09-NOC-30-later.md": entry({
      date: "2026-11-09",
      ticket: "NOC-30",
      supersedes: `${GOOD}, 2026-11-01-NOC-1-gone.md`,
    }),
  });
  const problems = checkDecisions(dir);
  assert.equal(problems.length, 1, problems.join("; "));
  assert.match(
    problems[0],
    /^2026-11-09-NOC-30-later\.md: .*Supersedes.*2026-11-01-NOC-1-gone\.md/,
  );
});

test("check reports every problem of every file, sorted by file name", (t) => {
  const dir = folder(t, {
    "b.md": entry(),
    [GOOD]: entry({ why: null, topics: null }),
  });
  const problems = checkDecisions(dir);
  assert.equal(problems.length, 3, problems.join("; "));
  assert.deepEqual(
    problems.map((problem) => problem.split(":")[0]),
    [GOOD, GOOD, "b.md"],
  );
});

test("--check exits 1 and names each problem on stderr", (t) => {
  const dir = folder(t, { [GOOD]: entry({ topics: null }) });
  const result = run("--check", "--dir", dir);
  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stderr, new RegExp(`${GOOD}: .*Topics`));
});

test("--check exits 0 on a well-formed folder", (t) => {
  const dir = folder(t, { [GOOD]: entry() });
  const result = run("--check", "--dir", dir);
  assert.equal(result.status, 0, result.stderr);
});

test("an unknown option exits 2 with the usage", () => {
  const result = run("--frobnicate");
  assert.equal(result.status, 2);
  assert.match(result.stderr, /usage/i);
});

test("pnpm gates runs the check over factory/decisions/", () => {
  const moon = parseYaml(read("moon.yml"));
  const task = moon.tasks.decisions;
  assert.ok(task, "moon.yml has no task `decisions`");
  assert.equal(task.command, "node scripts/decisions.mjs --check");
  assert.ok(task.inputs.includes("/factory/decisions/**/*"), "inputs");
  assert.ok(task.inputs.includes("/scripts/decisions.mjs"), "inputs");
  assert.ok(moon.tasks.gates.deps.includes("~:decisions"), "gates deps");
});

test("pnpm decisions runs the reading command", () => {
  const pkg = JSON.parse(read("package.json"));
  assert.equal(pkg.scripts.decisions, "node scripts/decisions.mjs");
});

/** A folder of four decisions over three days; the last supersedes the first. */
const LOG = {
  "2026-11-09-NOC-30-later.md": entry({
    date: "2026-11-09",
    ticket: "NOC-30",
    title: "Later",
    topics: "crypto, rfc-0003",
    supersedes: "2026-11-07-no-ticket-first.md",
  }),
  "2026-11-08-NOC-22-b-same-day.md": entry({
    title: "B same day",
    topics: "log",
  }),
  "2026-11-08-NOC-22-a-same-day.md": entry({
    title: "A same day",
    topics: "log, crypto",
  }),
  "2026-11-07-no-ticket-first.md": entry({
    date: "2026-11-07",
    ticket: "no ticket",
    title: "First",
    topics: "factory",
  }),
};
const ORDER = [
  "2026-11-07-no-ticket-first.md",
  "2026-11-08-NOC-22-a-same-day.md",
  "2026-11-08-NOC-22-b-same-day.md",
  "2026-11-09-NOC-30-later.md",
];

/** The titles of the decisions printed in full, in output order. */
const titles = (stdout) =>
  [...stdout.matchAll(/^# \S+ · .+? · (.+)$/gm)].map(([, title]) => title);

test("without arguments it prints every decision in full, in date order", (t) => {
  const dir = folder(t, LOG);
  const result = run("--dir", dir);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(titles(result.stdout), [
    "First",
    "A same day",
    "B same day",
    "Later",
  ]);
  for (const name of ORDER) {
    assert.ok(result.stdout.includes(LOG[name].trim()), `${name} in full`);
  }
});

test("the full log says which decision superseded an entry", (t) => {
  const dir = folder(t, LOG);
  const { stdout } = run("--dir", dir);
  const first = stdout.slice(0, stdout.indexOf("# 2026-11-08"));
  assert.match(first, /Superseded by 2026-11-09-NOC-30-later\.md/);
  assert.equal(stdout.match(/Superseded by/g).length, 1);
});
