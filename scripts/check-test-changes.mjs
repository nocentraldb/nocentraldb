#!/usr/bin/env node
// The test-change check (NOC-508): a pull request that removes a test, skips
// one, removes an assertion or changes an expected value in one is flagged.
//
// usage: check-test-changes --base <rev> [--head <rev>]
//
// The pull request's test files are compared as they stand at the merge base
// of base and head and at head (lib/test-changes/). A flagged pull request
// passes when one of its own commits (base..head, so never a commit of the
// base branch merged into it) carries a `Test-change: <reason>` trailer whose
// reason has at least two words; the check then prints the findings and the
// reasons, for pm's review and the daily note.
//
// Exit 0 when nothing is flagged or a reason is given, 1 with one line per
// finding (a git error is a failure too, never a pass), 2 on a usage error.
import { realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { changedPaths, fileAt, git, mergeBase } from "./release/lib/git.mjs";
import { compare } from "./lib/test-changes/compare.mjs";
import { kindOf } from "./lib/test-changes/files.mjs";
import { scan } from "./lib/test-changes/scan.mjs";

const NAME = "check-test-changes";
const USAGE = `usage: ${NAME} --base <rev> [--head <rev>]`;

function parseArgs(argv) {
  const args = { head: "HEAD" };
  for (let i = 0; i < argv.length; i += 1) {
    const flag = argv[i];
    if (flag === "--base" || flag === "--head") {
      if (argv[i + 1] === undefined) return null;
      args[flag.slice(2)] = argv[(i += 1)];
    } else {
      return null;
    }
  }
  return args.base ? args : null;
}

/** The scans of `paths` as they stand at `ref`; a file absent there is left out. */
function scansAt(ref, paths) {
  const out = [];
  for (const path of paths) {
    const text = fileAt(ref, path);
    if (text !== null) out.push({ path, scan: scan(text, kindOf(path)) });
  }
  return out;
}

/**
 * A reason in words: at least two words of two letters or more, and not a
 * placeholder in angle brackets such as `<reason>`.
 */
export const isReason = (text) =>
  !/^<.*>$/.test(text) && (text.match(/\p{L}{2,}/gu) ?? []).length >= 2;

/** The `Test-change:` reasons of the commits in base..head, each with its commit. */
function reasons(base, head) {
  const log = git([
    "log",
    "--format=%h%x1f%(trailers:key=Test-change,valueonly,separator=%x1e)%x00",
    `${base}..${head}`,
  ]);
  return log
    .split("\0")
    .filter((entry) => entry.trim())
    .flatMap((entry) => {
      const [commit, values = ""] = entry.trim().split("\x1f");
      return values
        .split("\x1e")
        .map((value) => value.trim())
        .filter(isReason)
        .map((reason) => ({ commit, reason }));
    });
}

const HOW = `The pull request removes, skips or loosens tests (above). If that is
intended, say why in words, in a trailer of one of its commits (the last
paragraph of the message):

    Test-change: <reason>

pm reads the reason in review, and the daily note lists the pull request
(factory/FACTORY.md, "Rules for every coding agent"). If it is not intended,
restore the tests.`;

export function main(argv) {
  const args = parseArgs(argv);
  if (!args) {
    console.error(USAGE);
    return 2;
  }
  const from = mergeBase(args.base, args.head);
  const paths = changedPaths(args.base, args.head).filter(kindOf);
  const findings = compare(scansAt(from, paths), scansAt(args.head, paths));
  if (findings.length === 0) return 0;
  const given = reasons(args.base, args.head);
  const print = given.length ? console.log : console.error;
  for (const f of findings) print(`${NAME}: ${f.path}: ${f.text}`);
  if (given.length === 0) {
    console.error(`\n${HOW}`);
    return 1;
  }
  for (const { commit, reason } of given) {
    print(`${NAME}: Test-change: ${reason} (${commit})`);
    if (process.env.GITHUB_ACTIONS === "true") {
      print(`::notice title=Test-change::${reason} (${commit})`);
    }
  }
  return 0;
}

const invoked = process.argv[1] && realpathSync(process.argv[1]);
if (invoked === realpathSync(fileURLToPath(import.meta.url))) {
  try {
    process.exitCode = main(process.argv.slice(2));
  } catch (error) {
    console.error(`${NAME}: ${error.message}`);
    process.exitCode = 1;
  }
}
