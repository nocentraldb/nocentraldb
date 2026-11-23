// Asserts that the relay crates `ncdb-relay` and `ncdb-relay-server` are
// prepared for crates.io (NOC-507): the release workflow's publish-crates
// job publishes every crate scripts/release/publishable.mjs lets through,
// so each of the two is publishable, carries the metadata crates.io shows
// and the two license texts, and is pinned in [workspace.dependencies].
// The Lambda and AWS crates stay unpublished: they are deployed to AWS, not
// used by an app. docs/release-management.md's crate table says the same.
// Agents never publish: the release workflow does, on the owner's tag.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { parse as parseToml } from "smol-toml";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const PUBLISHED = ["ncdb-relay", "ncdb-relay-server"];
const UNPUBLISHED = ["ncdb-relay-aws", "ncdb-relay-lambda"];

const manifest = (name) =>
  parseToml(readFileSync(join(root, "crates", name, "Cargo.toml"), "utf8"));

const publishable = () => {
  const r = spawnSync(
    process.execPath,
    [join(root, "scripts/release/publishable.mjs"), "--json"],
    { cwd: root, encoding: "utf8" },
  );
  assert.equal(r.status, 0, r.stderr);
  return new Map(
    JSON.parse(r.stdout).crates.map((c) => [c.name, c.publishable]),
  );
};

test("publishable lets the two relay crates through, and not the AWS ones", () => {
  const crates = publishable();
  for (const name of PUBLISHED) assert.equal(crates.get(name), true, name);
  for (const name of UNPUBLISHED) assert.equal(crates.get(name), false, name);
});

test("each relay crate carries crates.io's metadata and both license texts", () => {
  for (const name of PUBLISHED) {
    const pkg = manifest(name).package;
    assert.equal(pkg.publish ?? true, true, name);
    for (const key of ["license", "repository", "homepage", "readme"])
      assert.equal(pkg[key]?.workspace, true, `${name}: ${key}`);
    assert.ok(pkg.description?.length > 20, `${name}: description`);
    assert.ok(pkg.keywords?.length > 0, `${name}: keywords`);
    assert.ok(pkg.categories?.length > 0, `${name}: categories`);
    for (const file of ["LICENSE-MIT", "LICENSE-APACHE"])
      assert.ok(
        existsSync(join(root, "crates", name, file)),
        `${name}: ${file}`,
      );
  }
});

test("the workspace pins each relay crate to its version", () => {
  const workspace = parseToml(readFileSync(join(root, "Cargo.toml"), "utf8"));
  const version = workspace.workspace.package.version;
  for (const name of PUBLISHED)
    assert.equal(
      workspace.workspace.dependencies[name].version,
      `=${version}`,
      name,
    );
});

test("the crate table of docs/release-management.md publishes them", () => {
  const doc = readFileSync(join(root, "docs/release-management.md"), "utf8");
  for (const name of PUBLISHED) {
    const row = doc.split("\n").find((l) => l.startsWith(`| \`${name}\` |`));
    assert.ok(row, `no row of its own for ${name}`);
    assert.match(row, /^\| `[^`]+` \| yes \|/, name);
  }
});
