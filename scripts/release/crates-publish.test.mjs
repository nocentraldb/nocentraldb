import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const script = join(
  dirname(fileURLToPath(import.meta.url)),
  "crates-publish.mjs",
);

const VERSION = "0.2.0-beta.1";

/** A workspace tree in a temporary directory: path -> text. */
const tree = (t, files) => {
  const dir = mkdtempSync(join(tmpdir(), "release-crates-publish-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  for (const [file, text] of Object.entries(files)) {
    mkdirSync(join(dir, dirname(file)), { recursive: true });
    writeFileSync(join(dir, file), text);
  }
  return dir;
};

const crate = (name, body = "") =>
  `[package]\nname = "${name}"\nversion.workspace = true\n${body}`;

// The directories sort cli, core, harness, plugin, sdk; the dependency order
// of the published ones is plugin, core, sdk, cli.
const FIXTURE = {
  "Cargo.toml": `[workspace]
members = ["crates/*"]

[workspace.package]
version = "${VERSION}"

[workspace.dependencies]
plugin = { path = "crates/plugin", version = "=${VERSION}" }
core = { path = "crates/core", version = "=${VERSION}" }
sdk = { path = "crates/sdk", version = "=${VERSION}" }
harness = { path = "crates/harness" }
`,
  "crates/cli/Cargo.toml": crate(
    "cli",
    "\n[dependencies]\nsdk.workspace = true\ncore.workspace = true\n",
  ),
  "crates/core/Cargo.toml": crate(
    "core",
    "\n[dependencies]\nplugin.workspace = true\n\n[dev-dependencies]\nharness.workspace = true\n",
  ),
  "crates/harness/Cargo.toml": crate("harness", "publish = false\n"),
  "crates/plugin/Cargo.toml": crate("plugin"),
  "crates/sdk/Cargo.toml": crate(
    "sdk",
    "\n[dependencies]\ncore.workspace = true\n",
  ),
};
const ORDER = ["plugin", "core", "sdk", "cli"];

// A stand-in for cargo: it logs every call, with its working directory, as a
// JSON line to $FAKE_CARGO_LOG. `package` fails when $FAKE_CARGO_PACKAGE is
// "error"; `publish` fails when $FAKE_CARGO_PUBLISH names the crate after -p.
const FAKE_CARGO = `#!/usr/bin/env node
import { appendFileSync } from "node:fs";
const args = process.argv.slice(2);
appendFileSync(process.env.FAKE_CARGO_LOG, JSON.stringify({ args, cwd: process.cwd() }) + "\\n");
if (args[0] === "package") process.exit(process.env.FAKE_CARGO_PACKAGE === "error" ? 101 : 0);
if (args[0] === "publish") {
  const name = args[args.indexOf("-p") + 1];
  process.exit(process.env.FAKE_CARGO_PUBLISH === name ? 101 : 0);
}
process.exit(3);
`;

/** The path of a crate in the sparse index, as cargo builds it. */
const indexPath = (name) => {
  const n = name.toLowerCase();
  if (n.length <= 2) return `/${n.length}/${n}`;
  if (n.length === 3) return `/3/${n[0]}/${n}`;
  return `/${n.slice(0, 2)}/${n.slice(2, 4)}/${n}`;
};

/**
 * A stand-in for the crates.io sparse index: `held` maps a crate name to the
 * versions it holds; a name missing from it answers 404, and a name in
 * `broken` answers 500. Records every requested path.
 */
const index = async (t, held = {}, broken = []) => {
  const requests = [];
  const paths = new Map(
    Object.entries(held).map(([name, versions]) => [
      indexPath(name),
      versions
        .map((vers) => JSON.stringify({ name, vers, deps: [], yanked: false }))
        .join("\n"),
    ]),
  );
  const fail = new Set(broken.map(indexPath));
  const server = createServer((req, res) => {
    requests.push(req.url);
    if (fail.has(req.url)) {
      res.writeHead(500).end("boom");
    } else if (paths.has(req.url)) {
      res.writeHead(200).end(`${paths.get(req.url)}\n`);
    } else res.writeHead(404).end("not found");
  });
  await new Promise((done) => server.listen(0, "127.0.0.1", done));
  t.after(() => server.close());
  return { url: `http://127.0.0.1:${server.address().port}`, requests };
};

/** Runs the script in `dir` with the stand-in cargo and index; returns the result and cargo's calls. */
const run = async (t, dir, args, { held, broken, env = {} } = {}) => {
  const idx = await index(t, held, broken);
  const bin = mkdtempSync(join(tmpdir(), "release-crates-bin-"));
  t.after(() => rmSync(bin, { recursive: true, force: true }));
  const cargo = join(bin, "cargo.mjs");
  writeFileSync(cargo, FAKE_CARGO);
  chmodSync(cargo, 0o755);
  const log = join(bin, "calls.jsonl");
  const child = spawn(process.execPath, [script, ...args], {
    cwd: dir,
    env: {
      ...process.env,
      CRATES_PUBLISH_CARGO: cargo,
      CRATES_PUBLISH_INDEX: idx.url,
      FAKE_CARGO_LOG: log,
      ...env,
    },
  });
  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (d) => {
    stdout += d;
  });
  child.stderr.on("data", (d) => {
    stderr += d;
  });
  const status = await new Promise((done) => child.on("close", done));
  const calls = existsSync(log)
    ? readFileSync(log, "utf8")
        .trim()
        .split("\n")
        .map((line) => JSON.parse(line))
    : [];
  return { status, stdout, stderr, calls, requests: idx.requests };
};

