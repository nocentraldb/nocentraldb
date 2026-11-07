// Runs check-pins.mjs against throwaway fixture trees and asserts its exit
// code and message for every rule it enforces.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";

const script = join(dirname(fileURLToPath(import.meta.url)), "check-pins.mjs");
const fixtures = [];
after(() => {
  for (const dir of fixtures) rmSync(dir, { recursive: true, force: true });
});

const PROTOTOOLS = 'moon = "2.6.5"\nnode = "24.22.2"\npnpm = "12.17.0"\n';

/** Writes a valid tree, applies the overrides (null deletes a file), runs the check. */
function run(overrides = {}) {
  const dir = mkdtempSync(join(tmpdir(), "check-pins-"));
  fixtures.push(dir);
  const files = {
    ".prototools": PROTOTOOLS,
    ".nvmrc": "24.22.2\n",
    "package.json": {
      packageManager: "pnpm@12.17.0",
      devDependencies: { vitest: "5.0.5" },
    },
    "packages/sdk/package.json": { name: "@x/sdk" },
    ...overrides,
  };
  for (const [file, content] of Object.entries(files)) {
    if (content === null) continue;
    mkdirSync(dirname(join(dir, file)), { recursive: true });
    const text =
      typeof content === "string" ? content : JSON.stringify(content);
    writeFileSync(join(dir, file), text);
  }
  const result = spawnSync(process.execPath, [script], {
    cwd: dir,
    encoding: "utf8",
  });
  return { status: result.status, stderr: result.stderr };
}

const withDep = (spec, field = "dependencies") => ({
  name: "@x/sdk",
  [field]: { dep: spec },
});

test("a fully pinned tree passes", () => {
  assert.deepEqual(run(), { status: 0, stderr: "" });
});

test("a range in the root package.json fails", () => {
  const { status, stderr } = run({
    "package.json": {
      packageManager: "pnpm@12.17.0",
      devDependencies: { vitest: "^5.0.5" },
    },
  });
  assert.equal(status, 1);
  assert.match(
    stderr,
    /package\.json devDependencies\.vitest = "\^5\.0\.5" is not exact/,
  );
});

test("a range in packages/*/package.json fails", () => {
  const { status, stderr } = run({
    "packages/sdk/package.json": withDep("~1.0.0"),
  });
  assert.equal(status, 1);
  assert.match(
    stderr,
    /packages\/sdk\/package\.json dependencies\.dep = "~1\.0\.0" is not exact/,
  );
});

test("a range in apps/*/package.json fails", () => {
  const { status, stderr } = run({
    "apps/site/package.json": withDep(">=1.0.0", "peerDependencies"),
  });
  assert.equal(status, 1);
  assert.match(stderr, /apps\/site\/package\.json peerDependencies\.dep/);
});

for (const spec of [
  "^1.0.0",
  "~1.0.0",
  ">=1.0.0",
  "1.0.0 - 2.0.0",
  "1.0.0 || 2.0.0",
  "*",
  "x",
  "1",
  "1.x",
  "1.2",
  "latest",
  "next",
  "git+https://github.com/a/b.git#main",
  "github:a/b",
  "file:../b",
  "link:../b",
  "npm:other@^1.0.0",
  "https://example.com/b.tgz",
  "",
]) {
  test(`the non-exact spec "${spec}" fails`, () => {
    const { status, stderr } = run({
      "packages/sdk/package.json": withDep(spec, "optionalDependencies"),
    });
    assert.equal(status, 1);
    assert.match(stderr, /optionalDependencies\.dep = .* is not exact/);
  });
}

for (const spec of ["1.2.3", "0.0.0", "1.2.3-beta.1", "1.2.3-rc.0+build.5"]) {
  test(`the exact version "${spec}" passes`, () => {
    const { status } = run({ "packages/sdk/package.json": withDep(spec) });
    assert.equal(status, 0);
  });
}

test("the workspace protocol passes: it names a project in this repo", () => {
  const { status } = run({
    "packages/sdk/package.json": withDep("workspace:*"),
  });
  assert.equal(status, 0);
});

test(".nvmrc differing from the node pin fails", () => {
  const { status, stderr } = run({ ".nvmrc": "24.22.1\n" });
  assert.equal(status, 1);
  assert.match(stderr, /\.nvmrc 24\.22\.1 != \.prototools node 24\.22\.2/);
});

test("packageManager differing from the pnpm pin fails", () => {
  const { status, stderr } = run({
    "package.json": { packageManager: "pnpm@12.16.1" },
  });
  assert.equal(status, 1);
  assert.match(
    stderr,
    /packageManager pnpm@12\.16\.1 != pnpm@12\.17\.0 from \.prototools/,
  );
});

