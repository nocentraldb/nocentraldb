#!/usr/bin/env node
// Publishes the crates of a beta or a release to crates.io (NOC-374;
// docs/release-management.md, "Beta" and "Release"). The publish-crates job
// of .github/workflows/release.yml runs it twice on a checkout whose version
// set-version wrote: first with --verify-only, before it holds a crates.io
// token, then without, with the token. Paths are relative to the working
// directory (the repository root; the tests run it in fixture trees). Rules:
//
// C1 Only a beta or a release: --version is X.Y.Z-beta.N or X.Y.Z. An alpha
//    or anything else is a usage error: alphas never go to crates.io.
// C2 The crates are those scripts/release/publishable.mjs lists. One it
//    calls not publishable is reported as `skip <name>: <reason>`.
// C3 Every publishable crate is at --version: its own version, or the one
//    of [workspace.package] it inherits. One that is not fails the run
//    before anything is published.
// C4 They go out in dependency order: a crate after every publishable crate
//    it names as a workspace dependency, dev-dependencies included, since
//    crates.io must resolve a dev-dependency that keeps its version. Between
//    crates that do not depend on each other, publishable's order holds. A
//    cycle fails the run before anything is published.
// C5 Before anything is published, the crates.io sparse index is asked for
//    each crate. A version it already lists is reported as
//    `skip <name>@<version>: already on crates.io`, so a re-run publishes
//    only what is missing. An answer other than 200 or 404 fails the run
//    before anything is published.
// C6 --verify-only runs one `cargo package --locked --allow-dirty -p <crate>
//    …` over the missing crates, in order: cargo packages each one and builds
//    it from its package, against the packages of the crates before it. It
//    reports `would publish <name>@<version>` and publishes nothing.
//    --allow-dirty: set-version changed the manifests of the checkout.
// C7 Without it, each missing crate is published in order with `cargo
//    publish --locked --allow-dirty --no-verify -p <crate>`; --verify-only
//    built them already. cargo waits until crates.io serves a crate before
//    it returns, so the next one resolves it. cargo reads its token from
//    CARGO_REGISTRY_TOKEN, which the job sets from Trusted Publishing; this
//    script reads no token. The first failed publish fails the run.
//
// CRATES_PUBLISH_CARGO names the cargo to run and CRATES_PUBLISH_INDEX the
// sparse index, for the tests; they default to `cargo` and
// https://index.crates.io.
//
// Exit 0 when every crate is published, verified or skipped, 1 on a failure,
// named on stderr, 2 on a usage error.
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { cli } from "./lib/cli.mjs";
import { crateManifest, readWorkspaceVersion } from "./lib/manifests.mjs";

const USAGE = `usage: crates-publish --version <X.Y.Z-beta.N|X.Y.Z> [--verify-only]
Publishes every publishable crate of the workspace at that version to
crates.io, in dependency order, skipping a version crates.io already holds.
--verify-only packages and builds the missing crates and publishes nothing.`;

const { fail, usageError, parseArgs } = cli("crates-publish", USAGE);

const VERSION = /^\d+\.\d+\.\d+(?:-beta\.\d+)?$/;
const CARGO = process.env.CRATES_PUBLISH_CARGO || "cargo";
const INDEX = (
  process.env.CRATES_PUBLISH_INDEX || "https://index.crates.io"
).replace(/\/$/, "");
const PUBLISHABLE = join(
  dirname(fileURLToPath(import.meta.url)),
  "publishable.mjs",
);

/** publishable.mjs's crate entries, in its order; null after a failure. */
const crates = () => {
  const r = spawnSync(process.execPath, [PUBLISHABLE, "--json"], {
    encoding: "utf8",
  });
  if (r.status !== 0) {
    process.stderr.write(r.stderr);
    fail("publishable.mjs failed");
    return null;
  }
  return JSON.parse(r.stdout).crates;
};

