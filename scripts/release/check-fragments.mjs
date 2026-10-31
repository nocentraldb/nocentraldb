#!/usr/bin/env node
// Fails when a change fragment under changes/ breaks the schema in
// docs/release-management.md: slug, front matter fields and values, the body
// sections its type requires. README.md in changes/ is the writer's guide and
// is not a fragment. Paths are relative to the working directory (the
// repository root; the tests run it in fixture trees). Exit 0 when every
// fragment is valid, 1 with one line per violation, 2 on a usage error.
import { existsSync, readFileSync } from "node:fs";
import { listFragments, validateFragment } from "./lib/fragments.mjs";

const USAGE = `usage: check-fragments [--dir <changes>]
Validates every fragment in <dir> (default changes/).`;

const fail = (message) => {
  console.error(`check-fragments: ${message}`);
  process.exitCode = 1;
};

const args = process.argv.slice(2);
let dir = "changes";
for (let i = 0; i < args.length; i++) {
  if (args[i] === "--help") {
    console.log(USAGE);
    process.exit(0);
  } else if (args[i] === "--dir" && args[i + 1]) {
    dir = args[++i];
  } else {
    console.error(USAGE);
    process.exit(2);
  }
}

if (!existsSync(dir)) {
  console.log(`check-fragments: no ${dir}/ directory, nothing to check`);
} else {
  for (const { slug, file } of listFragments(dir)) {
    for (const rule of validateFragment(slug, readFileSync(file, "utf8")))
      fail(`${file}: ${rule}`);
  }
}
