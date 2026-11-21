// The install lines of the landing page's code section, chosen when the site
// is built (NOC-360, the owner's point 13 of 2026-11-19). Betas and releases
// reach the registries; alphas reach npm under the dist-tag `alpha` and never
// crates.io (docs/release-management.md). So the lines follow
// release-notes/index.json, which lists every published beta and release:
//   - a release last: plain installs;
//   - a beta last: npm's dist-tag `beta`, and the crate by its exact
//     prerelease version, since cargo otherwise picks crates.io's 0.0.0
//     placeholder;
//   - with a beta or a release indexed but the crate not publishable
//     (`publish = false` in its manifest), the crate from that tag in git;
//   - nothing indexed: npm's dist-tag `alpha`, and the crate from the
//     repository at the newest alpha tag, or at its main branch when no tag
//     can be read.
// A line that names a registry's version waits until that registry serves it
// (NOC-373, registry.mjs): npm stages a beta or release until the owner
// approves it, and the index lists it at once. Until then the line stays the
// one of the version indexed before, or the alpha's.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { cachedProbe } from "./registry.mjs";

const GIT = "https://github.com/nocentraldb/nocentraldb";
const ALPHA = /^v(\d+)\.(\d+)\.(\d+)-alpha\.(\d+)$/;

/** The newest of `tags` of the form vX.Y.Z-alpha.N, by version, or undefined. */
export const newestAlpha = (tags) =>
  tags
    .map((tag) => [tag, tag.match(ALPHA)])
    .filter(([, m]) => m)
    .map(([tag, m]) => [tag, m.slice(1).map(Number)])
    .sort(([, a], [, b]) => {
      for (let i = 0; i < 4; i += 1) if (a[i] !== b[i]) return b[i] - a[i];
      return 0;
    })[0]?.[0];

/** The tag names in `git ls-remote --tags` output, each once. */
export const tagsOf = (out) => [
  ...new Set(
    out
      .split("\n")
      .map((line) => line.split("\t")[1] ?? "")
      .filter((ref) => ref.startsWith("refs/tags/"))
      .map((ref) => ref.slice("refs/tags/".length).replace(/\^\{\}$/, "")),
  ),
];

/**
 * Whether a Cargo.toml's [package] lets the crate reach crates.io: unless it
 * says `publish = false`, as crates/nocentraldb does until the owner opens it.
 */
