// Asserts the invariants of .github/workflows/ci.yml that the design review
// made binding: the triggers and permissions, the check name `gates`, the
// full uncached gate and its exact command, the supply-chain rules, the cache
// scope and bounds, the job timeout and the concurrency policy, and the
// Dependabot configuration that keeps the action pins current.
//
// Every file is parsed (YAML with the `yaml` package, JSON with JSON.parse)
// and checked by structure, never by matching lines of text, and every check
// asserts the declared value rather than a range. Each check is also run
// against mutants of the real files, each with one property removed or
// changed, and must reject every one of them: a check that cannot fail is
// caught here, on every run.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parse, parseDocument } from "yaml";
import { runWorkflowChecks } from "./lib/workflow-checks.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (file) => readFileSync(join(root, file), "utf8");
const DEPENDABOT = ".github/dependabot.yml";

/**
 * The parsed inputs every check reads. `doc` keeps the workflow's comments
 * (the action version comments, the checkout depth decision); the plain
 * object view of it is `workflow(ctx)`.
 */
const load = () => {
  const doc = parseDocument(read(".github/workflows/ci.yml"));
  assert.deepEqual(doc.errors, [], "ci.yml does not parse");
  return {
    doc,
    pkg: JSON.parse(read("package.json")),
    workspace: parse(read("pnpm-workspace.yaml")),
    // null when the file is missing, so the check fails as an assertion.
    dependabot: existsSync(join(root, DEPENDABOT))
      ? parse(read(DEPENDABOT))
      : null,
  };
};

const clone = (ctx) => ({
  doc: ctx.doc.clone(),
  pkg: structuredClone(ctx.pkg),
  workspace: structuredClone(ctx.workspace),
  dependabot: structuredClone(ctx.dependabot),
});

const workflow = (ctx) => ctx.doc.toJS();
const steps = (ctx) => workflow(ctx).jobs.gates.steps;
const STEPS = ["jobs", "gates", "steps"];

/** The step that uses `action` (owner/name), with its index. */
const findStep = (ctx, action) => {
  const index = steps(ctx).findIndex((step) =>
    step.uses?.startsWith(`${action}@`),
  );
  assert.notEqual(index, -1, `no step uses ${action}`);
  return { index, step: steps(ctx)[index] };
};

/** The index of the first step that satisfies `pred`, for the mutants. */
const at = (ctx, pred) => {
  const index = steps(ctx).findIndex(pred);
  assert.notEqual(index, -1, "no step matches");
  return index;
};
const cargoStep = (ctx) =>
  at(ctx, (step) => String(step.with?.path).includes("target"));
const pnpmStep = (ctx) =>
  at(ctx, (step) => String(step.with?.key).startsWith("pnpm-"));
const checkoutStep = (ctx) =>
  at(ctx, (step) => step.uses?.startsWith("actions/checkout@"));
const runStep = (ctx, run) => at(ctx, (step) => step.run === run);

/** The comment above step `index`; the first one belongs to the sequence. */
const commentAbove = (ctx, index) => {
  const seq = ctx.doc.getIn(STEPS, true);
  return (
    (index === 0 ? seq.commentBefore : seq.items[index].commentBefore) ?? ""
  );
};

/** Every string scalar in the parsed workflow, keys included. */
const strings = (value) =>
  typeof value === "string"
    ? [value]
    : value && typeof value === "object"
      ? Object.entries(value).flatMap(([k, v]) => [k, ...strings(v)])
      : [];

/** The cache steps, each with its `path` split into one entry per line. */
const caches = (ctx) =>
  steps(ctx)
    .filter((step) => step.uses?.startsWith("actions/cache@"))
    .map((step) => ({
      ...step,
      paths: String(step.with?.path ?? "")
        .split("\n")
        .map((path) => path.trim())
        .filter(Boolean),
    }));

/** A GitHub Actions expression, `${{ … }}`, as the workflow spells it. */
const gh = (expression) => `\${{ ${expression} }}`;

