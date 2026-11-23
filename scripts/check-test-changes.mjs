#!/usr/bin/env node
// The test-change check (NOC-508): a pull request that removes a test, skips
// one, removes an assertion or changes an expected value in one is flagged.
//
// usage: check-test-changes --base <rev> [--head <rev>]
//
// The pull request's test files are compared as they stand at the merge base
// of base and head and at head (lib/test-changes/).
//
// Exit 0 when nothing is flagged, 1 with one line per finding (a git error is
// a failure too, never a pass), 2 on a usage error.
import { realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { changedPaths, fileAt, mergeBase } from "./release/lib/git.mjs";
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

export function main(argv) {
  const args = parseArgs(argv);
  if (!args) {
    console.error(USAGE);
    return 2;
  }
  const from = mergeBase(args.base, args.head);
  const paths = changedPaths(args.base, args.head).filter(kindOf);
  const findings = compare(scansAt(from, paths), scansAt(args.head, paths));
  for (const f of findings) console.error(`${NAME}: ${f.path}: ${f.text}`);
  return findings.length ? 1 : 0;
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