for (const key of ["node", "pnpm"]) {
  test(`a .prototools without a ${key} pin fails with a missing-key message`, () => {
    const prototools = PROTOTOOLS.replace(new RegExp(`^${key} .*\\n`, "m"), "");
    const { status, stderr } = run({ ".prototools": prototools });
    assert.equal(status, 1);
    assert.match(stderr, new RegExp(`\\.prototools has no ${key} pin`));
    assert.doesNotMatch(stderr, /undefined/);
  });
}

test("a missing .nvmrc fails with a message, not a stack trace", () => {
  const { status, stderr } = run({ ".nvmrc": null });
  assert.equal(status, 1);
  assert.match(stderr, /check-pins: cannot read \.nvmrc/);
});

// pnpm overrides and catalogs: a range there would bypass the exact-pin rule.
// The repository has none yet, so these fixtures build them.
const WORKSPACE_YAML =
  'packages:\n  - "apps/*"\n  - "packages/*"\nsaveExact: true\n';
const workspaceWith = (yaml) => ({
  "pnpm-workspace.yaml": WORKSPACE_YAML + yaml,
});
const rootWith = (extra) => ({
  "package.json": { packageManager: "pnpm@12.17.0", ...extra },
});

test("a tree without pnpm-workspace.yaml passes: nothing to check there", () => {
  assert.deepEqual(run({ "pnpm-workspace.yaml": null }), {
    status: 0,
    stderr: "",
  });
});

test("exact versions in every override and catalog field pass", () => {
  const { status, stderr } = run({
    ...rootWith({ pnpm: { overrides: { "foo@<2": "1.2.3", "a>b": "4.5.6" } } }),
    ...workspaceWith(
      [
        "overrides:",
        '  "@scope/foo": "1.2.3" # a comment',
        "  bar@^1: '2.0.0-rc.1'",
        "  baz: 3.0.0",
        "catalog:",
        "  react: 19.2.0",
        "catalogs:",
        "  # a comment line",
        "  legacy:",
        "    react: 18.3.1",
        "",
        "    vue: 3.5.0",
        "  empty: {}",
        "",
      ].join("\n"),
    ),
  });
  assert.deepEqual({ status, stderr }, { status: 0, stderr: "" });
});

test("an empty overrides mapping passes", () => {
  const { status } = run(workspaceWith("overrides: {}\ncatalog:\n"));
  assert.equal(status, 0);
});

test("a range in package.json pnpm.overrides fails", () => {
  const { status, stderr } = run(
    rootWith({ pnpm: { overrides: { "foo@<2": "^1.2.3" } } }),
  );
  assert.equal(status, 1);
  assert.match(
    stderr,
    /package\.json pnpm\.overrides\.foo@<2 = "\^1\.2\.3" is not exact/,
  );
});

test("a range in package.json resolutions fails: pnpm honours it like overrides", () => {
  const { status, stderr } = run(rootWith({ resolutions: { foo: "~1.0.0" } }));
  assert.equal(status, 1);
  assert.match(
    stderr,
    /package\.json resolutions\.foo = "~1\.0\.0" is not exact/,
  );
});

for (const [name, yaml, message] of [
  [
    "overrides",
    'overrides:\n  "@scope/foo": "^1.2.3"\n',
    /pnpm-workspace\.yaml overrides\.@scope\/foo = "\^1\.2\.3" is not exact/,
  ],
  [
    "catalog",
    "catalog:\n  react: ~19.2.0\n",
    /pnpm-workspace\.yaml catalog\.react = "~19\.2\.0" is not exact/,
  ],
  [
    "catalogs",
    "catalogs:\n  legacy:\n    react: 18.3.1\n    vue: 'latest'\n",
    /pnpm-workspace\.yaml catalogs\.legacy\.vue = "latest" is not exact/,
  ],
]) {
  test(`a non-exact version in pnpm-workspace.yaml ${name} fails`, () => {
    const { status, stderr } = run(workspaceWith(yaml));
    assert.equal(status, 1);
    assert.match(stderr, message);
  });
}

test("an entry named __proto__ is checked, not lost on the prototype", () => {
  const { status, stderr } = run(
    workspaceWith("overrides:\n  __proto__: ^1.0.0\n"),
  );
  assert.equal(status, 1);
  assert.match(stderr, /overrides\.__proto__ = "\^1\.0\.0" is not exact/);
});

test("an override with no value fails as not exact", () => {
  const { status, stderr } = run(workspaceWith("overrides:\n  foo:\n"));
  assert.equal(status, 1);
  assert.match(stderr, /overrides\.foo = "" is not exact/);
});