const GATES_ARGV = [
  "moon",
  "exec",
  ":gates",
  "--force",
  "--base",
  "HEAD",
  "--head",
  "HEAD",
  "--on-failure",
  "continue",
  "--summary",
  "normal",
];
const TIMEOUT_MINUTES = 30;
const CONCURRENCY = {
  group: `ci-${gh("github.event_name == 'pull_request' && github.ref || github.sha")}`,
  "cancel-in-progress": gh("github.event_name == 'pull_request'"),
};
const CARGO_KEY = `cargo-${gh("runner.os")}-${gh("hashFiles('rust-toolchain.toml', 'Cargo.lock')")}`;
// Dependabot proposes updates for the SHA-pinned actions only, resolving each
// pin together with its version comment. npm and cargo are left out on
// purpose (see the README): adding one changes this declared list.
const DEPENDABOT_UPDATES = [
  {
    "package-ecosystem": "github-actions",
    directory: "/",
    schedule: { interval: "weekly" },
  },
];
const PNPM_KEY = `pnpm-${gh("runner.os")}-${gh("hashFiles('.prototools', 'pnpm-lock.yaml')")}`;

/**
 * Each check names one rule, asserts it on the parsed files, and lists its
 * mutants: edits that break exactly that rule and that it must reject.
 */
