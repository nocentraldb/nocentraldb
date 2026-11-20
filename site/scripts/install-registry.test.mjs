// The install lines wait for the registries (NOC-373): a line that names npm's
// dist-tag for the newest indexed beta or release appears only once npm
// serves that version under it, and a line that names a crates.io version
// only once crates.io serves it. Until then, or when a registry cannot be
// reached, each line stays the previous one: the line of the version indexed
// before, or the alpha line before the first beta.
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { pathToFileURL } from "node:url";
import { currentInstallLines, installLines } from "../src/install.mjs";

const GIT = "https://github.com/nocentraldb/nocentraldb";
const entry = (stage, tag) => ({
  version: tag.slice(1).replace(/-.*/, ""),
  stage,
  tag,
  date: "2026-11-20",
  path: `release-notes/${tag.slice(1).replace(/-.*/, "")}.md`,
});
const beta1 = entry("beta", "v0.1.0-beta.1");
const beta2 = entry("beta", "v0.1.0-beta.2");
const release = entry("release", "v0.1.0");
const ALPHA_TAGS = ["v0.1.0-alpha.73", "v0.1.0-alpha.8"];
const ALPHA = {
  npm: "npm install @nocentraldb/sdk@alpha",
  cargo: `cargo add nocentraldb --git ${GIT} --tag v0.1.0-alpha.73`,
};

const lines = (releases, served, publishable = true) =>
  installLines({ releases, tags: ALPHA_TAGS, publishable, served });

test("the first beta, staged on npm and not yet on crates.io: the alpha lines", () => {
  const got = lines([beta1], { npm: false, crate: false });
  assert.equal(got.npm, ALPHA.npm);
  assert.equal(got.cargo, ALPHA.cargo);
  assert.equal(got.stage, "alpha");
  assert.equal(got.cargoStage, "alpha");
});

test("the first beta on crates.io, still staged on npm: each line on its own", () => {
  const got = lines([beta1], { npm: false, crate: true });
  assert.equal(got.npm, ALPHA.npm);
  assert.equal(got.stage, "alpha");
  assert.equal(got.cargo, "cargo add nocentraldb@0.1.0-beta.1");
  assert.equal(got.cargoStage, "beta");
});

test("the first beta approved on npm, not yet on crates.io: npm's beta, the crate's alpha line", () => {
  const got = lines([beta1], { npm: true, crate: false });
  assert.equal(got.npm, "npm install @nocentraldb/sdk@beta");
  assert.equal(got.stage, "beta");
  assert.equal(got.cargo, ALPHA.cargo);
  assert.equal(got.cargoStage, "alpha");
});

test("a second beta not yet served: the first beta's lines", () => {
  const got = lines([beta1, beta2], { npm: false, crate: false });
  assert.equal(got.npm, "npm install @nocentraldb/sdk@beta");
  assert.equal(got.cargo, "cargo add nocentraldb@0.1.0-beta.1");
  assert.equal(got.stage, "beta");
  assert.equal(got.cargoStage, "beta");
});

test("a release staged on npm and on crates.io: the beta's npm line, the plain crate", () => {
  const got = lines([beta1, beta2, release], { npm: false, crate: true });
  assert.equal(got.npm, "npm install @nocentraldb/sdk@beta");
  assert.equal(got.stage, "beta");
  assert.equal(got.cargo, "cargo add nocentraldb");
  assert.equal(got.cargoStage, "release");
});

test("a crate that is not publishable comes from the newest indexed tag in git, whatever crates.io serves", () => {
  const got = lines([beta1, beta2], { npm: true, crate: false }, false);
  assert.equal(got.cargo, `cargo add nocentraldb --git ${GIT} --tag v0.1.0-beta.2`);
  assert.equal(got.cargoStage, "beta");
});

test("tags are read only when a line falls back to the alpha form", () => {
  let reads = 0;
  const tags = () => {
    reads += 1;
    return ALPHA_TAGS;
  };
  installLines({
    releases: [beta1],
    tags,
    publishable: true,
    served: { npm: true, crate: true },
  });
  assert.equal(reads, 0);
  const got = installLines({
    releases: [beta1],
    tags,
    publishable: true,
    served: { npm: false, crate: false },
  });
  assert.equal(got.cargo, ALPHA.cargo);
  assert.ok(reads >= 1);
});

/** A repository root with `releases` indexed and the crate's manifest. */
const repository = (releases, manifest = '[package]\nname = "nocentraldb"\n') => {
  const root = mkdtempSync(join(tmpdir(), "ncdb-install-"));
  mkdirSync(join(root, "release-notes"));
  mkdirSync(join(root, "crates/nocentraldb"), { recursive: true });
  writeFileSync(
    join(root, "release-notes/index.json"),
    JSON.stringify({ releases }),
  );
  writeFileSync(join(root, "crates/nocentraldb/Cargo.toml"), manifest);
  return pathToFileURL(`${root}/`);
};

test("the build asks the registries for the newest indexed version under its dist-tag", () => {
  const asked = [];
  const probe = (query) => {
    asked.push(query);
    return { npm: true, crate: true };
  };
  const got = currentInstallLines(repository([beta1, beta2]), { probe });
  assert.deepEqual(asked, [
    {
      npm: { distTag: "beta", version: "0.1.0-beta.2" },
      crate: "0.1.0-beta.2",
    },
  ]);
  assert.equal(got.cargo, "cargo add nocentraldb@0.1.0-beta.2");

  asked.length = 0;
  currentInstallLines(
    repository([beta1, release], '[package]\nname = "x"\npublish = false\n'),
    { probe },
  );
  assert.deepEqual(asked, [
    { npm: { distTag: "latest", version: "0.1.0" }, crate: null },
  ]);
});

test("nothing indexed: no registry is asked", () => {
  const probe = () => {
    throw new Error("asked");
  };
  const got = currentInstallLines(repository([]), { probe, tags: () => [] });
  assert.equal(got.npm, ALPHA.npm);
});

test("a registry that cannot be reached: the build still succeeds, with the previous lines", () => {
  for (const probe of [
    () => ({ npm: false, crate: false }),
    () => {
      throw new Error("getaddrinfo ENOTFOUND registry.npmjs.org");
    },
  ]) {
    const got = currentInstallLines(repository([beta1, beta2]), { probe });
    assert.equal(got.npm, "npm install @nocentraldb/sdk@beta");
    assert.equal(got.cargo, "cargo add nocentraldb@0.1.0-beta.1");
  }
});