test("a catalog that is not a mapping of names to versions fails", () => {
  const { status, stderr } = run(workspaceWith("catalogs:\n  legacy: 1.2.3\n"));
  assert.equal(status, 1);
  assert.match(
    stderr,
    /pnpm-workspace\.yaml catalogs\.legacy is not a mapping/,
  );
});

// Fail closed: anything outside the narrow YAML subset check-pins reads in
// these sections is an error, never silently skipped.
for (const [name, yaml] of [
  ["a flow mapping", "overrides: { foo: ^1.0.0 }\n"],
  ["a flow mapping entry", "catalog:\n  react: { version: ^1 }\n"],
  ["an alias", "overrides:\n  foo: *range\n"],
  ["an anchor", "catalog:\n  react: &r ^19.0.0\n"],
  ["a block scalar", "catalog:\n  react: |\n    ^19.0.0\n"],
  ["a merge key", "catalog:\n  <<: *base\n"],
  ["a tag", "catalog:\n  react: !!str ^19.0.0\n"],
  ["a sequence", "overrides:\n- foo\n"],
  ["a continuation line", "overrides:\n  foo: 1.0.0\n    ^2.0.0\n"],
  ["inconsistent indentation", "catalog:\n    react: 19.2.0\n  vue: ^3.0.0\n"],
  ["a tab", "catalog:\n\treact: ^19.0.0\n"],
  ["an escape in a double-quoted value", 'catalog:\n  react: "\\x5e19.0.0"\n'],
  ["a complex key", "? overrides\n: { foo: ^1.0.0 }\n"],
  ["entries under an empty flow mapping", "overrides: {}\n  foo: ^1.0.0\n"],
  [
    "a repeated section",
    "overrides:\n  foo: ^1.0.0\noverrides:\n  bar: 1.0.0\n",
  ],
  ["a nested mapping on one line", "catalog:\n  react: version: ^19.0.0\n"],
  ["a repeated key", "catalog:\n  react: ^19.0.0\n  react: 19.2.0\n"],
]) {
  test(`${name} in pnpm-workspace.yaml fails closed, not silently skipped`, () => {
    const { status, stderr } = run(workspaceWith(yaml));
    assert.equal(status, 1);
    assert.match(stderr, /check-pins: pnpm-workspace\.yaml:\d+: cannot parse/);
  });
}

test("a quoted top-level key is still read as that section", () => {
  const { status, stderr } = run(
    workspaceWith('"overrides":\n  foo: ^1.0.0\n'),
  );
  assert.equal(status, 1);
  assert.match(
    stderr,
    /pnpm-workspace\.yaml overrides\.foo = "\^1\.0\.0" is not exact/,
  );
});

test("a catalog: reference to an existing exact catalog entry passes", () => {
  const { status, stderr } = run({
    ...workspaceWith(
      "catalog:\n  dep: 1.2.3\ncatalogs:\n  legacy:\n    dep: 0.9.0\n",
    ),
    "packages/sdk/package.json": {
      name: "@x/sdk",
      dependencies: { dep: "catalog:" },
      devDependencies: { dep: "catalog:legacy" },
      peerDependencies: { dep: "catalog:default" },
    },
  });
  assert.deepEqual({ status, stderr }, { status: 0, stderr: "" });
});

test("catalog:default may also come from catalogs.default", () => {
  const { status } = run({
    ...workspaceWith("catalogs:\n  default:\n    dep: 1.2.3\n"),
    "packages/sdk/package.json": withDep("catalog:"),
  });
  assert.equal(status, 0);
});

for (const spec of ["catalog:", "catalog:legacy"]) {
  test(`a "${spec}" reference to no catalog entry fails`, () => {
    const { status, stderr } = run({
      ...workspaceWith("catalogs:\n  legacy:\n    other: 1.2.3\n"),
      "packages/sdk/package.json": withDep(spec),
    });
    assert.equal(status, 1);
    assert.match(
      stderr,
      new RegExp(`dependencies\\.dep = "${spec}" names no catalog entry`),
    );
  });
}

test("a catalog: reference without pnpm-workspace.yaml fails", () => {
  const { status, stderr } = run({
    "pnpm-workspace.yaml": null,
    "packages/sdk/package.json": withDep("catalog:"),
  });
  assert.equal(status, 1);
  assert.match(stderr, /dependencies\.dep = "catalog:" names no catalog entry/);
});