const checks = [
  {
    name: "runs on pull_request and on push to main, never pull_request_target",
    check: (ctx) => {
      assert.deepEqual(workflow(ctx).on, {
        pull_request: null,
        push: { branches: ["main"] },
      });
    },
    mutants: {
      "adds pull_request_target": (ctx) =>
        ctx.doc.setIn(["on", "pull_request_target"], null),
      "drops the push trigger": (ctx) => ctx.doc.deleteIn(["on", "push"]),
      "pushes on every branch": (ctx) =>
        ctx.doc.setIn(["on", "push", "branches"], ["main", "dev"]),
    },
  },
  {
    name: "grants contents: read and nothing else",
    check: (ctx) => {
      const wf = workflow(ctx);
      assert.deepEqual(wf.permissions, { contents: "read" });
      for (const [id, job] of Object.entries(wf.jobs)) {
        assert.equal(job.permissions, undefined, `job ${id} sets permissions`);
      }
    },
    mutants: {
      "drops the permissions block": (ctx) => ctx.doc.delete("permissions"),
      "adds a write permission": (ctx) =>
        ctx.doc.setIn(["permissions", "pull-requests"], "write"),
      "grants permissions on the job": (ctx) =>
        ctx.doc.setIn(["jobs", "gates", "permissions"], { contents: "write" }),
    },
  },
  {
    name: "reports one job with the id gates, the check name GitHub shows",
    check: (ctx) => {
      const { jobs } = workflow(ctx);
      assert.deepEqual(Object.keys(jobs), ["gates"]);
      // A job `name` would replace the id as the reported check name.
      assert.equal(jobs.gates.name, undefined, "the gates job sets a name");
    },
    mutants: {
      "renames the job": (ctx) => {
        const pair = ctx.doc
          .get("jobs", true)
          .items.find((item) => item.key.value === "gates");
        pair.key.value = "build";
      },
      "gives the job a display name": (ctx) =>
        ctx.doc.setIn(["jobs", "gates", "name"], "CI gates"),
      "adds a second job": (ctx) =>
        ctx.doc.setIn(["jobs", "lint"], { "runs-on": "ubuntu-latest" }),
    },
  },
  {
    name: "pins every action by full commit SHA with its version as a comment",
    check: (ctx) => {
      const seq = ctx.doc.getIn(STEPS, true);
      const uses = seq.items
        .map((step) => step.get("uses", true))
        .filter(Boolean);
      assert.ok(uses.length > 0, "the workflow uses no action");
      for (const node of uses) {
        assert.match(node.value, /^[\w.-]+\/[\w.-]+@[0-9a-f]{40}$/);
        assert.match(node.comment ?? "", /^ v\d+\.\d+\.\d+$/, node.value);
      }
    },
    mutants: {
      "pins checkout by tag": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, checkoutStep(ctx), "uses"],
          "actions/checkout@v7",
        ),
      "drops a version comment": (ctx) => {
        ctx.doc.getIn([...STEPS, checkoutStep(ctx), "uses"], true).comment =
          undefined;
      },
    },
  },
  {
    name: "uses no secrets",
    check: (ctx) => {
      for (const value of strings(workflow(ctx))) {
        assert.doesNotMatch(value, /secrets\.|GITHUB_TOKEN/);
      }
    },
    mutants: {
      "reads a secret": (ctx) =>
        ctx.doc.setIn([...STEPS, checkoutStep(ctx), "with"], {
          token: gh("secrets.PAT"),
        }),
      "exports GITHUB_TOKEN": (ctx) =>
        ctx.doc.setIn(["jobs", "gates", "env"], { GITHUB_TOKEN: "x" }),
    },
  },
  {
    name: "installs from the frozen lockfile, then runs the uncached project gate last",
    check: (ctx) => {
      const runs = steps(ctx)
        .map((step) => step.run)
        .filter(Boolean);
      const install = runs.indexOf("pnpm install --frozen-lockfile");
      assert.notEqual(install, -1, "no step installs from the lockfile");
      assert.equal(runs.at(-1), "pnpm gates");
      assert.ok(install < runs.length - 1, "pnpm gates runs before install");
      for (const run of runs) {
        assert.doesNotMatch(run, /moon ci|gates:dev|moon run/);
      }
    },
    mutants: {
      "installs without the frozen lockfile": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, runStep(ctx, "pnpm install --frozen-lockfile"), "run"],
          "pnpm install",
        ),
      "runs the cached inner loop": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, runStep(ctx, "pnpm gates"), "run"],
          "pnpm gates:dev",
        ),
      "runs affected-only moon ci": (ctx) =>
        ctx.doc.addIn(STEPS, { run: "moon ci" }),
    },
  },
  {
    name: "keeps the default checkout depth and states that decision above it",
    check: (ctx) => {
      const { index, step } = findStep(ctx, "actions/checkout");
      assert.equal(step.with?.["fetch-depth"], undefined, "fetch-depth is set");
      assert.match(commentAbove(ctx, index), /checkout depth/i);
    },
    mutants: {
      "fetches the full history": (ctx) =>
        ctx.doc.setIn([...STEPS, checkoutStep(ctx), "with"], {
          "fetch-depth": 0,
        }),
      "drops the depth comment": (ctx) => {
        ctx.doc.getIn(STEPS, true).commentBefore = undefined;
      },
    },
  },
  {
    name: "never caches or restores moon's task cache",
    check: (ctx) => {
      for (const cache of caches(ctx)) {
        for (const path of cache.paths) {
          assert.doesNotMatch(path, /(^|\/)\.moon(\/|$)/, cache.name);
        }
      }
    },
    mutants: {
      "caches .moon/cache": (ctx) =>
        ctx.doc.addIn(STEPS, {
          uses: steps(ctx).find((step) =>
            step.uses?.startsWith("actions/cache@"),
          ).uses,
          with: { path: ".moon/cache", key: "moon" },
        }),
    },
  },
  {
    name: "bounds the Cargo target cache: one exact key, no restore-keys fallback",
    check: (ctx) => {
      // actions/cache saves only on a key miss, so one entry holds at most
      // one build per toolchain and lockfile. A restore-keys fallback would
      // seed every new entry with the previous target/ and let it grow.
      const cargo = caches(ctx).filter((cache) =>
        cache.paths.includes("target"),
      );
      assert.equal(cargo.length, 1, "exactly one step caches target/");
      assert.equal(cargo[0].with.key, CARGO_KEY);
      assert.equal(cargo[0].with["restore-keys"], undefined);
    },
    mutants: {
      "adds a restore-keys fallback": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, cargoStep(ctx), "with", "restore-keys"],
          "cargo-",
        ),
      "keys on the toolchain only": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, cargoStep(ctx), "with", "key"],
          `cargo-${gh("runner.os")}-${gh("hashFiles('rust-toolchain.toml')")}`,
        ),
      "caches target/ twice": (ctx) =>
        ctx.doc.addIn(STEPS, structuredClone(steps(ctx)[cargoStep(ctx)])),
    },
  },
  {
    name: "keys the pnpm store cache on .prototools and pnpm-lock.yaml",
    check: (ctx) => {
      const store = caches(ctx).filter((cache) =>
        cache.with.key.startsWith("pnpm-"),
      );
      assert.equal(store.length, 1, "exactly one pnpm store cache");
      assert.equal(store[0].with.key, PNPM_KEY);
    },
    mutants: {
      "keys on the lockfile only": (ctx) =>
        ctx.doc.setIn(
          [...STEPS, pnpmStep(ctx), "with", "key"],
          `pnpm-${gh("runner.os")}-${gh("hashFiles('pnpm-lock.yaml')")}`,
        ),
    },
  },
  {
    // A hung step must not burn the free-plan Actions minutes until GitHub's
    // 6-hour default job timeout.
    name: `bounds the gates job at ${TIMEOUT_MINUTES} minutes`,
    check: (ctx) => {
      assert.equal(
        workflow(ctx).jobs.gates["timeout-minutes"],
        TIMEOUT_MINUTES,
      );
    },
    mutants: {
      "drops the timeout": (ctx) =>
        ctx.doc.deleteIn(["jobs", "gates", "timeout-minutes"]),
      "raises the timeout within the old range": (ctx) =>
        ctx.doc.setIn(["jobs", "gates", "timeout-minutes"], 45),
    },
  },
  {
    // Superseded pull-request runs are cancelled; every push to main runs to
    // the end. A shared group would still cancel a *pending* main run when
    // the next push queues behind it, so push runs get a group of their own
    // per commit.
    name: "cancels superseded pull-request runs only, never a run on main",
    check: (ctx) => {
      const wf = workflow(ctx);
      assert.deepEqual(wf.concurrency, CONCURRENCY);
      assert.equal(wf.jobs.gates.concurrency, undefined, "job concurrency");
    },
    mutants: {
      "drops the concurrency block": (ctx) => ctx.doc.delete("concurrency"),
      "always cancels": (ctx) =>
        ctx.doc.setIn(["concurrency", "cancel-in-progress"], true),
      "shares one group per ref, main included": (ctx) =>
        ctx.doc.setIn(["concurrency", "group"], `ci-${gh("github.ref")}`),
      "overrides concurrency on the job": (ctx) =>
        ctx.doc.setIn(["jobs", "gates", "concurrency"], { group: "x" }),
    },
  },
  {
    // `moon exec` queries changed files on every run, even with --force. In a
    // pull request it takes the base from GITHUB_BASE_REF (`main`), which a
    // shallow checkout does not contain; --base HEAD --head HEAD keeps that
    // query resolvable. --on-failure continue gives every gate a result.
    name: "pnpm gates forces every task, diffs HEAD against itself and continues past failures",
    check: (ctx) => {
      assert.deepEqual(ctx.pkg.scripts.gates.split(/\s+/), GATES_ARGV);
    },
    mutants: {
      "stops at the first failure": (ctx) => {
        ctx.pkg.scripts.gates = ctx.pkg.scripts.gates.replace(
          " --on-failure continue",
          "",
        );
      },
      "bails on failure": (ctx) => {
        ctx.pkg.scripts.gates = ctx.pkg.scripts.gates.replace(
          "continue",
          "bail",
        );
      },
      "reads the task cache": (ctx) => {
        ctx.pkg.scripts.gates = ctx.pkg.scripts.gates.replace(" --force", "");
      },
      "selects affected tasks": (ctx) => {
        ctx.pkg.scripts.gates += " --affected";
      },
      "diffs against main": (ctx) => {
        ctx.pkg.scripts.gates = ctx.pkg.scripts.gates.replace(
          "--base HEAD",
          "--base main",
        );
      },
    },
  },
  {
    name: "pnpm build scripts stay an explicit, empty allow-list",
    check: (ctx) => {
      assert.deepEqual(ctx.workspace.onlyBuiltDependencies, []);
      assert.equal(ctx.workspace.dangerouslyAllowAllBuilds, undefined);
    },
    mutants: {
      "drops the allow-list": (ctx) => {
        delete ctx.workspace.onlyBuiltDependencies;
      },
      "allows a build script": (ctx) => {
        ctx.workspace.onlyBuiltDependencies.push("esbuild");
      },
      "allows every build script": (ctx) => {
        ctx.workspace.dangerouslyAllowAllBuilds = true;
      },
    },
  },
  {
    name: "Dependabot keeps the action pins current, weekly, and nothing else",
    check: (ctx) => {
      assert.ok(ctx.dependabot, `${DEPENDABOT} is missing`);
      assert.equal(ctx.dependabot.version, 2);
      assert.deepEqual(ctx.dependabot.updates, DEPENDABOT_UPDATES);
    },
    mutants: {
      "has no dependabot.yml": (ctx) => {
        ctx.dependabot = null;
      },
      "uses config version 1": (ctx) => {
        ctx.dependabot.version = 1;
      },
      "drops the github-actions ecosystem": (ctx) => {
        ctx.dependabot.updates = [];
      },
      "checks monthly": (ctx) => {
        ctx.dependabot.updates[0].schedule.interval = "monthly";
      },
      "watches a subdirectory only": (ctx) => {
        ctx.dependabot.updates[0].directory = "/.github/workflows";
      },
      "adds the npm ecosystem": (ctx) => {
        ctx.dependabot.updates.push({
          "package-ecosystem": "npm",
          directory: "/",
          schedule: { interval: "weekly" },
        });
      },
      "adds the cargo ecosystem": (ctx) => {
        ctx.dependabot.updates.push({
          "package-ecosystem": "cargo",
          directory: "/",
          schedule: { interval: "weekly" },
        });
      },
    },
  },
];

runWorkflowChecks({ files: load(), clone, checks });
