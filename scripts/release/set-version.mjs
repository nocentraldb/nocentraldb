#!/usr/bin/env node
// Writes one full version (X.Y.Z, X.Y.Z-alpha.N or X.Y.Z-beta.N) into the
// manifests of a checkout (docs/release-management.md, "Versioning"): the
// alpha and publish jobs run it in their build checkout, the cut runs it on
// the release branch. What it writes:
//
// - Cargo.toml: the version of [workspace.package] (the crates inherit it),
//   and the pin version = "=<version>" of every [workspace.dependencies]
//   entry with a path, inserted where it is missing, so a prerelease version
//   resolves between workspace crates.
// - every packages/*/package.json and apps/*/package.json: the value of its
//   top-level "version", and no other byte of the file.
// - Cargo.lock, unless --no-lock: the version of every workspace member,
//   rewritten directly, the way cargo update --workspace writes it. It needs
//   no cargo, registry index or network, so it works in a fresh CI runner
//   without a cargo cache (NOC-281).
//
// Everything is read and validated before anything is written: an invalid
// version or a manifest or lock the reader cannot parse leaves every file as
// it was, so a rewritten manifest never stays beside a stale lock.
// Exit 0 on success, 1 with a message naming the version or the file, 2 on a
// usage error.
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import {
  packageJsonFiles,
  pinWorkspaceDependencies,
  writeLockVersions,
  writePackageVersion,
  writeWorkspaceVersion,
} from "./lib/manifests.mjs";
import { parseVersion } from "./lib/semver.mjs";

const USAGE = `usage: set-version <version> [--no-lock]
Writes <version> (X.Y.Z, X.Y.Z-alpha.N or X.Y.Z-beta.N) into Cargo.toml
[workspace.package], pins every [workspace.dependencies] path entry to
=<version>, sets "version" in every packages/*/package.json and
apps/*/package.json, and sets the version of every workspace member in
Cargo.lock (skipped with --no-lock).`;

const fail = (message) => {
  console.error(`set-version: ${message}`);
  process.exitCode = 1;
};

const usageError = () => {
  console.error(USAGE);
  process.exit(2);
};

const parseArgs = (args) => {
  const opts = { lock: true };
  for (const arg of args) {
    if (arg === "--help") {
      console.log(USAGE);
      process.exit(0);
    } else if (arg === "--no-lock") opts.lock = false;
    else if (!arg.startsWith("--") && opts.version === undefined)
      opts.version = arg;
    else usageError();
  }
  if (opts.version === undefined) usageError();
  return opts;
};

/** fn(), or throws with the file named. */
const at = (file, fn) => {
  try {
    return fn();
  } catch (error) {
    throw new Error(`${file}: ${error.message}`);
  }
};

const main = () => {
  const { version, lock } = parseArgs(process.argv.slice(2));
  try {
    parseVersion(version);
    // Read and rewrite everything in memory first; write only when all of it
    // succeeded.
    const writes = [];
    const cargo = readFileSync("Cargo.toml", "utf8");
    writes.push([
      "Cargo.toml",
      at("Cargo.toml", () =>
        pinWorkspaceDependencies(
          writeWorkspaceVersion(cargo, version),
          version,
        ),
      ),
    ]);
    for (const file of packageJsonFiles(".")) {
      const text = readFileSync(file, "utf8");
      writes.push([file, at(file, () => writePackageVersion(text, version))]);
    }
    const manifests = writes.length;
    if (lock && existsSync("Cargo.lock")) {
      const text = readFileSync("Cargo.lock", "utf8");
      writes.push([
        "Cargo.lock",
        at("Cargo.lock", () => writeLockVersions(text, version)),
      ]);
    }
    for (const [file, text] of writes) writeFileSync(file, text);
    console.log(`set-version: ${version} written to ${manifests} manifests`);
  } catch (error) {
    fail(error.message);
  }
};

main();
