#!/usr/bin/env node
// Checks change fragments (changes/<slug>.md) against the rules in
// docs/release-management.md, "Writing a change fragment". Two modes:
//
// Schema mode (default; the gate task repo:fragments): every fragment in the
// directory passes the schema: slug, front matter fields and values, the body
// sections its type requires. README.md in changes/ is the writer's guide and
// is not a fragment. Paths are relative to the working directory (the
// repository root; the tests run it in fixture trees).
//
// PR mode (--base <rev> --head <rev>; the `changes` check, changes.yml):
// - The PR's changes are diffed from the merge base of base and head, so
//   commits on the base branch after the fork never count. Paths are read
//   with -z; a rename counts as its old and its new path.
// - Every fragment the PR adds or modifies is validated as it stands at head.
// - Required fragment: when the PR changes a path outside ALLOWLIST and adds
//   no fragment (a new changes/<slug>.md; a modified or renamed one is not
//   new), each such path is named, followed once by how to add a fragment.
//   --skip (the PR's skip-changes label) waives this rule and nothing else.
// - --require-breaking (a detected breaking change): the PR must add a
//   fragment of type breaking. --skip does not waive it.
//
// Exit 0 when every rule holds, 1 with one line per violation (a git error
// is a violation too, never a pass), 2 on a usage error.
import { existsSync, readFileSync, realpathSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import {
  COMPONENTS,
  listFragments,
  parseFragment,
  SLUG,
  TYPES,
  validateFragment,
} from "./lib/fragments.mjs";
import { addedPaths, changedPaths, git, mergeBase } from "./lib/git.mjs";
import { matchesAny } from "./lib/glob.mjs";

/** Paths that ship nothing to a user: no fragment needed (epic §12). */
export const ALLOWLIST = [
  "docs/**",
  "README.md",
  "AGENTS.md",
  "CONTRIBUTING.md",
  "NEXUS_MEMORY.md",
  "factory/**",
  "LICENSE-*",
  ".github/**",
  "scripts/**",
  "moon.yml",
  "**/moon.yml",
  ".moon/**",
  "biome.json",
  "tsconfig.base.json",
  ".prototools",
  ".nvmrc",
  ".gitignore",
  "**/tests/**",
  "**/test/**",
  "**/*.test.*",
  "changes/**",
  "release-notes/**",
];

const FRAGMENT = /^changes\/([a-z0-9][a-z0-9-]*)\.md$/;

const USAGE = `usage: check-fragments [--dir <changes>]
       check-fragments --base <rev> --head <rev> [--skip] [--require-breaking]
Without --base/--head, validates every fragment in <dir> (default changes/).
With them, checks the pull request from <base> to <head>: a change outside the
allowlist needs an added fragment under changes/ (--skip waives that: the
skip-changes label), every added or modified fragment must be valid, and with
--require-breaking an added fragment of type breaking is required (--skip does
not waive that).`;

const fail = (message) => {
  console.error(`check-fragments: ${message}`);
  process.exitCode = 1;
};

const usageError = () => {
  console.error(USAGE);
  process.exit(2);
};

const parseArgs = (args) => {
  const opts = { dir: "changes", skip: false, requireBreaking: false };
  for (let i = 0; i < args.length; i++) {
    const value = () => (args[i + 1] ? args[++i] : usageError());
    if (args[i] === "--help") {
      console.log(USAGE);
      process.exit(0);
    } else if (args[i] === "--dir") opts.dir = value();
    else if (args[i] === "--base") opts.base = value();
    else if (args[i] === "--head") opts.head = value();
    else if (args[i] === "--skip") opts.skip = true;
    else if (args[i] === "--require-breaking") opts.requireBreaking = true;
    else usageError();
  }
  const pr = opts.base !== undefined || opts.head !== undefined;
  if (pr && (opts.base === undefined || opts.head === undefined)) usageError();
  if (!pr && (opts.skip || opts.requireBreaking)) usageError();
  return { ...opts, pr };
};

/**
 * How to add a fragment, printed once after the paths that need one: an
 * outside contributor may have nothing but the CI log.
 */
const HOW_TO = `check-fragments: to add a fragment, create changes/<slug>.md, its slug matching ${SLUG.source} (e.g. changes/add-sync-retry.md), starting with this front matter:
  ---
  type: <one of ${TYPES.join(", ")}>
  tickets: [<ticket id>]
  issues: [<GitHub issue number>]   (optional)
  components: [<one or more of ${COMPONENTS.join(", ")}>]
  ---
then the body sections its type requires (a fix also needs introduced:). The writer's guide with a template: changes/README.md; the full rules: docs/release-management.md#writing-a-change-fragment`;

const checkDirectory = (dir) => {
  if (!existsSync(dir)) {
    console.log(`check-fragments: no ${dir}/ directory, nothing to check`);
    return;
  }
  if (!statSync(dir).isDirectory())
    return fail(`--dir ${dir} is not a directory`);
  for (const { slug, file } of listFragments(dir)) {
    let text;
    try {
      text = readFileSync(file, "utf8");
    } catch (error) {
      fail(`${file}: ${error.message}`);
      continue;
    }
    for (const rule of validateFragment(slug, text)) fail(`${file}: ${rule}`);
  }
};

const typeOf = (text) => {
  try {
    return parseFragment(text).front.type;
  } catch {
    return undefined; // reported by the validation already
  }
};

const checkPullRequest = ({ base, head, skip, requireBreaking }) => {
  const show = (path) => git(["show", `${head}:${path}`]);
  const added = addedPaths(mergeBase(base, head), head, "changes/").filter(
    (path) => FRAGMENT.test(path),
  );
  // Every fragment the PR adds or modifies, as it stands at head. A deleted
  // one is gone at head and has nothing to validate.
  for (const path of changedPaths(base, head, { filter: "ACMR" })) {
    if (!FRAGMENT.test(path)) continue;
    for (const rule of validateFragment(FRAGMENT.exec(path)[1], show(path)))
      fail(`${path}: ${rule}`);
  }
  if (added.length === 0 && !skip) {
    const shipping = changedPaths(base, head).filter(
      (path) => !matchesAny(path, ALLOWLIST),
    );
    for (const path of shipping)
      fail(
        `${path} ships to a user and the pull request adds no fragment under changes/ (add one, or label the PR skip-changes)`,
      );
    if (shipping.length > 0) console.error(HOW_TO);
  }
  if (
    requireBreaking &&
    !added.some((path) => typeOf(show(path)) === "breaking")
  )
    fail(
      "a breaking change was detected and no breaking fragment was added (the skip-changes label does not waive this)",
    );
};

const main = () => {
  const opts = parseArgs(process.argv.slice(2));
  if (!opts.pr) return checkDirectory(opts.dir);
  try {
    checkPullRequest(opts);
  } catch (error) {
    fail(error.message);
  }
};

// Run only as a script; the tests import ALLOWLIST.
if (
  process.argv[1] &&
  realpathSync(process.argv[1]) === realpathSync(fileURLToPath(import.meta.url))
)
  main();