const PUBLISH = (name) => [
  "publish",
  "--locked",
  "--allow-dirty",
  "--no-verify",
  "-p",
  name,
];
const publishes = (calls) => calls.filter((c) => c.args[0] === "publish");
const packages = (calls) => calls.filter((c) => c.args[0] === "package");

test("publishes every publishable crate in dependency order, one cargo publish each, from the workspace root", async (t) => {
  const dir = tree(t, FIXTURE);
  const r = await run(t, dir, ["--version", VERSION]);
  assert.equal(r.status, 0, r.stderr);
  assert.deepEqual(
    publishes(r.calls).map((c) => c.args),
    ORDER.map(PUBLISH),
  );
  for (const c of r.calls) assert.equal(c.cwd, dir);
  assert.deepEqual(packages(r.calls), []);
  assert.match(r.stdout, /^skip harness: publish = false$/m);
  for (const name of ORDER)
    assert.match(
      r.stdout,
      new RegExp(`^published ${name}@${VERSION.replace(/\./g, "\\.")}$`, "m"),
    );
});

test("a published crate's dev-dependency on another published crate orders that one first", async (t) => {
  const dir = tree(t, {
    "Cargo.toml": `[workspace]\n\n[workspace.package]\nversion = "${VERSION}"\n\n[workspace.dependencies]\nzz = { path = "crates/zz", version = "=${VERSION}" }\n`,
    "crates/aa/Cargo.toml": crate(
      "aa",
      "\n[dev-dependencies]\nzz.workspace = true\n",
    ),
    "crates/zz/Cargo.toml": crate("zz"),
  });
  const r = await run(t, dir, ["--version", VERSION]);
  assert.equal(r.status, 0, r.stderr);
  assert.deepEqual(
    publishes(r.calls).map((c) => c.args.at(-1)),
    ["zz", "aa"],
  );
});

test("asks the sparse index for every crate before publishing, and skips a version crates.io already holds", async (t) => {
  const dir = tree(t, FIXTURE);
  const r = await run(t, dir, ["--version", VERSION], {
    held: { plugin: ["0.0.0", VERSION], core: ["0.0.0"] },
  });
  assert.equal(r.status, 0, r.stderr);
  assert.deepEqual(
    [...r.requests].sort(),
    ["/3/c/cli", "/co/re/core", "/pl/ug/plugin", "/3/s/sdk"].sort(),
  );
  assert.deepEqual(
    publishes(r.calls).map((c) => c.args.at(-1)),
    ["core", "sdk", "cli"],
  );
  assert.match(
    r.stdout,
    new RegExp(`^skip plugin@${VERSION}: already on crates.io$`, "m"),
  );
});

test("a re-run after everything was published calls no cargo and succeeds", async (t) => {
  const dir = tree(t, FIXTURE);
  const held = Object.fromEntries(ORDER.map((name) => [name, [VERSION]]));
  for (const args of [
    ["--version", VERSION],
    ["--version", VERSION, "--verify-only"],
  ]) {
    const r = await run(t, dir, args, { held });
    assert.equal(r.status, 0, r.stderr);
    assert.deepEqual(r.calls, [], args.join(" "));
  }
});

