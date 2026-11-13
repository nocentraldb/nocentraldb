import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const script = join(dirname(fileURLToPath(import.meta.url)), "npm-publish.mjs");

const VERSION = "0.2.0-alpha.3";

/** A workspace tree in a temporary directory: path -> text. */
const tree = (t, files) => {
  const dir = mkdtempSync(join(tmpdir(), "release-npm-publish-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  for (const [file, text] of Object.entries(files)) {
    mkdirSync(join(dir, dirname(file)), { recursive: true });
    writeFileSync(join(dir, file), text);
  }
  return dir;
};

const REPO = "acme/widgets";
const REPOSITORY = {
  type: "git",
  url: `git+https://github.com/${REPO}.git`,
  directory: "packages/x",
};
const pkg = (fields) =>
  `${JSON.stringify({ version: VERSION, repository: REPOSITORY, ...fields }, null, 2)}\n`;

const FIXTURE = {
  "packages/sdk/package.json": pkg({
    name: "@x/sdk",
    exports: { ".": { default: "./dist/index.js" } },
  }),
  "packages/other/package.json": pkg({
    name: "@x/other",
    main: "./dist/index.js",
  }),
  "apps/example/package.json": pkg({
    name: "@x/example",
    private: true,
  }),
};

// A stand-in for npm: it logs every call, with its working directory, as a
// JSON line to $FAKE_NPM_LOG. `view <name>@<version> version` answers the
// version when $FAKE_NPM_PUBLISHED lists it, an E404 otherwise, or another
// error when $FAKE_NPM_VIEW is "error". `publish` fails when $FAKE_NPM_PUBLISH
// is "error".
const FAKE_NPM = `#!/usr/bin/env node
import { appendFileSync } from "node:fs";
const args = process.argv.slice(2);
appendFileSync(process.env.FAKE_NPM_LOG, JSON.stringify({ args, cwd: process.cwd() }) + "\\n");
if (args[0] === "view") {
  if (process.env.FAKE_NPM_VIEW === "error") {
    console.error("npm error code ETIMEDOUT");
    process.exit(1);
  }
  const published = (process.env.FAKE_NPM_PUBLISHED ?? "").split(",");
  if (published.includes(args[1])) {
    console.log(JSON.stringify(args[1].slice(args[1].lastIndexOf("@") + 1)));
    process.exit(0);
  }
  console.error("npm error code E404\\nnpm error 404 Not Found");
  process.exit(1);
}
if (args[0] === "publish") {
  if (process.env.FAKE_NPM_PUBLISH === "error") {
    console.error("npm error code E403");
    process.exit(1);
  }
  console.log("+ published");
  process.exit(0);
}
process.exit(3);
`;

/** Runs the script in `dir` with the stand-in npm; returns the result and npm's calls. */
const run = (t, dir, args, env = {}) => {
  const bin = mkdtempSync(join(tmpdir(), "release-npm-bin-"));
  t.after(() => rmSync(bin, { recursive: true, force: true }));
  const npm = join(bin, "npm.mjs");
  writeFileSync(npm, FAKE_NPM);
  chmodSync(npm, 0o755);
  const log = join(bin, "calls.jsonl");
  const r = spawnSync(process.execPath, [script, ...args], {
    cwd: dir,
    encoding: "utf8",
    env: { ...process.env, NPM_PUBLISH_NPM: npm, FAKE_NPM_LOG: log, ...env },
  });
  const calls = existsSync(log)
    ? readFileSync(log, "utf8")
        .trim()
        .split("\n")
        .map((line) => JSON.parse(line))
    : [];
  return { ...r, calls };
};

const ALPHA = ["--version", VERSION, "--dist-tag", "alpha", "--repo", REPO];
const publishes = (calls) => calls.filter((c) => c.args[0] === "publish");

test("publishes each publishable package under the dist-tag alpha, with provenance, from its directory", (t) => {
  const dir = tree(t, FIXTURE);
  const r = run(t, dir, ALPHA);
  assert.equal(r.status, 0, r.stderr);
  assert.deepEqual(
    publishes(r.calls).map((c) => ({ args: c.args, cwd: c.cwd })),
    [
      {
        args: [
          "publish",
          "--tag",
          "alpha",
          "--provenance",
          "--access",
          "public",
        ],
        cwd: join(dir, "packages/other"),
      },
      {
        args: [
          "publish",
          "--tag",
          "alpha",
          "--provenance",
          "--access",
          "public",
        ],
        cwd: join(dir, "packages/sdk"),
      },
    ],
  );
  assert.match(r.stdout, /^skip @x\/example: private: true$/m);
  assert.match(
    r.stdout,
    new RegExp(`^published @x/sdk@${VERSION} under alpha$`, "m"),
  );
  assert.match(
    r.stdout,
    new RegExp(`^published @x/other@${VERSION} under alpha$`, "m"),
  );
});

test("asks the registry for each version before publishing it, and skips one already there", (t) => {
  const dir = tree(t, FIXTURE);
  const r = run(t, dir, ALPHA, { FAKE_NPM_PUBLISHED: `@x/sdk@${VERSION}` });
  assert.equal(r.status, 0, r.stderr);
  const views = r.calls.filter((c) => c.args[0] === "view");
  assert.deepEqual(
    views.map((c) => c.args),
    [
      ["view", `@x/other@${VERSION}`, "version", "--json"],
      ["view", `@x/sdk@${VERSION}`, "version", "--json"],
    ],
  );
  assert.deepEqual(
    publishes(r.calls).map((c) => c.cwd),
    [join(dir, "packages/other")],
  );
  assert.match(
    r.stdout,
    new RegExp(`^skip @x/sdk@${VERSION}: already on npm$`, "m"),
  );
});

test("a re-run after everything was published publishes nothing and succeeds", (t) => {
  const dir = tree(t, FIXTURE);
  const all = `@x/sdk@${VERSION},@x/other@${VERSION}`;
  const r = run(t, dir, ALPHA, { FAKE_NPM_PUBLISHED: all });
  assert.equal(r.status, 0, r.stderr);
  assert.deepEqual(publishes(r.calls), []);
});

test("--dry-run reports what it would publish and publishes nothing", (t) => {
  const dir = tree(t, FIXTURE);
  const r = run(t, dir, [...ALPHA, "--dry-run"], {
    FAKE_NPM_PUBLISHED: `@x/other@${VERSION}`,
  });
  assert.equal(r.status, 0, r.stderr);
  assert.deepEqual(publishes(r.calls), []);
  assert.match(
    r.stdout,
    new RegExp(`^would publish @x/sdk@${VERSION} under alpha$`, "m"),
  );
  assert.match(
    r.stdout,
    new RegExp(`^skip @x/other@${VERSION}: already on npm$`, "m"),
  );
  assert.match(r.stdout, /^skip @x\/example: private: true$/m);
});

test("only an alpha: another version or dist-tag is a usage error, and npm is not called", (t) => {
  const dir = tree(t, FIXTURE);
  for (const args of [
    ["--version", "0.2.0-beta.1", "--dist-tag", "alpha", "--repo", REPO],
    ["--version", "0.2.0", "--dist-tag", "alpha", "--repo", REPO],
    ["--version", VERSION, "--dist-tag", "latest", "--repo", REPO],
    ["--version", VERSION, "--dist-tag", "beta", "--repo", REPO],
    ["--version", VERSION, "--dist-tag", "alpha"],
    ["--dist-tag", "alpha", "--repo", REPO],
    ["--version", VERSION, "--repo", REPO],
  ]) {
    const r = run(t, dir, args);
    assert.equal(r.status, 2, `${args.join(" ")}: ${r.stderr}`);
    assert.deepEqual(r.calls, [], args.join(" "));
  }
});

test("a package that does not carry the version fails the run before anything is published", (t) => {
  const dir = tree(t, {
    ...FIXTURE,
    "packages/sdk/package.json": pkg({
      name: "@x/sdk",
      version: "0.1.0",
      main: "./dist/index.js",
    }),
  });
  const r = run(t, dir, ALPHA);
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    new RegExp(
      `npm-publish: @x/sdk \\(packages/sdk\\) is at 0\\.1\\.0, not ${VERSION.replace(/\./g, "\\.")}`,
    ),
  );
  assert.deepEqual(publishes(r.calls), []);
});

