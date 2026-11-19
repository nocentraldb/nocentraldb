// The install lines of the code section (NOC-360, the owner's point 13 of
// 2026-11-19), chosen at build time from release-notes/index.json and the
// repository's alpha tags: a release installs plainly, a beta by its exact
// version (cargo would otherwise pick crates.io's 0.0.0 placeholder), and
// before the first beta the alpha forms, with the newest alpha tag.
import assert from "node:assert/strict";
import { test } from "node:test";
import { installLines, newestAlpha, tagsOf } from "../src/install.mjs";

const GIT = "https://github.com/nocentraldb/nocentraldb";
const beta = {
  version: "0.1.0",
  stage: "beta",
  tag: "v0.1.0-beta.1",
  date: "2026-11-20",
  path: "release-notes/0.1.0.md",
};
const release = {
  version: "0.1.0",
  stage: "release",
  tag: "v0.1.0",
  date: "2026-12-01",
  path: "release-notes/0.1.0.md",
};

test("nothing indexed: npm's alpha tag, and the crate from the newest alpha tag", () => {
  const lines = installLines({
    releases: [],
    tags: ["v0.1.0-alpha.9", "v0.1.0-alpha.70", "v0.1.0-alpha.8"],
  });
  assert.equal(lines.npm, "npm install @nocentraldb/sdk@alpha");
  assert.equal(
    lines.cargo,
    `cargo add nocentraldb --git ${GIT} --tag v0.1.0-alpha.70`,
  );
  assert.equal(lines.stage, "alpha");
});

test("nothing indexed and no alpha tag found: the crate from the repository's main branch", () => {
  const lines = installLines({ releases: [], tags: [] });
  assert.equal(lines.npm, "npm install @nocentraldb/sdk@alpha");
  assert.equal(lines.cargo, `cargo add nocentraldb --git ${GIT}`);
});

test("a beta indexed: npm's beta tag, and the crate by its exact prerelease version", () => {
  const lines = installLines({ releases: [beta], tags: ["v0.1.0-alpha.70"] });
  assert.equal(lines.npm, "npm install @nocentraldb/sdk@beta");
  assert.equal(lines.cargo, "cargo add nocentraldb@0.1.0-beta.1");
  assert.equal(lines.stage, "beta");
});

test("a release indexed last: plain installs", () => {
  const lines = installLines({ releases: [beta, release], tags: [] });
  assert.equal(lines.npm, "npm install @nocentraldb/sdk");
  assert.equal(lines.cargo, "cargo add nocentraldb");
  assert.equal(lines.stage, "release");
});

test("the newest alpha is the highest version, then the highest alpha number", () => {
  assert.equal(
    newestAlpha([
      "v0.1.0-alpha.70",
      "v0.2.0-alpha.1",
      "v0.10.0-alpha.2",
      "v0.1.0-beta.1",
      "v0.9.0",
      "not-a-tag",
    ]),
    "v0.10.0-alpha.2",
  );
  assert.equal(newestAlpha([]), undefined);
});

test("tags are read from git ls-remote's output, peeled entries once", () => {
  const out = [
    "ff85a69\trefs/tags/v0.1.0-alpha.70",
    "aa11bb2\trefs/tags/v0.1.0-alpha.70^{}",
    "5e28d37\trefs/tags/v0.1.0-alpha.8",
    "",
  ].join("\n");
  assert.deepEqual(tagsOf(out), ["v0.1.0-alpha.70", "v0.1.0-alpha.8"]);
});
