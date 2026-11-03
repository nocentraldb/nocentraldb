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
// - Cargo.lock: refreshed with cargo update --workspace --offline, unless
//   --no-lock. When cargo fails, every file is restored as it was before the
//   run and the error says so: a rewritten manifest never stays beside a
//   stale lock.
//
// Everything is read and validated before anything is written: an invalid
// version or a manifest the reader cannot parse leaves every file as it was.
// Exit 0 on success, 1 with a message naming the version or the file, 2 on a
// usage error.
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import {
  packageJsonFiles,
  pinWorkspaceDependencies,
  writePackageVersion,
  writeWorkspaceVersion,
} from "./lib/manifests.mjs";
import { parseVersion } from "./lib/semver.mjs";

const USAGE = `usage: set-version <version> [--no-lock]
Writes <version> (X.Y.Z, X.Y.Z-alpha.N or X.Y.Z-beta.N) into Cargo.toml
[workspace.package], pins every [workspace.dependencies] path entry to
=<version>, sets "version" in every packages/*/package.json and
apps/*/package.json, then refreshes Cargo.lock with
cargo update --workspace --offline (skipped with --no-lock).`;

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
    const originals = new Map([["Cargo.toml", cargo]]);
    for (const file of packageJsonFiles(".")) {
      const text = readFileSync(file, "utf8");
      originals.set(file, text);
      writes.push([file, at(file, () => writePackageVersion(text, version))]);
    }
    if (lock && existsSync("Cargo.lock"))
      originals.set("Cargo.lock", readFileSync("Cargo.lock", "utf8"));
    for (const [file, text] of writes) writeFileSync(file, text);
    if (lock) {
      try {
        execFileSync("cargo", ["update", "--workspace", "--offline"], {
          stdio: "inherit",
        });
      } catch {
        // cargo said why above. A rewritten manifest beside a stale lock
        // fails every --locked build, so the tree goes back as it was.
        for (const [file, text] of originals) writeFileSync(file, text);
        throw new Error(
          `cargo update --workspace --offline failed, so Cargo.lock could not follow ${version}; every manifest was restored as it was`,
        );
      }
    }
    console.log(
      `set-version: ${version} written to ${writes.length} manifests`,
    );
  } catch (error) {
    fail(error.message);
  }
};

main();
