// Asserts how every workflow job installs the toolchain (.prototools: proto,
// then moon, Node and pnpm): it restores ~/.proto from the Actions cache
// under one exact key, derived from .prototools and the install script (which
// pins proto's own version), with no fallback key; it then runs
// scripts/install-toolchain.sh, which downloads nothing when the cache is
// warm, retries a cold download with backoff, and fails naming the host it
// could not reach. No job uses moonrepo/setup-toolchain, which downloads
// proto on every run, before it even looks at its cache.
//
// The workflows are checked by structure, as in the other workflow contract
// tests; the script is checked by running it against fake `curl`, `sleep`
// and `proto` executables. Every check also runs against mutants (a workflow
// or the script with one property removed or changed) and must reject each.
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
import { fileURLToPath } from "node:url";
import { parseDocument } from "yaml";
import { runWorkflowChecks } from "./lib/workflow-checks.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (file) => readFileSync(join(root, file), "utf8");
const SCRIPT = "scripts/install-toolchain.sh";

/** The jobs that run pnpm, per workflow: each installs the toolchain. */
const JOBS = {
  ci: ["gates"],
  changes: ["changes"],
  release: ["alpha", "verify", "build", "publish", "commit-back"],
};

const load = () => {
  const docs = {};
  for (const name of Object.keys(JOBS)) {
    docs[name] = parseDocument(read(`.github/workflows/${name}.yml`));
    assert.deepEqual(docs[name].errors, [], `${name}.yml does not parse`);
  }
  // Empty when the script is missing, so the checks fail as assertions.
  return { docs, script: existsSync(join(root, SCRIPT)) ? read(SCRIPT) : "" };
};

const clone = (ctx) => ({
  docs: Object.fromEntries(
    Object.entries(ctx.docs).map(([name, doc]) => [name, doc.clone()]),
  ),
  script: ctx.script,
});

/** A GitHub Actions expression, `${{ … }}`, as the workflow spells it. */
const gh = (expression) => `\${{ ${expression} }}`;

const CACHE_NAME = "Cache the proto toolchain";
const CACHE_PATHS = [
  "~/.proto/bin",
  "~/.proto/shims",
  "~/.proto/tools",
  "~/.proto/plugins",
];
const CACHE_KEY = `proto-${gh("runner.os")}-${gh(`hashFiles('.prototools', '${SCRIPT}')`)}`;
const INSTALL = {
  name: "Install proto and the tools in .prototools",
  run: `bash ${SCRIPT}`,
};

const jobs = (ctx, name) => ctx.docs[name].toJS().jobs;
const steps = (ctx, name, job) => jobs(ctx, name)[job].steps;
const stepPath = (job) => ["jobs", job, "steps"];

/** Every [workflow, job] pair the contract covers. */
const covered = () =>
  Object.entries(JOBS).flatMap(([name, ids]) => ids.map((id) => [name, id]));

/** The index of a job's checkout step. */
const checkoutAt = (ctx, name, job) => {
  const index = steps(ctx, name, job).findIndex((step) =>
    step.uses?.startsWith("actions/checkout@"),
  );
  assert.notEqual(index, -1, `${name}/${job} has no checkout step`);
  return index;
};

/** The step that restores ~/.proto, in place of a step the mutant edits. */
const cacheAt = (ctx, name, job) => checkoutAt(ctx, name, job) + 1;

/** Sets `value` at `path` below step `index` of a job, for the mutants. */
const setStep = (ctx, name, job, index, path, value) =>
  ctx.docs[name].setIn([...stepPath(job), index, ...path], value);

/** Replaces `from` with `to` in the script; the mutant must apply. */
const editScript = (ctx, from, to) => {
  assert.ok(ctx.script.includes(from), `the script has no ${from}`);
  ctx.script = ctx.script.replace(from, to);
};

/** proto's own version, as the script pins it. */
const protoVersion = (script) => {
  const match = /^PROTO_VERSION="(\d+\.\d+\.\d+)"$/m.exec(script);
  assert.ok(match, "the script pins no exact proto version");
  return match[1];
};

/**
 * Runs `script` in a scratch workspace whose PATH holds fake `curl`,
 * `sleep` and (when `warm`) an installed fake `proto`. `curlFails` and
 * `installFails` count the failing calls before the first that succeeds.
 * Returns the exit status, the output, and what each fake recorded.
 */
