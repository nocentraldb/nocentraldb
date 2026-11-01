#!/usr/bin/env node
// The release index release-notes/index.json (docs/release-management.md,
// "Release notes"): one entry per published beta and release. The publish
// job runs `add` after publishing; validateIndex holds the committed file to
// its shape in repo:test and is used by the commit back to main. Rules:
//
// I1 The document is an object with exactly the key "releases", a list.
// I2 Every entry has exactly the keys version, stage, tag, date, path.
// I3 version is a plain X.Y.Z; stage is beta or release; tag is v<version>
//    for a release and v<version>-beta.N for a beta.
// I4 date is YYYY-MM-DD, a real calendar day, the UTC day of publishing.
// I5 path is release-notes/<version>.md.
// I6 tags are unique, and the entries are sorted ascending by version order
//    of their tags (0.1.0-beta.1 < 0.1.0-beta.2 < 0.1.0 < 0.1.1-beta.1).
// I7 add is idempotent: an entry identical to one already indexed changes
//    nothing ("already indexed", exit 0, the file untouched); an entry for
//    an indexed tag that differs in any field is exit 1 and writes nothing.
// I8 add writes JSON.stringify(index, null, 2) plus a newline, and only an
//    index that passes I1-I6 before and after.
//
// Exit 0 on success, 1 with `index: <message>` naming the file or the tag,
// 2 on a usage error.
import { readFileSync, realpathSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { cli } from "./lib/cli.mjs";
import { classifyTag, compare, PLAIN } from "./lib/semver.mjs";

const USAGE = `usage: index add --version <X.Y.Z> --stage <beta|release> --tag <tag> --date <YYYY-MM-DD> [--file <path>]
Appends a published beta or release to the release index <path> (default
release-notes/index.json), keeping it sorted. <date> is the UTC calendar day
of publishing. Adding an entry that is already indexed, identically, changes
nothing.`;

const KEYS = ["version", "stage", "tag", "date", "path"];
const DATE = /^\d{4}-\d{2}-\d{2}$/;

const isObject = (v) =>
  v !== null && typeof v === "object" && !Array.isArray(v);

/** A YYYY-MM-DD that names a real calendar day. */
const isDay = (date) =>
  typeof date === "string" &&
  DATE.test(date) &&
  !Number.isNaN(Date.parse(`${date}T00:00:00Z`)) &&
  new Date(`${date}T00:00:00Z`).toISOString().slice(0, 10) === date;

/** The problems of one entry, each naming it; empty when it is well-formed. */
const entryErrors = (entry, i) => {
  const name = `releases[${i}]${typeof entry?.tag === "string" ? ` (${entry.tag})` : ""}`;
  if (!isObject(entry)) return [`${name} is not an object`];
  const errors = [];
  for (const key of Object.keys(entry))
    if (!KEYS.includes(key)) errors.push(`${name} has the unknown key ${key}`);
  for (const key of KEYS)
    if (!(key in entry)) errors.push(`${name} has no ${key}`);
  const { version, stage, tag, date, path } = entry;
  if (typeof version !== "string" || !PLAIN.test(version))
    errors.push(`${name}: version ${JSON.stringify(version)} is not X.Y.Z`);
  if (stage !== "beta" && stage !== "release")
    errors.push(
      `${name}: stage ${JSON.stringify(stage)} is not beta or release`,
    );
  const t = typeof tag === "string" ? classifyTag(tag) : null;
  if (t === null || t.stage !== stage || t.version.split("-")[0] !== version)
    errors.push(
      `${name}: tag ${JSON.stringify(tag)} is not ${stage === "release" ? `v${version}` : `v${version}-beta.N`}`,
    );
  if (!isDay(date))
    errors.push(
      `${name}: date ${JSON.stringify(date)} is not a calendar day YYYY-MM-DD (UTC)`,
    );
  if (path !== `release-notes/${version}.md`)
    errors.push(
      `${name}: path ${JSON.stringify(path)} is not release-notes/${version}.md`,
    );
  return errors;
};

/** The problems of a parsed index document; empty when it has the shape I1-I6. */
export function validateIndex(index) {
  if (
    !isObject(index) ||
    Object.keys(index).join() !== "releases" ||
    !Array.isArray(index.releases)
  )
    return [
      'the index must be an object with exactly the key "releases", a list',
    ];
  const errors = index.releases.flatMap(entryErrors);
  if (errors.length > 0) return errors;
  const seen = new Set();
  let previous = null;
  for (const { tag } of index.releases) {
    if (seen.has(tag)) errors.push(`tag ${tag} is indexed more than once`);
    seen.add(tag);
    const t = classifyTag(tag);
    if (previous !== null && compare(previous.t, t) > 0)
      errors.push(
        `the entries are not sorted ascending: ${tag} comes after ${previous.tag}`,
      );
    previous = { tag, t };
  }
  return errors;
}

const sameEntry = (a, b) => KEYS.every((key) => a[key] === b[key]);

const { fail, usageError, parseArgs } = cli("index", USAGE);

const ARGS = {
  values: ["--version", "--stage", "--tag", "--date", "--file"],
  defaults: { file: "release-notes/index.json" },
};

/** The parsed index at `file`; null after reporting why it cannot be used. */
const load = (file) => {
  let index;
  try {
    index = JSON.parse(readFileSync(file, "utf8"));
  } catch (error) {
    fail(`${file}: ${error.code === "ENOENT" ? "missing" : error.message}`);
    return null;
  }
  const errors = validateIndex(index);
  for (const error of errors) fail(`${file}: ${error}`);
  return errors.length > 0 ? null : index;
};

const add = ({ file, version, stage, tag, date }) => {
  const entry = {
    version,
    stage,
    tag,
    date,
    path: `release-notes/${version}.md`,
  };
  const errors = entryErrors(entry, 0).map((e) =>
    e.replace(/^releases\[0\]/, "the new entry"),
  );
  for (const error of errors) fail(error);
  if (errors.length > 0) return;
  const index = load(file);
  if (index === null) return;
  const indexed = index.releases.find((e) => e.tag === tag);
  if (indexed !== undefined) {
    if (!sameEntry(indexed, entry))
      return fail(`index entry for ${tag} differs`);
    return console.log(`index: ${tag} is already indexed`);
  }
  const releases = [...index.releases, entry].sort((a, b) =>
    compare(classifyTag(a.tag), classifyTag(b.tag)),
  );
  writeFileSync(file, `${JSON.stringify({ releases }, null, 2)}\n`);
  console.log(`index: added ${tag} to ${file}`);
};

const main = () => {
  const [command, ...args] = process.argv.slice(2);
  if (command === "--help") parseArgs([command]);
  if (command !== "add") usageError();
  const opts = parseArgs(args, ARGS);
  for (const key of ["version", "stage", "tag", "date"])
    if (opts[key] === undefined) usageError();
  add(opts);
};

const invoked =
  process.argv[1] !== undefined &&
  realpathSync(process.argv[1]) === fileURLToPath(import.meta.url);
if (invoked) main();
