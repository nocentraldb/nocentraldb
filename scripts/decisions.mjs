#!/usr/bin/env node
// The decision log of the software factory (NOC-27): one Markdown file per
// decision in factory/decisions/, named YYYY-MM-DD-<ticket>-<slug>.md, so two
// pull requests never edit the same file. This script reads the folder and
// prints it (`pnpm decisions`), and checks it for the gate
// (`node scripts/decisions.mjs --check`, the task repo:decisions).

import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const DEFAULT_DIR = join(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "factory",
  "decisions",
);

const USAGE = `usage: pnpm decisions [--index] [--current] [--ticket <id>] [--topic <tag>] [--since <YYYY-MM-DD>]
       node scripts/decisions.mjs --check

Prints the decision log in factory/decisions/, every decision in full, in date order.
  --index           one line per decision: date, ticket, title, and what superseded it
  --current         leave out superseded decisions
  --ticket <id>     only decisions about this ticket, e.g. NOC-22
  --topic <tag>     only decisions with this topic, e.g. crypto
  --since <date>    only decisions made on or after this date
  --check           check every file in the folder (the gate) and print nothing else
  --dir <path>      read another folder (for tests)`;

/** YYYY-MM-DD-<ticket>-<slug>.md; the ticket is `no-ticket` or an id like NOC-22. */
const FILE_NAME =
  /^(\d{4}-\d{2}-\d{2})-(no-ticket|[A-Z][A-Z0-9]*-\d+)-([a-z0-9]+(?:-[a-z0-9]+)*)\.md$/;
/** `# YYYY-MM-DD · <ticket> · <title>` */
const HEADING = /^# (\d{4}-\d{2}-\d{2}) · (.+?) · (.+)$/;
const TICKET = /^[A-Z][A-Z0-9]*-\d+$/;
const RANGE = /^([A-Z][A-Z0-9]*)-(\d+) to \1-(\d+)$/;
const TOPIC = /^[a-z0-9][a-z0-9-]*$/;
const FIELD = /^- ([A-Z][a-z]*(?: [a-z]+)*): ?(.*)$/;

const REQUIRED = ["Decided by", "Why", "Instead of", "Topics"];

const isDate = (text) => {
  const date = new Date(`${text}T00:00:00Z`);
  return !Number.isNaN(date.getTime()) && date.toISOString().startsWith(text);
};

const list = (value) => value.split(",").map((item) => item.trim());

/**
 * The ticket ids a heading's ticket part names: `no ticket` names none,
 * `NOC-9, NOC-10` two, `NOC-6 to NOC-13` the range. Null when it is neither.
 */
export function ticketsOf(text) {
  if (text === "no ticket") return [];
  const ids = [];
  for (const part of list(text)) {
    const range = RANGE.exec(part);
    if (range) {
      const [, prefix, from, to] = range;
      if (Number(from) > Number(to)) return null;
      for (let n = Number(from); n <= Number(to); n++)
        ids.push(`${prefix}-${n}`);
    } else if (TICKET.test(part)) {
      ids.push(part);
    } else {
      return null;
    }
  }
  return ids;
}

/** One decision file, parsed, with the problems the gate reports for it. */
function parse(file, text) {
  const problems = [];
  const match = FILE_NAME.exec(file);
  const name = match && isDate(match[1]) ? match : null;
  if (!name) {
    problems.push(
      "the file name does not match YYYY-MM-DD-<ticket>-<slug>.md (<ticket> is no-ticket or an id like NOC-22, <slug> lowercase words joined by -)",
    );
  }
  const lines = text.split("\n");
  const heading = HEADING.exec(lines[0] ?? "");
  const tickets = heading ? ticketsOf(heading[2]) : null;
  if (!heading || tickets === null || !isDate(heading[1])) {
    problems.push(
      "the first line is not a heading `# YYYY-MM-DD · <ticket> · <title>` (<ticket> is `no ticket`, ids like `NOC-9, NOC-10`, or a range `NOC-6 to NOC-13`)",
    );
  } else if (name) {
    const fileTicket = tickets.length === 0 ? "no-ticket" : tickets[0];
    if (heading[1] !== name[1]) {
      problems.push(
        `the heading's date ${heading[1]} differs from the file name's ${name[1]}`,
      );
    }
    if (fileTicket !== name[2]) {
      problems.push(
        `the heading's first ticket ${fileTicket} differs from the file name's ${name[2]}`,
      );
    }
  }
  const fields = new Map();
  for (const line of lines.slice(1)) {
    const field = FIELD.exec(line);
    if (field && !fields.has(field[1])) fields.set(field[1], field[2].trim());
  }
  for (const key of REQUIRED) {
    if (!fields.get(key))
      problems.push(`the field \`- ${key}:\` is missing or empty`);
  }
  const topics = fields.get("Topics") ? list(fields.get("Topics")) : [];
  if (topics.some((topic) => !TOPIC.test(topic))) {
    problems.push(
      "Topics must be lowercase tags (letters, digits, -) separated by commas",
    );
  }
  const supersedes = fields.has("Supersedes")
    ? list(fields.get("Supersedes"))
    : [];
  return {
    entry: {
      file,
      date: heading?.[1] ?? name?.[1] ?? "",
      ticket: heading?.[2] ?? "",
      tickets: tickets ?? [],
      title: heading?.[3] ?? "",
      topics,
      supersedes,
      text,
    },
    problems,
  };
}

/**
 * Every decision in `dir`, sorted by file name (date order, same day by
 * name), and every problem the gate reports, as `<file>: <problem>`.
 */
export function readDecisions(dir) {
  const files = readdirSync(dir).sort();
  const entries = [];
  const problems = [];
  for (const file of files) {
    const parsed = parse(file, readFileSync(join(dir, file), "utf8"));
    entries.push(parsed.entry);
    problems.push(...parsed.problems.map((problem) => `${file}: ${problem}`));
  }
  const names = new Set(files);
  for (const { file, supersedes } of entries) {
    for (const old of supersedes) {
      if (!names.has(old)) {
        problems.push(
          `${file}: Supersedes names ${old}, which is not in the folder`,
        );
      }
    }
  }
  problems.sort((a, b) => a.split(":")[0].localeCompare(b.split(":")[0]));
  return { entries, problems };
}

/** The gate: every problem of the folder, empty when it is well formed. */
export function checkDecisions(dir) {
  return readDecisions(dir).problems;
}

export function render(_entries, _options = {}) {
  throw new Error("not implemented");
}

function main(argv) {
  let values;
  try {
    ({ values } = parseArgs({
      args: argv,
      options: {
        index: { type: "boolean" },
        current: { type: "boolean" },
        ticket: { type: "string" },
        topic: { type: "string" },
        since: { type: "string" },
        check: { type: "boolean" },
        dir: { type: "string" },
        help: { type: "boolean", short: "h" },
      },
    }));
  } catch (error) {
    process.stderr.write(`decisions: ${error.message}\n${USAGE}\n`);
    return 2;
  }
  if (values.help) {
    process.stdout.write(`${USAGE}\n`);
    return 0;
  }
  const { entries, problems } = readDecisions(values.dir ?? DEFAULT_DIR);
  if (problems.length > 0) {
    for (const problem of problems)
      process.stderr.write(`decisions: ${problem}\n`);
    return 1;
  }
  if (values.check) return 0;
  const output = render(entries, values);
  if (output) process.stdout.write(`${output}\n`);
  return 0;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  process.exitCode = main(process.argv.slice(2));
}