test("--verify-only packages and verifies every missing crate in one cargo package, in dependency order, and publishes nothing", async (t) => {
  const dir = tree(t, FIXTURE);
  const r = await run(t, dir, ["--version", VERSION, "--verify-only"], {
    held: { plugin: [VERSION] },
  });
  assert.equal(r.status, 0, r.stderr);
  assert.deepEqual(
    r.calls.map((c) => c.args),
    [
      [
        "package",
        "--locked",
        "--allow-dirty",
        "-p",
        "core",
        "-p",
        "sdk",
        "-p",
        "cli",
      ],
    ],
  );
  assert.match(
    r.stdout,
    new RegExp(`^would publish core@${VERSION.replace(/\./g, "\\.")}$`, "m"),
  );
  assert.match(
    r.stdout,
    new RegExp(`^skip plugin@${VERSION}: already on crates.io$`, "m"),
  );
});

test("a failed verification fails --verify-only", async (t) => {
  const dir = tree(t, FIXTURE);
  const r = await run(t, dir, ["--version", VERSION, "--verify-only"], {
    env: { FAKE_CARGO_PACKAGE: "error" },
  });
  assert.equal(r.status, 1);
  assert.match(r.stderr, /crates-publish: cargo package failed/);
});

test("a failed publish fails the run, names the crate and publishes nothing after it", async (t) => {
  const dir = tree(t, FIXTURE);
  const r = await run(t, dir, ["--version", VERSION], {
    env: { FAKE_CARGO_PUBLISH: "core" },
  });
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    new RegExp(`crates-publish: cargo publish of core@${VERSION} failed`),
  );
  assert.deepEqual(
    publishes(r.calls).map((c) => c.args.at(-1)),
    ["plugin", "core"],
  );
});

test("an index lookup that fails for another reason than 404 fails the run before anything is published", async (t) => {
  const dir = tree(t, FIXTURE);
  const r = await run(t, dir, ["--version", VERSION], { broken: ["sdk"] });
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    /crates-publish: the crates\.io index answered 500 for sdk/,
  );
  assert.deepEqual(r.calls, []);
});

test("a crate that does not carry the version fails the run before anything is published", async (t) => {
  for (const files of [
    {
      ...FIXTURE,
      "Cargo.toml": FIXTURE["Cargo.toml"].replace(
        `version = "${VERSION}"`,
        'version = "0.2.0"',
      ),
    },
    {
      ...FIXTURE,
      "crates/sdk/Cargo.toml": `[package]\nname = "sdk"\nversion = "0.1.0"\n`,
    },
  ]) {
    const dir = tree(t, files);
    const r = await run(t, dir, ["--version", VERSION]);
    assert.equal(r.status, 1);
    assert.match(
      r.stderr,
      /crates-publish: \w+ \(crates\/\w+\) is at 0\.[12]\.0, not 0\.2\.0-beta\.1: run set-version first/,
    );
    assert.deepEqual(r.calls, []);
  }
});

test("a dependency cycle between published crates fails the run before anything is published", async (t) => {
  const dir = tree(t, {
    "Cargo.toml": `[workspace]\n\n[workspace.package]\nversion = "${VERSION}"\n\n[workspace.dependencies]\naa = { path = "crates/aa", version = "=${VERSION}" }\nbb = { path = "crates/bb", version = "=${VERSION}" }\n`,
    "crates/aa/Cargo.toml": crate(
      "aa",
      "\n[dependencies]\nbb.workspace = true\n",
    ),
    "crates/bb/Cargo.toml": crate(
      "bb",
      "\n[dev-dependencies]\naa.workspace = true\n",
    ),
  });
  const r = await run(t, dir, ["--version", VERSION]);
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    /crates-publish: the published crates depend on each other in a cycle: aa, bb/,
  );
  assert.deepEqual(r.calls, []);
});

test("only a beta or a release: an alpha or another version is a usage error, and cargo is not called", async (t) => {
  const dir = tree(t, FIXTURE);
  for (const args of [
    ["--version", "0.2.0-alpha.3"],
    ["--version", "0.2.0-rc.1"],
    ["--version", "v0.2.0"],
    [],
    ["--version", VERSION, "--dry-run"],
  ]) {
    const r = await run(t, dir, args);
    assert.equal(r.status, 2, `${args.join(" ")}: ${r.stderr}`);
    assert.deepEqual(r.calls, [], args.join(" "));
  }
});

test("a release version publishes as a beta does", async (t) => {
  const dir = tree(t, {
    ...FIXTURE,
    "Cargo.toml": FIXTURE["Cargo.toml"].replaceAll(VERSION, "0.2.0"),
  });
  const r = await run(t, dir, ["--version", "0.2.0"]);
  assert.equal(r.status, 0, r.stderr);
  assert.deepEqual(
    publishes(r.calls).map((c) => c.args.at(-1)),
    ORDER,
  );
});
