#!/usr/bin/env node
// Publishes the alpha of every publishable npm package (NOC-281;
// docs/release-management.md, "Alpha"). The alpha-npm job of
// .github/workflows/release.yml runs it after the GitHub prerelease exists,
// on a checkout whose version set-version wrote and whose packages are
// built. Paths are relative to the working directory (the repository root;
// the tests run it in fixture trees). Rules:
//
// A1 Only an alpha: --version is X.Y.Z-alpha.N and --dist-tag is alpha.
//    Anything else is a usage error. Betas and releases publish from the
//    publish job, which this script does not serve yet.
// A2 The packages are those scripts/release/publishable.mjs lists. One it
//    calls not publishable is reported as `skip <name>: <reason>`.
// A3 Every publishable package's package.json carries --version; one that
//    does not fails the run before anything is published.
// A4 Before anything is published, the registry is asked for each
//    <name>@<version>. One already there is reported as
//    `skip <name>@<version>: already on npm`, so a re-run publishes only what
//    is missing. A lookup that fails for another reason than a 404 fails the
//    run before anything is published.
// A5 Each remaining package is published from its directory with
//    `npm publish --tag alpha --provenance --access public`. npm
//    authenticates through Trusted Publishing, with the job's OIDC token; no
//    npm token is read or needed. The first failed publish fails the run.
// A6 --dry-run asks the registry as A4 does and reports
//    `would publish <name>@<version> under alpha`, and publishes nothing.
//
// The environment variable NPM_PUBLISH_NPM names the npm to run, for the
// tests; it defaults to `npm`.
//
// Exit 0 when every package is published or skipped, 1 on a failure, named
// on stderr, 2 on a usage error.
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { cli } from "./lib/cli.mjs";

const USAGE = `usage: npm-publish --version <X.Y.Z-alpha.N> --dist-tag alpha [--dry-run]
Publishes every publishable npm package of the workspace at that version
under the dist-tag alpha, with provenance, skipping a version npm already
holds. --dry-run only reports.`;

const { fail, usageError, parseArgs } = cli("npm-publish", USAGE);

const ALPHA = /^\d+\.\d+\.\d+-alpha\.\d+$/;
const NPM = process.env.NPM_PUBLISH_NPM || "npm";
const PUBLISHABLE = join(
  dirname(fileURLToPath(import.meta.url)),
  "publishable.mjs",
);

/** publishable.mjs's npm entries, in its order; null after a failure. */
const packages = () => {
  const r = spawnSync(process.execPath, [PUBLISHABLE, "--json"], {
    encoding: "utf8",
  });
  if (r.status !== 0) {
    process.stderr.write(r.stderr);
    fail("publishable.mjs failed");
    return null;
  }
  return JSON.parse(r.stdout).npm;
};

/** Whether npm holds `spec`: true, false on a 404, null after a failure. */
const published = (spec) => {
  const r = spawnSync(NPM, ["view", spec, "version", "--json"], {
    encoding: "utf8",
  });
  if (r.status === 0) return r.stdout.trim() !== "";
  if (/\bE404\b/.test(r.stderr)) return false;
  process.stderr.write(r.stderr);
  fail(`npm view ${spec} failed`);
  return null;
};

const main = () => {
  const opts = parseArgs(process.argv.slice(2), {
    flags: ["--dry-run"],
    values: ["--version", "--dist-tag"],
    defaults: { dryRun: false },
  });
  if (!ALPHA.test(opts.version ?? "") || opts.distTag !== "alpha") usageError();
  const { version, distTag } = opts;

  const entries = packages();
  if (entries === null) return;
  const wanted = [];
  for (const entry of entries) {
    if (!entry.publishable) {
      console.log(`skip ${entry.name}: ${entry.reason}`);
      continue;
    }
    const pkg = JSON.parse(
      readFileSync(join(entry.dir, "package.json"), "utf8"),
    );
    if (pkg.version !== version) {
      fail(
        `${entry.name} (${entry.dir}) is at ${pkg.version}, not ${version}: run set-version first`,
      );
      continue;
    }
    wanted.push(entry);
  }
  if (process.exitCode) return;

  const missing = [];
  for (const entry of wanted) {
    const spec = `${entry.name}@${version}`;
    const there = published(spec);
    if (there === null) return;
    if (there) console.log(`skip ${spec}: already on npm`);
    else missing.push(entry);
  }

  for (const entry of missing) {
    const spec = `${entry.name}@${version}`;
    if (opts.dryRun) {
      console.log(`would publish ${spec} under ${distTag}`);
      continue;
    }
    const r = spawnSync(
      NPM,
      ["publish", "--tag", distTag, "--provenance", "--access", "public"],
      { cwd: resolve(entry.dir), stdio: ["ignore", "inherit", "inherit"] },
    );
    if (r.status !== 0) {
      fail(`npm publish of ${spec} failed`);
      return;
    }
    console.log(`published ${spec} under ${distTag}`);
  }
};

main();
