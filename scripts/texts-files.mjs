// The texts' files (NOC-512). Every text a developer reads lives in a file
// of its own, texts/<kind>/<name>.json, holding its one entry as
// `JSON.stringify(entry, null, 2)` writes it. Two pull requests that each
// add a text add two files, so they never conflict over the texts, as they
// did when all of them stood in one sorted texts/texts.json.
//
// The generated modules list their texts in `mergeOrder`, by a hash of
// their ids, not by id: texts of one feature share a prefix, so in id order
// two pull requests' new texts often land side by side and conflict there.
// In hash order two new texts land side by side only by chance, about once
// in as many texts as the module holds.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

/** The folder of the texts, relative to the root. */
export const TEXTS_DIR = "texts";

/** The file of `entry`, relative to the root: texts/<kind>/<name>.json. */
export const entryPath = (entry) =>
  `${TEXTS_DIR}/${entry.kind}/${entry.id.slice(entry.kind.length + 1)}.json`;

/** `entry` as its file holds it. */
export const entryJson = (entry) => `${JSON.stringify(entry, null, 2)}\n`;

const byId = (a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0);

/**
 * The entries of `files`, a list of [path, raw content] under texts/, and
 * the problems of their layout: a JSON file in a kind's folder, at its id's
 * path, one field per line. README.md at the top is the folder's guide.
 */
export const parseTexts = (files) => {
  const texts = [];
  const problems = [];
  for (const [path, raw] of files) {
    const rest = path.slice(TEXTS_DIR.length + 1);
    if (rest === "README.md") continue;
    if (!/^[^/]+\/[^/]+\.json$/.test(rest)) {
      problems.push(
        `${path}: the texts folder holds only texts/<kind>/<name>.json and README.md`,
      );
      continue;
    }
    let entry;
    try {
      entry = JSON.parse(raw);
    } catch (error) {
      problems.push(`${path}: not JSON: ${error.message}`);
      continue;
    }
    if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
      problems.push(`${path}: an entry is an object`);
      continue;
    }
    if (typeof entry.id === "string" && typeof entry.kind === "string") {
      const at = entryPath(entry);
      if (at !== path) problems.push(`${path}: ${entry.id} belongs in ${at}`);
    }
    if (raw !== entryJson(entry)) {
      problems.push(
        `${path}: write it as JSON.stringify(entry, null, 2) does, one field per line, with a final newline`,
      );
    }
    texts.push(entry);
  }
  texts.sort(byId);
  return { texts, problems };
};

/** Every file under `dir`, relative to `root`. */
const walk = (root, dir) =>
  readdirSync(join(root, dir)).flatMap((name) => {
    const path = `${dir}/${name}`;
    return statSync(join(root, path)).isDirectory() ? walk(root, path) : [path];
  });

/** The texts of the repository at `root`, by id, and the problems of their files. */
export const readTexts = (root) =>
  parseTexts(
    walk(root, TEXTS_DIR).map((path) => [
      path,
      readFileSync(join(root, path), "utf8"),
    ]),
  );

/**
 * The texts of the repository at `root` as they were at `tag`, by id: from
 * the folder, or from texts/texts.json for a tag from before the split; none
 * for a tag from before the texts.
 */
export const readTextsAt = (root, tag) => {
  const git = (...args) =>
    execFileSync("git", ["-C", root, ...args], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
  const paths = git("ls-tree", "-r", "--name-only", tag, "--", TEXTS_DIR)
    .split("\n")
    .filter(Boolean);
  const legacy = `${TEXTS_DIR}/texts.json`;
  if (paths.includes(legacy))
    return JSON.parse(git("show", `${tag}:${legacy}`)).texts;
  return parseTexts(paths.map((path) => [path, git("show", `${tag}:${path}`)]))
    .texts;
};

const hash = (id) => createHash("sha256").update(id).digest("hex");

/** `texts` in the order the generated modules list them: by a hash of each id. */
export const mergeOrder = (texts) =>
  texts
    .map((t) => [hash(t.id), t])
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
    .map(([, t]) => t);
