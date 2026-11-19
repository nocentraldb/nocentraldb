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
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

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

/**
 * The npm and cargo lines for `releases` (index.json's), the repository's
 * `tags`, and whether the Rust crate is `publishable` on crates.io; a crate
 * that is not takes the indexed tag, or the newest alpha tag, from git.
 */
export const installLines = ({ releases, tags, publishable = false }) => {
  const last = releases.at(-1);
  if (last?.stage === "release")
    return {
      stage: "release",
      npm: "npm install @nocentraldb/sdk",
      cargo: publishable
        ? "cargo add nocentraldb"
        : `cargo add nocentraldb --git ${GIT} --tag ${last.tag}`,
    };
  if (last?.stage === "beta")
    return {
      stage: "beta",
      npm: "npm install @nocentraldb/sdk@beta",
      cargo: publishable
        ? `cargo add nocentraldb@${last.tag.slice(1)}`
        : `cargo add nocentraldb --git ${GIT} --tag ${last.tag}`,
    };
  const tag = newestAlpha(tags);
  return {
    stage: "alpha",
    npm: "npm install @nocentraldb/sdk@alpha",
    cargo: `cargo add nocentraldb --git ${GIT}${tag ? ` --tag ${tag}` : ""}`,
  };
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

/** The install lines for the repository at `root`, as the site is built. */
export const currentInstallLines = (root) => {
  const { releases } = JSON.parse(
    readFileSync(new URL("release-notes/index.json", root), "utf8"),
  );
  const last = releases.at(-1);
  // A beta or a release needs no tags; only the alpha forms read them.
  const tags = last ? [] : repositoryTags(root);
  const publishable = publishableCrate(
    readFileSync(new URL("crates/nocentraldb/Cargo.toml", root), "utf8"),
  );
  return installLines({ releases, tags, publishable });
};
