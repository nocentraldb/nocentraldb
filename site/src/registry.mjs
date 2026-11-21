// Whether the registries serve a version (NOC-373). commit-back indexes a
// beta or release in release-notes/index.json as soon as npm has it staged,
// but npm serves it only once the owner approves it; the site shows a line
// that names a registry's version only once that registry serves it
// (install.mjs). A registry that cannot be reached, or answers anything but
// the version, serves nothing: the build never fails for it.
//
// The site reads its install lines synchronously in several places of one
// build, so `cachedProbe` runs this file as a child process and keeps its
// answer for an hour: every page of a build, and the output check after it,
// show the same lines.
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";

const NPM_DIST_TAGS =
  "https://registry.npmjs.org/-/package/@nocentraldb%2fsdk/dist-tags";
const CRATE = "https://crates.io/api/v1/crates/nocentraldb";
// crates.io refuses a request without a User-Agent that names its sender.
const USER_AGENT =
  "nocentraldb-site-build (https://github.com/nocentraldb/nocentraldb)";
const TIMEOUT_MS = 10_000;

/** The JSON body of `url`, or undefined for any failure. */
const json = async (fetchImpl, url, headers = {}) => {
  try {
    const res = await fetchImpl(url, {
      headers,
      signal: AbortSignal.timeout(TIMEOUT_MS),
    });
    return res.ok ? await res.json() : undefined;
  } catch {
    return undefined;
  }
};

/**
 * Whether npm serves `npm.version` under `npm.distTag`, and crates.io the
 * crate's version `crate`, unyanked; a query of null asks nothing.
 */
export const servedVersions = async ({ npm, crate }, fetchImpl = fetch) => {
  const [tags, version] = await Promise.all([
    npm ? json(fetchImpl, NPM_DIST_TAGS) : undefined,
    crate
      ? json(fetchImpl, `${CRATE}/${crate}`, { "User-Agent": USER_AGENT })
      : undefined,
  ]);
  return {
    npm: Boolean(npm) && tags?.[npm.distTag] === npm.version,
    crate:
      Boolean(crate) &&
      version?.version?.num === crate &&
      version.version.yanked === false,
  };
};

const SELF = fileURLToPath(import.meta.url);
const HOUR_MS = 60 * 60_000;

/** `servedVersions(query)` in a child process, so a synchronous caller can ask. */
const runChild = (query, script) =>
  JSON.parse(
    execFileSync(process.execPath, [script, JSON.stringify(query)], {
      encoding: "utf8",
      timeout: 3 * TIMEOUT_MS,
      stdio: ["ignore", "pipe", "ignore"],
    }),
  );

/**
 * A synchronous probe: `query` → `{ npm, crate }`, from this file run as
 * `script` (its path in the repository: Astro bundles the module that
 * imports it). It keeps each answer in `file` for an hour; a failed run
 * answers that nothing is served, and that answer is kept too, so one build
 * never shows two lines.
 */
export const cachedProbe = ({
  file,
  script = SELF,
  run = (query) => runChild(query, script),
  now = Date.now,
}) => {
  const read = () => {
    try {
      const cache = JSON.parse(readFileSync(file, "utf8"));
      return cache && typeof cache === "object" ? cache : {};
    } catch {
      return {};
    }
  };
  return (query) => {
    const key = JSON.stringify(query);
    const cache = read();
    const kept = cache[key];
    if (kept && now() - kept.at < HOUR_MS) return kept.served;
    let served;
    try {
      const answer = run(query);
      served = { npm: answer?.npm === true, crate: answer?.crate === true };
    } catch {
      served = { npm: false, crate: false };
    }
    try {
      mkdirSync(dirname(file), { recursive: true });
      writeFileSync(
        file,
        JSON.stringify({ ...cache, [key]: { at: now(), served } }),
      );
    } catch {}
    return served;
  };
};

// As a child process: the query as JSON in the first argument, the answer as
// JSON on stdout.
if (process.argv[1] === SELF) {
  const served = await servedVersions(JSON.parse(process.argv[2]));
  process.stdout.write(JSON.stringify(served));
}
