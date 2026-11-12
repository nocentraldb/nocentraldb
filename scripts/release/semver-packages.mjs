#!/usr/bin/env node
// Which crates the changes check runs cargo-semver-checks on (NOC-253): the
// publishable crates of the head (as scripts/release/publishable.mjs decides,
// rule P2) that the base also has. A crate new in the pull request has no
// baseline, and the tool would fail on it, which the check reads as a
// breaking change. Paths are relative to the working directory, the head's
// repository root; --baseline is the base's checkout. Rules:
//
// S1 A crate is listed when it is publishable in the head and a crate of the
//    same name exists in the base, publishable there or not.
// S2 A publishable crate the base does not have is skipped, and named on
//    stderr.
// S3 The output is two lines for $GITHUB_OUTPUT: packages=<names, comma
//    separated, in name order> and any=<true when the list is not empty>.
// S4 A manifest that cannot be read fails the run, naming the file.
//
// Exit 0 with the output, 1 with one line per unreadable manifest, 2 on a
// usage error.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { cli } from "./lib/cli.mjs";
import {
  crateManifest,
  crateManifestFiles,
  publishLine,
} from "./lib/manifests.mjs";

const USAGE = `usage: semver-packages --baseline <dir>
Prints packages=<crates> and any=<true|false> for $GITHUB_OUTPUT: the
publishable crates of the working directory that the base checkout at <dir>
also has, which cargo-semver-checks can compare.`;

const { fail, usageError, parseArgs } = cli("semver-packages", USAGE);

/** { name, publishable } of every crate manifest under root; null on error. */
const crates = (root) =>
  crateManifestFiles(root).map((file) => {
    const path = join(root, file);
    try {
      const text = readFileSync(path, "utf8");
      const publish = publishLine(text);
      return {
        name: crateManifest(text).name,
        publishable: publish === null || publish === "true",
      };
    } catch (error) {
      fail(`${path}: ${error.message}`);
      return null;
    }
  });

const main = () => {
  const opts = parseArgs(process.argv.slice(2), { values: ["--baseline"] });
  if (!opts.baseline) usageError();
  const head = crates(".");
  const base = crates(opts.baseline);
  if (process.exitCode) return;
  const inBase = new Set(base.map((crate) => crate.name));
  const listed = [];
  for (const { name } of head.filter((crate) => crate.publishable)) {
    if (inBase.has(name)) listed.push(name);
    else
      console.error(
        `semver-packages: ${name} is not in the base, so it has no baseline; skipped`,
      );
  }
  listed.sort();
  console.log(`packages=${listed.join(",")}`);
  console.log(`any=${listed.length > 0}`);
};

main();