// Cargo: every registry dependency in every Cargo.toml of the workspace is
// pinned to one exact version, `=x.y.z`. A loose requirement such as "1"
// lets `cargo update` move to any compatible release.
const CARGO_ROOT = [
  "[workspace]",
  'members = ["crates/*"]',
  'resolver = "3"',
  "",
  "[workspace.package]",
  'version = "0.1.0"',
  "",
].join("\n");
const crate = (body) => ({
  "Cargo.toml": CARGO_ROOT,
  "crates/a/Cargo.toml": `[package]\nname = "a"\nversion.workspace = true\n\n${body}`,
});

test("exact Cargo requirements in every dependency table pass", () => {
  const { status, stderr } = run({
    "Cargo.toml": `${CARGO_ROOT}\n[workspace.dependencies]\nserde = { version = "=1.0.228", features = ["derive"] }\n`,
    "crates/a/Cargo.toml": [
      "[package]",
      'name = "a"',
      "version.workspace = true",
      "",
      "[dependencies]",
      'yrs = "=0.25.0"',
      "serde.workspace = true",
      'renamed = { package = "rand", version = "=0.9.2", optional = true }',
      "",
      "[dev-dependencies]",
      'proptest = "=1.12.0"',
      "",
      "[build-dependencies]",
      'cc = { version = "=1.2.41" }',
      "",
      "[target.'cfg(unix)'.dependencies]",
      'libc = "=0.2.177"',
      "",
      "[target.'cfg(target_arch = \"wasm32\")'.dev-dependencies]",
      'wasm-bindgen-test = "=0.3.54"',
      "",
    ].join("\n"),
  });
  assert.deepEqual({ status, stderr }, { status: 0, stderr: "" });
});

for (const spec of [
  "1",
  "1.12",
  "1.12.0",
  "^1.12.0",
  "~1.12.0",
  ">=1.12.0",
  ">=1.12.0, <2",
  "*",
  "=1.12",
  "=1",
  "",
]) {
  test(`the loose Cargo requirement "${spec}" fails and names file and dependency`, () => {
    const { status, stderr } = run(
      crate(`[dependencies]\nproptest = "${spec}"\n`),
    );
    assert.equal(status, 1);
    assert.equal(
      stderr,
      `check-pins: crates/a/Cargo.toml dependencies.proptest = "${spec}" is not exact (use "=x.y.z")\n`,
    );
  });
}

for (const [table, label] of [
  ["dependencies", "dependencies"],
  ["dev-dependencies", "dev-dependencies"],
  ["build-dependencies", "build-dependencies"],
  ["dev_dependencies", "dev_dependencies"],
  ["build_dependencies", "build_dependencies"],
  ["target.'cfg(unix)'.dependencies", "target.cfg(unix).dependencies"],
  [
    "target.x86_64-pc-windows-msvc.dev-dependencies",
    "target.x86_64-pc-windows-msvc.dev-dependencies",
  ],
]) {
  test(`a loose requirement in [${table}] fails`, () => {
    const { status, stderr } = run(crate(`[${table}]\nfoo = "1"\n`));
    assert.equal(status, 1);
    assert.match(
      stderr,
      new RegExp(
        `crates/a/Cargo\\.toml ${label.replace(/[.()]/g, "\\$&")}\\.foo = "1" is not exact`,
      ),
    );
  });
}

test("a loose version in an inline table fails", () => {
  const { status, stderr } = run(
    crate(
      '[dev-dependencies]\nproptest = { version = "1", default-features = false }\n',
    ),
  );
  assert.equal(status, 1);
  assert.match(
    stderr,
    /crates\/a\/Cargo\.toml dev-dependencies\.proptest = "1" is not exact/,
  );
});

test("a loose version in a dependency sub-table fails", () => {
  const { status, stderr } = run(
    crate('[dependencies.proptest]\nversion = "1.12"\n'),
  );
  assert.equal(status, 1);
  assert.match(
    stderr,
    /crates\/a\/Cargo\.toml dependencies\.proptest = "1\.12" is not exact/,
  );
});

test("a loose requirement in [workspace.dependencies] fails", () => {
  const { status, stderr } = run({
    "Cargo.toml": `${CARGO_ROOT}\n[workspace.dependencies]\nproptest = "1"\n`,
  });
  assert.equal(status, 1);
  assert.match(
    stderr,
    /^check-pins: Cargo\.toml workspace\.dependencies\.proptest = "1" is not exact/m,
  );
});

test("a registry dependency without a version fails", () => {
  const { status, stderr } = run(
    crate('[dependencies]\nfoo = { features = ["x"] }\n'),
  );
  assert.equal(status, 1);
  assert.match(
    stderr,
    /crates\/a\/Cargo\.toml dependencies\.foo has no version/,
  );
});

