// The install lines of the code section (NOC-360, the owner's point 13 of
// 2026-11-19), chosen at build time from release-notes/index.json and the
// repository's alpha tags: a release installs plainly, a beta by its exact
// version (cargo would otherwise pick crates.io's 0.0.0 placeholder), and
// before the first beta the alpha forms, with the newest alpha tag.
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  installLines,
  newestAlpha,
  publishableCrate,
  remarkNpmLine,
  tagsOf,
  withNpmLine,
} from "../src/install.mjs";

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

test("a beta indexed and the crate publishable: npm's beta tag, and the crate by its exact prerelease version", () => {
  const lines = installLines({
    releases: [beta],
    tags: ["v0.1.0-alpha.70"],
    publishable: true,
  });
  assert.equal(lines.npm, "npm install @nocentraldb/sdk@beta");
  assert.equal(lines.cargo, "cargo add nocentraldb@0.1.0-beta.1");
  assert.equal(lines.stage, "beta");
});

test("a release indexed last and the crate publishable: plain installs", () => {
  const lines = installLines({
    releases: [beta, release],
    tags: [],
    publishable: true,
  });
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

test("a beta or release indexed but the crate not publishable: the crate from the indexed tag", () => {
  const fromBeta = installLines({
    releases: [beta],
    tags: ["v0.1.0-alpha.70"],
    publishable: false,
  });
  assert.equal(fromBeta.npm, "npm install @nocentraldb/sdk@beta");
  assert.equal(
    fromBeta.cargo,
    `cargo add nocentraldb --git ${GIT} --tag v0.1.0-beta.1`,
  );
  const fromRelease = installLines({
    releases: [beta, release],
    tags: [],
    publishable: false,
  });
  assert.equal(fromRelease.npm, "npm install @nocentraldb/sdk");
  assert.equal(
    fromRelease.cargo,
    `cargo add nocentraldb --git ${GIT} --tag v0.1.0`,
  );
});

test("the crate is publishable unless its manifest's package says publish = false", () => {
  assert.equal(
    publishableCrate('[package]\nname = "nocentraldb"\npublish = false\n'),
    false,
  );
  assert.equal(publishableCrate('[package]\nname = "nocentraldb"\n'), true);
  assert.equal(
    publishableCrate(
      '[package]\nname = "x"\n\n[dependencies]\npublish = false\n',
    ),
    true,
  );
});

// The docs' install lines follow the same index (NOC-361): an npm line for
// the SDK, written by hand in a docs page or in a file of docs/, is replaced
// by the current one, in the rendered page and in its Markdown copy.
test("an npm line for the SDK becomes the current one, whatever its tag", () => {
  const npm = "npm install @nocentraldb/sdk@beta";
  assert.equal(
    withNpmLine(
      "npm install @nocentraldb/sdk@alpha\nnpm install @nocentraldb/sdk\nnpm install @nocentraldb/sdk@0.1.0-beta.1 && x",
      npm,
    ),
    `${npm}\n${npm}\n${npm} && x`,
  );
  assert.equal(
    withNpmLine("npm install left-pad", npm),
    "npm install left-pad",
  );
});

test("the remark plugin rewrites the npm line in code, and leaves prose alone", () => {
  const npm = "npm install @nocentraldb/sdk";
  const tree = {
    type: "root",
    children: [
      { type: "code", value: "npm install @nocentraldb/sdk@alpha" },
      {
        type: "paragraph",
        children: [
          { type: "text", value: "npm install @nocentraldb/sdk@alpha" },
          { type: "inlineCode", value: "npm install @nocentraldb/sdk@alpha" },
        ],
      },
    ],
  };
  remarkNpmLine({ npm })(tree);
  assert.equal(tree.children[0].value, npm);
  assert.equal(
    tree.children[1].children[0].value,
    "npm install @nocentraldb/sdk@alpha",
  );
  assert.equal(tree.children[1].children[1].value, npm);
});