/** `entries` in dependency order (C4); null after reporting a cycle. */
const dependencyOrder = (entries) => {
  const names = new Set(entries.map((e) => e.name));
  const needs = new Map(
    entries.map((e) => [
      e.name,
      e.manifest.workspaceDependencies.filter(
        (d) => names.has(d) && d !== e.name,
      ),
    ]),
  );
  const done = new Set();
  const order = [];
  let rest = entries;
  while (rest.length > 0) {
    const ready = rest.filter((e) =>
      needs.get(e.name).every((d) => done.has(d)),
    );
    if (ready.length === 0) {
      fail(
        `the published crates depend on each other in a cycle: ${rest.map((e) => e.name).join(", ")}`,
      );
      return null;
    }
    // One at a time, so publishable's order holds between independent crates.
    const [next] = ready;
    order.push(next);
    done.add(next.name);
    rest = rest.filter((e) => e !== next);
  }
  return order;
};

/** The path of a crate in the sparse index, as cargo builds it. */
const indexPath = (name) => {
  const n = name.toLowerCase();
  if (n.length <= 2) return `${n.length}/${n}`;
  if (n.length === 3) return `3/${n[0]}/${n}`;
  return `${n.slice(0, 2)}/${n.slice(2, 4)}/${n}`;
};

/** Whether the index lists `name` at `version`: true, false, or null after a failure. */
const published = async (name, version) => {
  let res;
  try {
    res = await fetch(`${INDEX}/${indexPath(name)}`);
  } catch (error) {
    fail(
      `the crates.io index could not be reached for ${name}: ${error.message}`,
    );
    return null;
  }
  if (res.status === 404) return false;
  if (res.status !== 200) {
    fail(`the crates.io index answered ${res.status} for ${name}`);
    return null;
  }
  return (await res.text())
    .split("\n")
    .filter((line) => line.trim() !== "")
    .some((line) => JSON.parse(line).vers === version);
};

const main = async () => {
  const opts = parseArgs(process.argv.slice(2), {
    flags: ["--verify-only"],
    values: ["--version"],
    defaults: { verifyOnly: false },
  });
  if (!VERSION.test(opts.version ?? "")) usageError();
  const { version } = opts;

  const entries = crates();
  if (entries === null) return;
  const workspaceVersion = readWorkspaceVersion(
    readFileSync("Cargo.toml", "utf8"),
  );
  const wanted = [];
  for (const entry of entries) {
    if (!entry.publishable) {
      console.log(`skip ${entry.name}: ${entry.reason}`);
      continue;
    }
    const manifest = crateManifest(
      readFileSync(join(entry.dir, "Cargo.toml"), "utf8"),
    );
    const at = manifest.inheritsVersion
      ? workspaceVersion
      : manifest.ownVersion;
    if (at !== version) {
      fail(
        `${entry.name} (${entry.dir}) is at ${at}, not ${version}: run set-version first`,
      );
      continue;
    }
    wanted.push({ ...entry, manifest });
  }
  if (process.exitCode) return;
  const ordered = dependencyOrder(wanted);
  if (ordered === null) return;

  const missing = [];
  for (const entry of ordered) {
    const there = await published(entry.name, version);
    if (there === null) return;
    if (there)
      console.log(`skip ${entry.name}@${version}: already on crates.io`);
    else missing.push(entry);
  }
  if (missing.length === 0) return;

  if (opts.verifyOnly) {
    const r = spawnSync(
      CARGO,
      [
        "package",
        "--locked",
        "--allow-dirty",
        ...missing.flatMap((e) => ["-p", e.name]),
      ],
      { stdio: ["ignore", "inherit", "inherit"] },
    );
    if (r.status !== 0) {
      fail("cargo package failed");
      return;
    }
    for (const entry of missing)
      console.log(`would publish ${entry.name}@${version}`);
    return;
  }

  for (const entry of missing) {
    const spec = `${entry.name}@${version}`;
    const r = spawnSync(
      CARGO,
      ["publish", "--locked", "--allow-dirty", "--no-verify", "-p", entry.name],
      { stdio: ["ignore", "inherit", "inherit"] },
    );
    if (r.status !== 0) {
      fail(`cargo publish of ${spec} failed`);
      return;
    }
    console.log(`published ${spec}`);
  }
};

await main();
