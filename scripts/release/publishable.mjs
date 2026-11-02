#!/usr/bin/env node
// Which workspace packages the publish steps may publish
// (docs/release-management.md, "Versioning" and "Supply chain and
// policies"): the publish job skips a registry step for a package that is not
// publishable, without failing. Paths are relative to the working directory
// (the repository root; the tests run it in fixture trees). Rules:
//
// P1 Every packages/*/package.json and apps/*/package.json is listed. An npm
//    package is publishable when it is not "private" and its "exports" (any
//    target, conditional exports included) or "main" point under dist/.
// P2 Every crates/*/Cargo.toml is listed. A crate is publishable when its
//    [package] has no publish key or publish = true; publish = false, a
//    registry list or anything else keeps it off crates.io.
// P3 Each entry carries its name, its directory, publishable and the reason.
// P4 A manifest that cannot be read fails the run, naming the file.
//
// Exit 0 with the report (with --json { npm: [...], crates: [...] }), 1 with
// one line per unreadable manifest, 2 on a usage error.
import { readFileSync } from "node:fs";
import { dirname } from "node:path";
import { cli } from "./lib/cli.mjs";
import {
  crateManifest,
  crateManifestFiles,
  packageJsonFiles,
  publishLine,
} from "./lib/manifests.mjs";

const USAGE = `usage: publishable [--json]
Reports every workspace npm package and crate with whether the publish steps
may publish it, and why: an npm package that is not private and builds to
dist/, a crate without publish = false.`;

const { fail, parseArgs } = cli("publishable", USAGE);

const UNDER_DIST = /^(?:\.\/)?dist\//;

/** Every string target of an exports value: a path, or nested conditions. */
const targets = (value) =>
  typeof value === "string"
    ? [value]
    : value !== null && typeof value === "object"
      ? Object.values(value).flatMap(targets)
      : [];

const npmEntry = (file) => {
  const pkg = JSON.parse(readFileSync(file, "utf8"));
  const entry = { name: pkg.name, dir: dirname(file) };
  if (pkg.private === true)
    return { ...entry, publishable: false, reason: "private: true" };
  const built = [...targets(pkg.exports), ...targets(pkg.main)].some((t) =>
    UNDER_DIST.test(t),
  );
  return built
    ? { ...entry, publishable: true, reason: "public, builds to dist/" }
    : {
        ...entry,
        publishable: false,
        reason: "no build output under dist/ (exports or main)",
      };
};

const crateEntry = (file) => {
  const text = readFileSync(file, "utf8");
  const entry = { name: crateManifest(text).name, dir: dirname(file) };
  const publish = publishLine(text);
  if (publish === null || publish === "true")
    return { ...entry, publishable: true, reason: "publish is not restricted" };
  return {
    ...entry,
    publishable: false,
    reason:
      publish === "false"
        ? "publish = false"
        : `publish = ${publish} (only a crate without publish, or with publish = true, goes to crates.io)`,
  };
};

/** read(file) for every file; null entries after reporting the error against the file. */
const readAll = (files, read) =>
  files.map((file) => {
    try {
      return read(file);
    } catch (error) {
      fail(`${file}: ${error.message}`);
      return null;
    }
  });

const line = (kind, { name, dir, publishable, reason }) =>
  `${kind} ${name} (${dir}): ${publishable ? "publishable" : "not publishable"}, ${reason}`;

const main = () => {
  const opts = parseArgs(process.argv.slice(2), {
    flags: ["--json"],
    defaults: { json: false },
  });
  const npm = readAll(packageJsonFiles("."), npmEntry);
  const crates = readAll(crateManifestFiles("."), crateEntry);
  if (process.exitCode) return;
  if (opts.json) console.log(JSON.stringify({ npm, crates }, null, 2));
  else
    for (const out of [
      ...npm.map((e) => line("npm", e)),
      ...crates.map((e) => line("crate", e)),
    ])
      console.log(out);
};

main();
