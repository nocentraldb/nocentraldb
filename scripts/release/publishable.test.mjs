import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const script = join(dirname(fileURLToPath(import.meta.url)), "publishable.mjs");

/** A workspace tree in a temporary directory: path -> text. */
const tree = (t, files) => {
  const dir = mkdtempSync(join(tmpdir(), "release-publishable-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  for (const [file, text] of Object.entries(files)) {
    mkdirSync(join(dir, dirname(file)), { recursive: true });
    writeFileSync(join(dir, file), text);
  }
  return dir;
};

const pkg = (fields) =>
  `${JSON.stringify({ version: "0.1.0", ...fields }, null, 2)}\n`;
const crate = (name, extra = "") =>
  `[package]\nname = "${name}"\nversion.workspace = true\n${extra}`;

const run = (dir, ...args) =>
  spawnSync(process.execPath, [script, ...args], {
    cwd: dir,
    encoding: "utf8",
  });

const FIXTURE = {
  "packages/private/package.json": pkg({
    name: "@x/private",
    private: true,
    exports: "./dist/index.js",
  }),
  "packages/public/package.json": pkg({
    name: "@x/public",
    exports: "./dist/index.js",
  }),
  "packages/source/package.json": pkg({
    name: "@x/source",
    main: "./src/index.ts",
  }),
  "apps/conditional/package.json": pkg({
    name: "conditional",
    exports: { ".": { import: "./dist/index.mjs", types: "./dist/x.d.ts" } },
  }),
  "apps/main-dist/package.json": pkg({
    name: "main-dist",
    main: "dist/index.js",
  }),
  "crates/closed/Cargo.toml": crate("closed", "publish = false\n"),
  "crates/open/Cargo.toml": crate("open"),
  "crates/registry/Cargo.toml": crate("registry", 'publish = ["my-reg"]\n'),
};

test("--json reports every workspace package and crate with publishable and a reason", (t) => {
  const r = run(tree(t, FIXTURE), "--json");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stderr, "");
  assert.deepEqual(JSON.parse(r.stdout), {
    npm: [
      {
        name: "conditional",
        dir: "apps/conditional",
        publishable: true,
        reason: "public, builds to dist/",
      },
      {
        name: "main-dist",
        dir: "apps/main-dist",
        publishable: true,
        reason: "public, builds to dist/",
      },
      {
        name: "@x/private",
        dir: "packages/private",
        publishable: false,
        reason: "private: true",
      },
      {
        name: "@x/public",
        dir: "packages/public",
        publishable: true,
        reason: "public, builds to dist/",
      },
      {
        name: "@x/source",
        dir: "packages/source",
        publishable: false,
        reason: "no build output under dist/ (exports or main)",
      },
    ],
    crates: [
      {
        name: "closed",
        dir: "crates/closed",
        publishable: false,
        reason: "publish = false",
      },
      {
        name: "open",
        dir: "crates/open",
        publishable: true,
        reason: "publish is not restricted",
      },
      {
        name: "registry",
        dir: "crates/registry",
        publishable: false,
        reason:
          'publish = ["my-reg"] (only a crate without publish, or with publish = true, goes to crates.io)',
      },
    ],
  });
});

test("without --json it prints one line per package", (t) => {
  const r = run(
    tree(t, {
      "packages/private/package.json": FIXTURE["packages/private/package.json"],
      "crates/open/Cargo.toml": FIXTURE["crates/open/Cargo.toml"],
    }),
  );
  assert.equal(r.status, 0, r.stderr);
  assert.equal(
    r.stdout,
    "npm @x/private (packages/private): not publishable, private: true\ncrate open (crates/open): publishable, publish is not restricted\n",
  );
});

test("an empty workspace reports empty lists", (t) => {
  const r = run(tree(t, { "README.md": "x\n" }), "--json");
  assert.equal(r.status, 0, r.stderr);
  assert.deepEqual(JSON.parse(r.stdout), { npm: [], crates: [] });
});

test("an unreadable manifest fails naming the file", (t) => {
  const r = run(
    tree(t, {
      "packages/bad/package.json": "{ nope",
      "crates/bad/Cargo.toml": '[package]\nname = "bad"\npublish = [\n]\n',
    }),
    "--json",
  );
  assert.equal(r.status, 1);
  assert.equal(r.stdout, "");
  assert.match(r.stderr, /^publishable: packages\/bad\/package\.json: /m);
  assert.match(
    r.stderr,
    /^publishable: crates\/bad\/Cargo\.toml: unsupported manifest syntax at line 3$/m,
  );
});

test("--help prints the usage; anything else is a usage error", (t) => {
  const dir = tree(t, {});
  const help = run(dir, "--help");
  assert.equal(help.status, 0);
  assert.match(help.stdout, /^usage: publishable/);
  const r = run(dir, "--nope");
  assert.equal(r.status, 2);
  assert.match(r.stderr, /^usage: publishable/);
});
