#!/usr/bin/env node
// Publishes every publishable npm package of an alpha, and stages those of a
// beta or a release (NOC-281, NOC-346, NOC-374; docs/release-management.md,
// "Alpha", "Beta" and "Approving a staged version on npm"). The alpha-npm and
// publish-npm jobs of .github/workflows/release.yml run it after the GitHub
// release exists, on a checkout whose version set-version wrote and whose
// packages are built. Paths are relative to the working directory (the
// repository root; the tests run it in fixture trees). Rules:
//
// A1 The dist-tag follows the version's stage: X.Y.Z-alpha.N goes under
//    alpha, X.Y.Z-beta.N under beta, X.Y.Z under latest. Any other version,
//    or a dist-tag that does not match it, is a usage error.
// A2 The packages are those scripts/release/publishable.mjs lists. One it
//    calls not publishable is reported as `skip <name>: <reason>`.
// A3 Every publishable package's package.json carries --version, and its
//    repository.url names the repository --repo gives, as
//    https://github.com/<owner>/<name>, with or without git+ and .git: npm
//    checks the provenance against it. A package that does not fails the run
//    before anything is published.
// A4 Before anything is published, the registry is asked for each
//    <name>@<version>. One already there is reported as
//    `skip <name>@<version>: already on npm`, so a re-run publishes only what
//    is missing. A lookup that fails for another reason than a 404 fails the
//    run before anything is published.
// A5 Each remaining package is sent from its directory. An alpha is
//    published directly, with `npm publish --tag alpha --provenance
//    --access public`. A beta or a release is staged, with `npm stage
//    publish --tag <beta or latest> --provenance --access public`: it reaches
//    npm only when the owner approves it with 2FA (NOC-346). npm
//    authenticates through Trusted Publishing, with the job's OIDC token; no
//    npm token is read or needed. The first failure fails the run. A4 cannot
//    see a version that is staged and not yet approved, so a re-run stages it
//    again.
// A6 --dry-run asks the registry as A4 does and reports
//    `would publish <name>@<version> under alpha` (or `would stage … under
//    <tag>`), and sends nothing.
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

const USAGE = `usage: npm-publish --version <version> --dist-tag <alpha|beta|latest> --repo <owner/name> [--dry-run]
Publishes every publishable npm package of the workspace at an alpha version
under alpha, or stages it at a beta version under beta or at a release
version under latest, with provenance, skipping a version npm already holds.
--dry-run only reports.`;

const { fail, usageError, parseArgs } = cli("npm-publish", USAGE);

/** The dist-tag a version goes under, by its stage; null for any other version. */
const distTagOf = (version) => {
  const m = /^\d+\.\d+\.\d+(?:-(alpha|beta)\.\d+)?$/.exec(version);
  return m ? (m[1] ?? "latest") : null;
};
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

/** A package's repository.url without git+ and .git, or null. */
const repositoryUrl = (pkg) => {
  const url = pkg.repository?.url;
  if (typeof url !== "string") return null;
  return url.replace(/^git\+/, "").replace(/\.git$/, "");
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
    values: ["--version", "--dist-tag", "--repo"],
    defaults: { dryRun: false },
  });
  if (
    distTagOf(opts.version ?? "") === null ||
    opts.distTag !== distTagOf(opts.version) ||
    !/^[\w.-]+\/[\w.-]+$/.test(opts.repo ?? "")
  )
    usageError();
  const { version, distTag } = opts;
  const home = `https://github.com/${opts.repo}`;

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
    if (repositoryUrl(pkg) !== home) {
      fail(
        `${entry.name} (${entry.dir}): its repository.url must name ${home}, which npm checks the provenance against`,
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

  // An alpha is published directly, a beta or a release staged (A5).
  const staged = distTag !== "alpha";
  const command = staged ? ["stage", "publish"] : ["publish"];
  for (const entry of missing) {
    const spec = `${entry.name}@${version}`;
    if (opts.dryRun) {
      console.log(
        `would ${staged ? "stage" : "publish"} ${spec} under ${distTag}`,
      );
      continue;
    }
    const r = spawnSync(
      NPM,
      [...command, "--tag", distTag, "--provenance", "--access", "public"],
      { cwd: resolve(entry.dir), stdio: ["ignore", "inherit", "inherit"] },
    );
    if (r.status !== 0) {
      fail(`npm ${command.join(" ")} of ${spec} failed`);
      return;
    }
    console.log(`${staged ? "staged" : "published"} ${spec} under ${distTag}`);
  }
};

main();