test("a Cargo.toml outside crates/ is checked too", () => {
  const { status, stderr } = run({
    "apps/desktop/src-tauri/Cargo.toml":
      '[package]\nname = "d"\nversion = "0.1.0"\n\n[dependencies]\ntauri = "2"\n',
  });
  assert.equal(status, 1);
  assert.match(
    stderr,
    /apps\/desktop\/src-tauri\/Cargo\.toml dependencies\.tauri = "2" is not exact/,
  );
});

test("Cargo.toml files under target/ and node_modules/ are not the workspace's", () => {
  const loose =
    '[package]\nname = "x"\nversion = "0.1.0"\n\n[dependencies]\nfoo = "1"\n';
  const { status } = run({
    "target/package/x-0.1.0/Cargo.toml": loose,
    "node_modules/x/Cargo.toml": loose,
  });
  assert.equal(status, 0);
});

test("a Cargo.toml that is not valid TOML fails", () => {
  const { status, stderr } = run(crate("[dependencies\nfoo = 1\n"));
  assert.equal(status, 1);
  assert.match(stderr, /check-pins: crates\/a\/Cargo\.toml is not valid TOML/);
});

test("the repository's own manifests pass", () => {
  const root = join(dirname(fileURLToPath(import.meta.url)), "..");
  const result = spawnSync(process.execPath, [script], {
    cwd: root,
    encoding: "utf8",
  });
  assert.deepEqual(
    { status: result.status, stderr: result.stderr },
    { status: 0, stderr: "" },
  );
});

// Path dependencies between workspace crates keep their own rule, which
// scripts/release/check-version.mjs enforces: declared in
// [workspace.dependencies] with version = "=<workspace version>" (R4) and
// never as path = in a crate (R3). check-pins does not report them twice.
test("a workspace path dependency pinned to the workspace version passes", () => {
  const { status, stderr } = run({
    "Cargo.toml": `${CARGO_ROOT}\n[workspace.dependencies]\na = { path = "crates/a", version = "=0.1.0" }\n`,
    "crates/a/Cargo.toml": '[package]\nname = "a"\nversion.workspace = true\n',
    "crates/b/Cargo.toml":
      '[package]\nname = "b"\nversion.workspace = true\n\n[dependencies]\na.workspace = true\n',
  });
  assert.deepEqual({ status, stderr }, { status: 0, stderr: "" });
});

test("a path dependency is left to check-version, not reported as a registry dependency", () => {
  const { stderr } = run({
    "Cargo.toml": `${CARGO_ROOT}\n[workspace.dependencies]\na = { path = "crates/a" }\n`,
    "crates/a/Cargo.toml": '[package]\nname = "a"\nversion.workspace = true\n',
  });
  assert.doesNotMatch(stderr, /workspace\.dependencies\.a/);
});

// Git dependencies and [patch]/[replace] sections bypass the registry pin.
for (const [name, files, message] of [
  [
    "a git dependency",
    crate(
      '[dependencies]\nfoo = { git = "https://github.com/a/foo", rev = "abc123" }\n',
    ),
    /check-pins: crates\/a\/Cargo\.toml dependencies\.foo is a git dependency; use a registry release pinned to "=x\.y\.z"/,
  ],
  [
    "a git dependency with a version",
    crate(
      '[dev-dependencies]\nfoo = { git = "https://github.com/a/foo", version = "=1.0.0" }\n',
    ),
    /crates\/a\/Cargo\.toml dev-dependencies\.foo is a git dependency/,
  ],
  [
    "a git entry in [workspace.dependencies]",
    {
      "Cargo.toml": `${CARGO_ROOT}\n[workspace.dependencies]\nfoo = { git = "https://github.com/a/foo" }\n`,
    },
    /Cargo\.toml workspace\.dependencies\.foo is a git dependency/,
  ],
  [
    "a [patch.crates-io] section",
    {
      "Cargo.toml": `${CARGO_ROOT}\n[patch.crates-io]\nfoo = { path = "vendor/foo" }\n`,
    },
    /check-pins: Cargo\.toml has a \[patch\] section; it replaces pinned dependencies and is not allowed/,
  ],
  [
    "a [replace] section",
    crate('[replace]\n"foo:1.0.0" = { path = "vendor/foo" }\n'),
    /check-pins: crates\/a\/Cargo\.toml has a \[replace\] section; it replaces pinned dependencies and is not allowed/,
  ],
]) {
  test(`${name} is refused with a clear message`, () => {
    const { status, stderr } = run(files);
    assert.equal(status, 1);
    assert.match(stderr, message);
  });
}