test("a registry lookup that fails for another reason than 404 fails the run and publishes nothing", (t) => {
  const dir = tree(t, FIXTURE);
  const r = run(t, dir, ALPHA, { FAKE_NPM_VIEW: "error" });
  assert.equal(r.status, 1);
  assert.match(r.stderr, /npm-publish: npm view @x\/other@[^ ]+ failed/);
  assert.deepEqual(publishes(r.calls), []);
});

test("a failed publish fails the run and names the package", (t) => {
  const dir = tree(t, FIXTURE);
  const r = run(t, dir, ALPHA, { FAKE_NPM_PUBLISH: "error" });
  assert.equal(r.status, 1);
  assert.match(
    r.stderr,
    new RegExp(`npm-publish: npm publish of @x/other@${VERSION} failed`),
  );
});

test("a package whose repository is not this repository fails the run, since npm refuses its provenance", (t) => {
  for (const repository of [
    undefined,
    { type: "git", url: "git+https://github.com/someone/else.git" },
    "acme/widgets",
  ]) {
    const dir = tree(t, {
      ...FIXTURE,
      "packages/sdk/package.json": pkg({
        name: "@x/sdk",
        main: "./dist/index.js",
        repository,
      }),
    });
    const r = run(t, dir, ALPHA);
    assert.equal(r.status, 1, JSON.stringify(repository));
    assert.match(
      r.stderr,
      /npm-publish: @x\/sdk \(packages\/sdk\): its repository\.url must name https:\/\/github\.com\/acme\/widgets/,
    );
    assert.deepEqual(publishes(r.calls), []);
  }
});

test("a repository.url with or without git+ and .git names the repository", (t) => {
  for (const url of [
    "https://github.com/acme/widgets",
    "git+https://github.com/acme/widgets.git",
    "https://github.com/acme/widgets.git",
  ]) {
    const dir = tree(t, {
      "packages/sdk/package.json": pkg({
        name: "@x/sdk",
        main: "./dist/index.js",
        repository: { type: "git", url },
      }),
    });
    const r = run(t, dir, ALPHA);
    assert.equal(r.status, 0, `${url}: ${r.stderr}`);
  }
});