const runScript = (script, { warm, curlFails = 0, installFails = 0 }) => {
  const version = protoVersion(script);
  const dir = mkdtempSync(join(tmpdir(), "install-toolchain-"));
  try {
    const bin = join(dir, "fake-bin");
    const log = join(dir, "log");
    const home = join(dir, "home");
    for (const d of [bin, log, home, join(dir, "repo", "scripts")]) {
      mkdirSync(d, { recursive: true });
    }
    const exe = (path, body) => {
      writeFileSync(path, `#!/usr/bin/env bash\n${body}`);
      chmodSync(path, 0o755);
    };
    const count = (name) =>
      `echo "$*" >> "$FAKE_LOG/${name}"; n=$(wc -l < "$FAKE_LOG/${name}")\n`;
    // The fake proto: reports the pinned version; `install` fails the
    // first installFails times like a download from nodejs.org that 504s.
    const proto = `case "$1" in
--version) echo "proto ${version}" ;;
install)
  ${count("install")}
  if [ "$n" -le ${installFails} ]; then
    echo "Failed to download file from https://nodejs.org/dist/v24.22.2/node-v24.22.2-linux-x64.tar.xz (504 Gateway Timeout)."
    exit 1
  fi ;;
esac
`;
    const archive = join(dir, "archive");
    mkdirSync(join(archive, "proto_cli"), { recursive: true });
    exe(join(archive, "proto_cli", "proto"), proto);
    exe(join(archive, "proto_cli", "proto-shim"), "exit 0\n");
    const tarball = join(dir, "proto.tar.xz");
    const tar = spawnSync("tar", ["-cJf", tarball, "-C", archive, "proto_cli"]);
    assert.equal(tar.status, 0, String(tar.stderr));
    exe(
      join(bin, "curl"),
      `${count("curl")}if [ "$n" -le ${curlFails} ]; then
  echo "curl: (22) The requested URL returned error: 504" >&2
  exit 22
fi
while [ $# -gt 0 ]; do
  case "$1" in --output|-o) out="$2"; shift ;; esac
  shift
done
cp "${tarball}" "$out"
`,
    );
    exe(join(bin, "sleep"), `${count("sleep")}`);
    if (warm) {
      mkdirSync(join(home, ".proto", "bin"), { recursive: true });
      exe(join(home, ".proto", "bin", "proto"), proto);
    }
    writeFileSync(join(dir, "repo", SCRIPT), script);
    const githubPath = join(dir, "github-path");
    const githubEnv = join(dir, "github-env");
    writeFileSync(githubPath, "");
    writeFileSync(githubEnv, "");
    const result = spawnSync("bash", [join(dir, "repo", SCRIPT)], {
      cwd: join(dir, "repo"),
      encoding: "utf8",
      env: {
        PATH: `${bin}:/usr/bin:/bin`,
        HOME: home,
        FAKE_LOG: log,
        GITHUB_PATH: githubPath,
        GITHUB_ENV: githubEnv,
      },
    });
    const lines = (name) =>
      existsSync(join(log, name))
        ? readFileSync(join(log, name), "utf8").split("\n").filter(Boolean)
        : [];
    return {
      status: result.status,
      output: `${result.stdout}${result.stderr}`,
      curl: lines("curl"),
      sleep: lines("sleep"),
      install: lines("install"),
      githubPath: readFileSync(githubPath, "utf8"),
      githubEnv: readFileSync(githubEnv, "utf8"),
      installed: existsSync(join(home, ".proto", "bin", "proto")),
    };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
};

const checks = [
  {
    name: "no workflow uses moonrepo/setup-toolchain",
    check: (ctx) => {
      for (const [name, doc] of Object.entries(ctx.docs)) {
        for (const [id, job] of Object.entries(doc.toJS().jobs)) {
          for (const step of job.steps ?? []) {
            assert.doesNotMatch(
              String(step.uses ?? ""),
              /^moonrepo\/setup-toolchain@/,
              `${name}/${id}`,
            );
          }
        }
      }
    },
    mutants: {
      "the release build job goes back to setup-toolchain": (ctx) =>
        setStep(
          ctx,
          "release",
          "build",
          cacheAt(ctx, "release", "build"),
          ["uses"],
          "moonrepo/setup-toolchain@261c62cb5b0f580c7be7c8cd0f023a2e96756095",
        ),
    },
  },
  {
    name: "every job that runs pnpm is one the contract covers",
    check: (ctx) => {
      for (const [name, ids] of Object.entries(JOBS)) {
        const usesPnpm = Object.entries(jobs(ctx, name))
          .filter(([, job]) =>
            (job.steps ?? []).some((step) => /\bpnpm\b/.test(step.run ?? "")),
          )
          .map(([id]) => id);
        assert.deepEqual(usesPnpm.sort(), [...ids].sort(), name);
      }
    },
    mutants: {
      "ci adds a job that runs pnpm": (ctx) =>
        ctx.docs.ci.setIn(["jobs", "lint"], {
          "runs-on": "ubuntu-latest",
          steps: [{ run: "pnpm install --frozen-lockfile" }],
        }),
    },
  },
  {
    // A warm cache needs no request to proto's download host. The key is
    // exact and there are no restore-keys: a fallback would restore a
    // toolchain that .prototools no longer pins, the same rule as the Cargo
    // cache in ci.yml.
    name: "every job restores ~/.proto right after its checkout, under one exact key, with no fallback",
    check: (ctx) => {
      for (const [name, job] of covered()) {
        const all = steps(ctx, name, job);
        const checkout = all[checkoutAt(ctx, name, job)];
        const cache = all[cacheAt(ctx, name, job)];
        const where = `${name}/${job}`;
        assert.equal(cache?.name, CACHE_NAME, where);
        assert.match(String(cache.uses), /^actions\/cache@/, where);
        assert.deepEqual(
          String(cache.with?.path ?? "")
            .split("\n")
            .map((path) => path.trim())
            .filter(Boolean),
          CACHE_PATHS,
          where,
        );
        assert.equal(cache.with.key, CACHE_KEY, where);
        assert.deepEqual(
          Object.keys(cache.with).sort(),
          ["key", "path"],
          where,
        );
        assert.equal(cache.if, checkout.if, `${where}: its condition`);
      }
    },
    mutants: {
      "ci adds a restore-keys fallback": (ctx) =>
        setStep(
          ctx,
          "ci",
          "gates",
          cacheAt(ctx, "ci", "gates"),
          ["with", "restore-keys"],
          "proto-",
        ),
      "changes keys on .prototools without the script": (ctx) =>
        setStep(
          ctx,
          "changes",
          "changes",
          cacheAt(ctx, "changes", "changes"),
          ["with", "key"],
          `proto-${gh("runner.os")}-${gh("hashFiles('.prototools')")}`,
        ),
      "the publish job drops its cache step": (ctx) =>
        ctx.docs.release.deleteIn([
          ...stepPath("publish"),
          cacheAt(ctx, "release", "publish"),
        ]),
      "verify caches all of ~/.proto": (ctx) =>
        setStep(
          ctx,
          "release",
          "verify",
          cacheAt(ctx, "release", "verify"),
          ["with", "path"],
          "~/.proto",
        ),
      "alpha restores the cache when its guard skips the job": (ctx) =>
        ctx.docs.release.deleteIn([
          ...stepPath("alpha"),
          cacheAt(ctx, "release", "alpha"),
          "if",
        ]),
    },
  },
  {
    name: "every job installs the toolchain with the script, right after the cache",
    check: (ctx) => {
      for (const [name, job] of covered()) {
        const all = steps(ctx, name, job);
        const checkout = all[checkoutAt(ctx, name, job)];
        const { if: condition, ...install } =
          all[cacheAt(ctx, name, job) + 1] ?? {};
        assert.deepEqual(install, INSTALL, `${name}/${job}`);
        assert.equal(condition, checkout.if, `${name}/${job}: its condition`);
      }
    },
    mutants: {
      "ci runs proto install without the retry": (ctx) =>
        setStep(
          ctx,
          "ci",
          "gates",
          cacheAt(ctx, "ci", "gates") + 1,
          ["run"],
          "proto install",
        ),
      "commit-back drops the install step": (ctx) =>
        ctx.docs.release.deleteIn([
          ...stepPath("commit-back"),
          cacheAt(ctx, "release", "commit-back") + 1,
        ]),
    },
  },
  {
    name: "the script pins proto to one exact version",
    check: (ctx) => {
      protoVersion(ctx.script);
    },
    mutants: {
      "installs the latest proto": (ctx) =>
        editScript(
          ctx,
          `PROTO_VERSION="${protoVersion(ctx.script)}"`,
          'PROTO_VERSION="latest"',
        ),
    },
  },
  {
    name: "a warm cache downloads nothing and keeps the cached plugins valid",
    check: (ctx) => {
      const run = runScript(ctx.script, { warm: true });
      assert.equal(run.status, 0, run.output);
      assert.deepEqual(run.curl, [], "a warm cache downloaded proto");
      assert.equal(run.install.length, 1, "proto install did not run once");
      // proto deletes and re-fetches a plugin older than its cache
      // duration, and the restored files keep their old mtimes.
      assert.match(run.githubEnv, /^PROTO_CACHE_DURATION=\d{9,}$/m);
      assert.match(run.githubPath, /\/\.proto\/bin$/m);
      assert.match(run.githubPath, /\/\.proto\/shims$/m);
    },
    mutants: {
      "downloads proto even when it is installed": (ctx) =>
        editScript(ctx, '"$(installed_version)" != "$PROTO_VERSION"', "true"),
      "keeps proto's default plugin cache duration": (ctx) =>
        editScript(ctx, 'echo "PROTO_CACHE_DURATION=', 'echo "UNUSED='),
    },
  },
  {
    name: "a cold install downloads the pinned proto from GitHub, retrying with backoff",
    check: (ctx) => {
      const version = protoVersion(ctx.script);
      const run = runScript(ctx.script, { warm: false, curlFails: 2 });
      assert.equal(run.status, 0, run.output);
      assert.equal(run.curl.length, 3, "not three download attempts");
      for (const call of run.curl) {
        assert.match(call, /--fail\b/);
        assert.ok(
          call.includes(
            `https://github.com/moonrepo/proto/releases/download/v${version}/proto_cli-`,
          ),
          call,
        );
      }
      assert.deepEqual(run.sleep, ["10", "20"], "no growing backoff");
      assert.ok(run.installed, "proto is not in ~/.proto/bin");
      assert.equal(run.install.length, 1);
    },
    mutants: {
      "tries once": (ctx) => editScript(ctx, "ATTEMPTS=3", "ATTEMPTS=1"),
      "waits a constant time": (ctx) =>
        editScript(ctx, "$((10 * attempt))", "10"),
    },
  },
  {
    name: "a download that keeps failing fails the job naming the host",
    check: (ctx) => {
      const run = runScript(ctx.script, { warm: false, curlFails: 99 });
      assert.notEqual(run.status, 0);
      assert.equal(run.curl.length, 3);
      assert.match(
        run.output,
        /^::error title=[^:]+::.*\bgithub\.com\b.*after 3 attempts/m,
      );
      assert.equal(run.install.length, 0, "proto install ran without proto");
    },
    mutants: {
      "reports the bare curl error": (ctx) =>
        editScript(ctx, "from $(host_of", "from $(true"),
    },
  },
  {
    name: "a proto install that keeps failing is retried and names the host",
    check: (ctx) => {
      const retried = runScript(ctx.script, { warm: true, installFails: 2 });
      assert.equal(retried.status, 0, retried.output);
      assert.equal(retried.install.length, 3);
      assert.deepEqual(retried.sleep, ["10", "20"]);

      const failed = runScript(ctx.script, { warm: true, installFails: 99 });
      assert.notEqual(failed.status, 0);
      assert.equal(failed.install.length, 3);
      assert.match(
        failed.output,
        /^::error title=[^:]+::.*\bnodejs\.org\b.*after 3 attempts/m,
      );
    },
    mutants: {
      "runs proto install once": (ctx) =>
        editScript(ctx, 'retry "proto install"', "run_once"),
      "names no host": (ctx) => editScript(ctx, "hosts_in", "true"),
    },
  },
];

runWorkflowChecks({ files: load(), clone, checks });