export const publishableCrate = (manifest) => {
  const start = manifest.search(/^\[package\]\s*$/m);
  if (start < 0) return true;
  const rest = manifest.slice(start).split("\n").slice(1);
  const end = rest.findIndex((line) => /^\[/.test(line));
  const section = (end < 0 ? rest : rest.slice(0, end)).join("\n");
  return !/^publish\s*=\s*false\s*$/m.test(section);
};

/** The npm line for an indexed `entry`, or for the alpha before any. */
const npmLine = (entry) => {
  if (entry?.stage === "release")
    return { stage: "release", npm: "npm install @nocentraldb/sdk" };
  if (entry?.stage === "beta")
    return { stage: "beta", npm: "npm install @nocentraldb/sdk@beta" };
  return { stage: "alpha", npm: "npm install @nocentraldb/sdk@alpha" };
};

/** The crates.io line for an indexed `entry`, or the alpha's from git. */
const cargoLine = (entry, alphaTag) => {
  if (entry?.stage === "release")
    return { cargoStage: "release", cargo: "cargo add nocentraldb" };
  if (entry?.stage === "beta")
    return {
      cargoStage: "beta",
      cargo: `cargo add nocentraldb@${entry.tag.slice(1)}`,
    };
  const tag = alphaTag();
  return {
    cargoStage: "alpha",
    cargo: `cargo add nocentraldb --git ${GIT}${tag ? ` --tag ${tag}` : ""}`,
  };
};

/**
 * The npm and cargo lines for `releases` (index.json's), the repository's
 * `tags` (or a function that reads them, called only for an alpha line),
 * whether the Rust crate is `publishable` on crates.io, and whether npm and
 * crates.io serve the newest indexed version (`served`, NOC-373). A line
 * names the newest indexed version only once its registry serves it, and is
 * the previous line until then: the version indexed before, or the alpha's.
 * A crate that is not publishable takes the newest indexed tag from git.
 * `stage` is the npm line's, `cargoStage` the cargo line's.
 */
export const installLines = ({
  releases,
  tags,
  publishable = false,
  served = {},
}) => {
  const last = releases.at(-1);
  const previous = releases.at(-2);
  const alphaTag = () => newestAlpha(typeof tags === "function" ? tags() : tags);
  const cargo =
    last && !publishable
      ? {
          cargoStage: last.stage,
          cargo: `cargo add nocentraldb --git ${GIT} --tag ${last.tag}`,
        }
      : cargoLine(served.crate ? last : previous, alphaTag);
  return { ...npmLine(served.npm ? last : previous), ...cargo };
};

/** The repository's tags: the remote's, else the checkout's, else none. */
const repositoryTags = (root) => {
  for (const args of [
    ["ls-remote", "--tags", "origin"],
    ["tag", "--list"],
  ]) {
    try {
      const out = execFileSync("git", args, {
        cwd: root,
        encoding: "utf8",
        timeout: 15_000,
        stdio: ["ignore", "pipe", "ignore"],
      });
      const tags =
        args[0] === "tag" ? out.split("\n").filter(Boolean) : tagsOf(out);
      if (newestAlpha(tags)) return tags;
    } catch {}
  }
  return [];
};

/** What the newest indexed `entry` asks the registries (registry.mjs). */
const registryQuery = (entry, publishable) => ({
  npm: {
    distTag: entry.stage === "release" ? "latest" : "beta",
    version: entry.tag.slice(1),
  },
  crate: publishable ? entry.tag.slice(1) : null,
});

/**
 * The install lines for the repository at `root`, as the site is built.
 * `probe` answers whether the registries serve the newest indexed version;
 * it is asked only when something is indexed, and a probe that fails leaves
 * the previous lines. `tags` reads the repository's tags for an alpha line.
 */
export const currentInstallLines = (
  root,
  {
    probe = cachedProbe({
      file: fileURLToPath(
        new URL("site/node_modules/.cache/ncdb-registry.json", root),
      ),
      script: fileURLToPath(new URL("site/src/registry.mjs", root)),
    }),
    tags = () => repositoryTags(root),
  } = {},
) => {
  const { releases } = JSON.parse(
    readFileSync(new URL("release-notes/index.json", root), "utf8"),
  );
  const publishable = publishableCrate(
    readFileSync(new URL("crates/nocentraldb/Cargo.toml", root), "utf8"),
  );
  const last = releases.at(-1);
  let served = {};
  if (last)
    try {
      served = probe(registryQuery(last, publishable));
    } catch {}
  return installLines({ releases, tags, publishable, served });
};

// The docs' npm lines (NOC-361): a page of the docs, or a file of docs/ that
// the site renders, writes the SDK's npm line by hand; the site shows the
// current one instead, as the landing page's code section does.
const NPM_LINE = /npm install @nocentraldb\/sdk(?:@[\w.-]+)?/g;

/** `text` with every npm line for the SDK replaced by `npm`. */
export const withNpmLine = (text, npm) => text.replace(NPM_LINE, npm);

/**
 * An Expressive Code plugin that writes `npm` for the SDK's npm line in the
 * docs' code blocks. Astro's Markdown processor runs no remark plugins, and
 * Starlight renders every code block through Expressive Code.
 */
export const expressiveNpmLine = ({ npm }) => ({
  name: "ncdb-npm-line",
  hooks: {
    preprocessCode: ({ codeBlock }) => {
      for (const line of codeBlock.getLines()) {
        const text = withNpmLine(line.text, npm);
        if (text !== line.text) line.editText(0, line.text.length, text);
      }
    },
  },
});
