#!/usr/bin/env node
// Checks that every manifest carries the one version of the project
// (docs/release-management.md, "Versioning"). The gate task repo:version runs
// it on the repository; the publish job runs it with --tag. Paths are
// relative to the working directory (the repository root; the tests run it in
// fixture trees). Rules:
//
// R1 Cargo.toml [workspace.package] has a version, and it is plain X.Y.Z:
//    committed manifests never carry a stage suffix (or build metadata).
// R2 every crates/*/Cargo.toml inherits it with version.workspace = true and
//    declares no own version in [package].
// R3 no crates/*/Cargo.toml declares a path dependency itself: dependencies
//    between workspace crates are declared in [workspace.dependencies] and
//    used as name.workspace = true.
// R4 every [workspace.dependencies] entry with a path is pinned to
//    version = "=<workspace version>", so prerelease versions resolve.
// R5 every packages/*/package.json and apps/*/package.json has the workspace
//    version.
// R6 Cargo.lock lists every workspace crate at the workspace version.
// R7 with --tag: the tag is vX.Y.Z or vX.Y.Z-beta.N (an alpha is never
//    checked against the manifests), and X.Y.Z is the workspace version.
//
// Exit 0 when every rule holds, 1 with one line per violation, 2 on a usage
// error.
import { existsSync, readFileSync } from "node:fs";
import {
  crateManifest,
  crateManifestFiles,
  lockVersions,
  packageJsonFiles,
  readWorkspaceVersion,
  workspaceDependencies,
} from "./lib/manifests.mjs";
import { classifyTag, PLAIN, plain } from "./lib/semver.mjs";

const USAGE = `usage: check-version [--tag <vX.Y.Z | vX.Y.Z-beta.N>]
Checks that Cargo.toml [workspace.package] holds a plain X.Y.Z, that every
crate inherits it, that dependencies between workspace crates are declared in
[workspace.dependencies] pinned to =X.Y.Z, that every workspace package.json
and Cargo.lock carry it, and with --tag that the tag is a beta or release tag
of that version.`;

const fail = (message) => {
  console.error(`check-version: ${message}`);
  process.exitCode = 1;
};

const usageError = () => {
  console.error(USAGE);
  process.exit(2);
};

const parseArgs = (args) => {
  const opts = {};
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--help") {
      console.log(USAGE);
      process.exit(0);
    } else if (args[i] === "--tag" && args[i + 1]) opts.tag = args[++i];
    else usageError();
  }
  return opts;
};

/** The file's text, or undefined after reporting it missing. */
const read = (file) => {
  if (existsSync(file)) return readFileSync(file, "utf8");
  fail(`${file}: missing`);
  return undefined;
};

/** fn(), or undefined after reporting its error against `file`. */
const attempt = (file, fn) => {
  try {
    return fn();
  } catch (error) {
    fail(`${file}: ${error.message}`);
    return undefined;
  }
};

const checkWorkspace = (cargoToml) => {
  const version = attempt("Cargo.toml", () => readWorkspaceVersion(cargoToml));
  if (version === undefined) return undefined;
  if (!PLAIN.test(version))
    fail(
      `Cargo.toml: [workspace.package] version "${version}" is not X.Y.Z: committed manifests never carry a stage suffix`,
    );
  const pin = `=${version}`;
  for (const dep of attempt("Cargo.toml", () =>
    workspaceDependencies(cargoToml),
  ) ?? []) {
    if (dep.version !== pin)
      fail(
        `Cargo.toml: [workspace.dependencies] ${dep.name} must be pinned with version = "${pin}" (found ${dep.version === null ? "none" : `"${dep.version}"`})`,
      );
  }
  return version;
};

/** The names of the workspace crates, after checking R2 and R3 for each. */
const checkCrates = () => {
  const names = [];
  for (const file of crateManifestFiles(".")) {
    const crate = attempt(file, () =>
      crateManifest(readFileSync(file, "utf8")),
    );
    if (!crate) continue;
    if (crate.name === null) fail(`${file}: [package] has no name`);
    else names.push(crate.name);
    if (crate.ownVersion !== null)
      fail(
        `${file}: [package] declares its own version "${crate.ownVersion}"; crates inherit the workspace version with version.workspace = true`,
      );
    else if (!crate.inheritsVersion)
      fail(
        `${file}: [package] must inherit the workspace version with version.workspace = true`,
      );
    for (const dep of crate.pathDependencies)
      fail(
        `${file}: dependency ${dep} is declared with path =; declare dependencies between workspace crates in [workspace.dependencies] of Cargo.toml and use ${dep}.workspace = true`,
      );
  }
  return names;
};

const checkPackages = (version) => {
  for (const file of packageJsonFiles(".")) {
    const pkg = attempt(file, () => JSON.parse(readFileSync(file, "utf8")));
    if (!pkg) continue;
    if (pkg.version !== version)
      fail(
        `${file}: version ${typeof pkg.version === "string" ? `"${pkg.version}"` : "none"} is not the workspace version "${version}"`,
      );
  }
};

const checkLock = (version, crates) => {
  const text = read("Cargo.lock");
  if (text === undefined) return;
  const locked = attempt("Cargo.lock", () => lockVersions(text));
  if (!locked) return;
  for (const name of crates) {
    if (!locked.has(name))
      fail(
        `Cargo.lock: does not list the workspace crate ${name} (run cargo update --workspace --offline)`,
      );
    else if (locked.get(name) !== version)
      fail(
        `Cargo.lock: ${name} is at ${locked.get(name)}, not at the workspace version ${version} (run cargo update --workspace --offline)`,
      );
  }
};

const checkTag = (tag, version) => {
  const t = classifyTag(tag);
  if (!t)
    return fail(
      `tag ${tag}: unknown tag shape (expected vX.Y.Z or vX.Y.Z-beta.N)`,
    );
  if (t.stage === "alpha")
    return fail(
      `tag ${tag}: alpha tags are not release tags; an alpha is never checked against the manifests`,
    );
  if (plain(t) !== version)
    fail(
      `tag ${tag}: its version ${plain(t)} is not the workspace version ${version}`,
    );
};

const main = () => {
  const opts = parseArgs(process.argv.slice(2));
  const cargoToml = read("Cargo.toml");
  const version =
    cargoToml === undefined ? undefined : checkWorkspace(cargoToml);
  const crates = checkCrates();
  if (version === undefined) return;
  checkPackages(version);
  checkLock(version, crates);
  if (opts.tag !== undefined) checkTag(opts.tag, version);
  if (!process.exitCode) console.log(`check-version: ${version}`);
};

main();
