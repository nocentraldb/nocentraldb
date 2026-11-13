// Asserts .github/workflows/relay-deploy.yml (NOC-272; NOC-264 point 4 B):
// the build and deploy of the relay on AWS.
//
// - The triggers: pull requests and pushes to main that touch the relay.
// - The build job: the function's arm64 package with cargo-lambda installed
//   by hash, and no AWS access.
// - The deploy job: a push to main only, in the environment `aws-prod`,
//   OIDC with no stored key, the only job with `id-token: write`; it shows
//   `cdk diff` before `cdk deploy`; the deployment's values reach the shell
//   as variables, never spliced into a command.
// - Every action is pinned by SHA.
//
// Each check also rejects its mutants (scripts/lib/workflow-checks.mjs).
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parse as parseToml } from "smol-toml";
import { parse } from "yaml";
import { runWorkflowChecks } from "./lib/workflow-checks.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const WORKFLOW = ".github/workflows/relay-deploy.yml";
const text = existsSync(join(root, WORKFLOW))
  ? readFileSync(join(root, WORKFLOW), "utf8")
  : null;

const CODEOWNERS = ".github/CODEOWNERS";
const codeowners = existsSync(join(root, CODEOWNERS))
  ? readFileSync(join(root, CODEOWNERS), "utf8")
  : null;

/** The owner who must approve every change that is deployed to AWS. */
const OWNER = "@cabcookie";

/** CODEOWNERS' entries, as [pattern, owners]. */
const owned = (text) =>
  text
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "" && !line.startsWith("#"))
    .map((line) => {
      const [pattern, ...owners] = line.split(/\s+/);
      return [pattern, owners];
    });

/** A workflow path filter as CODEOWNERS spells it: `a/**` is `/a/`. */
const asOwned = (path) =>
  path.endsWith("/**") ? `/${path.slice(0, -2)}` : `/${path}`;

/**
 * The workspace crates the function links in its deployed build: the
 * normal dependencies, optional ones included, of ncdb-relay-lambda and of
 * every workspace crate they reach, as `crates/<dir>`.
 */
function linkedCrates() {
  const workspace = parseToml(readFileSync(join(root, "Cargo.toml"), "utf8"));
  const paths = Object.fromEntries(
    Object.entries(workspace.workspace.dependencies)
      .filter(([, d]) => typeof d === "object" && d.path)
      .map(([name, d]) => [name, d.path]),
  );
  const seen = new Set();
  const queue = ["crates/ncdb-relay-lambda"];
  while (queue.length > 0) {
    const dir = queue.shift();
    if (seen.has(dir)) continue;
    seen.add(dir);
    const manifest = parseToml(
      readFileSync(join(root, dir, "Cargo.toml"), "utf8"),
    );
    for (const [name, dep] of Object.entries(manifest.dependencies ?? {})) {
      if (typeof dep === "object" && dep.workspace && paths[name]) {
        queue.push(paths[name]);
      }
    }
  }
  return [...seen].sort();
}

const workflow = (ctx) => {
  assert.ok(ctx.workflow, `${WORKFLOW} is missing`);
  return ctx.workflow;
};

const job = (ctx, id) => {
  const found = workflow(ctx).jobs?.[id];
  assert.ok(found, `${WORKFLOW} has no job ${id}`);
  return found;
};

/** A GitHub Actions expression, `${{ … }}`, as the workflow spells it. */
const gh = (expression) => `\${{ ${expression} }}`;

const runs = (j) => (j.steps ?? []).map((s) => s.run ?? "").join("\n");

const INSTALL_TOOLS =
  'pip install --require-hashes --only-binary ":all:" -r infra/aws/cargo-lambda.txt';
const BUILD =
  "cargo lambda build --locked --release --arm64 -p ncdb-relay-lambda --features aws --bin ncdb-relay-lambda";
const RELAY_PATHS = [
  "infra/aws/**",
  "crates/ncdb-core/**",
  "crates/ncdb-plugin/**",
  "crates/ncdb-relay/**",
  "crates/ncdb-relay-aws/**",
  "crates/ncdb-relay-lambda/**",
];

runWorkflowChecks({
  files: { workflow: text === null ? null : parse(text), text, codeowners },
  clone: (files) => structuredClone(files),
  checks: [
    {
      name: "it runs on pull requests and on pushes to main that touch the relay",
      check: (ctx) => {
        const on = workflow(ctx).on ?? {};
        assert.deepEqual(Object.keys(on).sort(), ["pull_request", "push"]);
        assert.deepEqual(on.push.branches, ["main"]);
        for (const trigger of [on.push, on.pull_request]) {
          for (const path of [...RELAY_PATHS, WORKFLOW]) {
            assert.ok(trigger.paths?.includes(path), `no path ${path}`);
          }
        }
      },
      mutants: {
        "on every branch": (ctx) => {
          delete ctx.workflow.on.push.branches;
        },
        "by hand too": (ctx) => {
          ctx.workflow.on.workflow_dispatch = null;
        },
        "misses the Lambda crate": (ctx) => {
          ctx.workflow.on.push.paths = ctx.workflow.on.push.paths.filter(
            (p) => p !== "crates/ncdb-relay-lambda/**",
          );
        },
      },
    },
    {
      name: "it runs on every workspace crate the function links",
      check: (ctx) => {
        const on = workflow(ctx).on ?? {};
        for (const dir of linkedCrates()) {
          for (const trigger of [on.push, on.pull_request]) {
            assert.ok(
              trigger.paths?.includes(`${dir}/**`),
              `no path ${dir}/**`,
            );
          }
        }
      },
      mutants: {
        "misses a crate the function links": (ctx) => {
          ctx.workflow.on.push.paths = ctx.workflow.on.push.paths.filter(
            (p) => p !== "crates/ncdb-plugin/**",
          );
        },
      },
    },
    {
      name: "the owner must approve every change that is deployed: CODEOWNERS owns exactly the deploy's paths",
      check: (ctx) => {
        assert.ok(ctx.codeowners, `${CODEOWNERS} is missing`);
        const on = workflow(ctx).on ?? {};
        assert.deepEqual(
          [...(on.push.paths ?? [])].sort(),
          [...(on.pull_request.paths ?? [])].sort(),
          "the push and pull request paths differ",
        );
        const entries = owned(ctx.codeowners);
        for (const [pattern, owners] of entries) {
          assert.deepEqual(
            owners,
            [OWNER],
            `${pattern} is not the owner's alone`,
          );
        }
        assert.deepEqual(
          entries.map(([pattern]) => pattern).sort(),
          [...on.push.paths.map(asOwned), `/${CODEOWNERS}`].sort(),
        );
      },
      mutants: {
        "a deployed path without an owner": (ctx) => {
          ctx.codeowners = ctx.codeowners
            .split("\n")
            .filter((l) => !l.startsWith("/crates/ncdb-relay-aws/"))
            .join("\n");
        },
        "CODEOWNERS owns itself no more": (ctx) => {
          ctx.codeowners = ctx.codeowners
            .split("\n")
            .filter((l) => !l.startsWith("/.github/CODEOWNERS"))
            .join("\n");
        },
        "another owner may approve": (ctx) => {
          ctx.codeowners = ctx.codeowners.replace(
            "/infra/aws/ @cabcookie",
            "/infra/aws/ @cabcookie @someone-else",
          );
        },
        "a trigger the owner does not own": (ctx) => {
          ctx.workflow.on.push.paths.push("crates/ncdb/**");
          ctx.workflow.on.pull_request.paths.push("crates/ncdb/**");
        },
        "pull requests watch other paths": (ctx) => {
          ctx.workflow.on.pull_request.paths.pop();
        },
      },
    },
    {
      name: "the workflow reads only, and only the deploy job may ask for an OIDC token",
      check: (ctx) => {
        assert.deepEqual(workflow(ctx).permissions, { contents: "read" });
        assert.equal(job(ctx, "build").permissions, undefined);
        assert.deepEqual(job(ctx, "deploy").permissions, {
          contents: "read",
          "id-token": "write",
        });
      },
      mutants: {
        "an OIDC token for the whole workflow": (ctx) => {
          ctx.workflow.permissions["id-token"] = "write";
        },
        "the build job may ask for one": (ctx) => {
          ctx.workflow.jobs.build.permissions = { "id-token": "write" };
        },
        "the deploy job writes the repository": (ctx) => {
          ctx.workflow.jobs.deploy.permissions.contents = "write";
        },
      },
    },
    {
      name: "the deploy runs on a push to main only, in the environment aws-prod, after the build",
      check: (ctx) => {
        const deploy = job(ctx, "deploy");
        assert.equal(
          deploy.if,
          "github.event_name == 'push' && github.ref == 'refs/heads/main'",
        );
        assert.equal(deploy.environment, "aws-prod");
        assert.equal(deploy.needs, "build");
      },
      mutants: {
        "on pull requests too": (ctx) => {
          delete ctx.workflow.jobs.deploy.if;
        },
        "without the environment": (ctx) => {
          delete ctx.workflow.jobs.deploy.environment;
        },
        "without the build first": (ctx) => {
          delete ctx.workflow.jobs.deploy.needs;
        },
      },
    },
    {
      name: "both jobs build the package with cargo-lambda installed by hash",
      check: (ctx) => {
        for (const id of ["build", "deploy"]) {
          const commands = runs(job(ctx, id));
          assert.ok(
            commands.includes(INSTALL_TOOLS),
            `${id}: no install by hash`,
          );
          assert.ok(commands.includes(BUILD), `${id}: no build of the package`);
        }
        const pins = readFileSync(
          join(root, "infra/aws/cargo-lambda.txt"),
          "utf8",
        );
        for (const tool of ["cargo-lambda==", "ziglang=="]) {
          assert.ok(pins.includes(tool), `cargo-lambda.txt pins no ${tool}`);
        }
        assert.equal(
          (pins.match(/--hash=sha256:[0-9a-f]{64}/g) ?? []).length,
          2,
          "each tool has its hash",
        );
      },
      mutants: {
        "installs without hashes": (ctx) => {
          for (const s of ctx.workflow.jobs.build.steps) {
            if (s.run?.includes("pip install")) {
              s.run = "pip install cargo-lambda";
            }
          }
        },
        "the deploy uses another build": (ctx) => {
          for (const s of ctx.workflow.jobs.deploy.steps) {
            if (s.run?.includes("cargo lambda build")) {
              s.run = "cargo lambda build --release";
            }
          }
        },
      },
    },
    {
      name: "the deploy assumes its role through OIDC and shows the diff before it deploys",
      check: (ctx) => {
        const steps = job(ctx, "deploy").steps ?? [];
        const assume = steps.findIndex((s) =>
          s.uses?.startsWith("aws-actions/configure-aws-credentials@"),
        );
        assert.ok(assume >= 0, "no OIDC step");
        const creds = steps[assume].with ?? {};
        assert.equal(creds["role-to-assume"], gh("vars.AWS_DEPLOY_ROLE_ARN"));
        assert.equal(creds["aws-region"], "eu-central-1");
        for (const key of ["aws-access-key-id", "aws-secret-access-key"]) {
          assert.equal(creds[key], undefined, `a stored key: ${key}`);
        }
        const diff = steps.findIndex((s) => s.run?.includes("cdk diff"));
        const deploy = steps.findIndex((s) => s.run?.includes("cdk deploy"));
        assert.ok(diff > assume, "the diff follows the role");
        assert.ok(deploy > diff, "the deploy follows the diff");
      },
      mutants: {
        "a stored key": (ctx) => {
          for (const s of ctx.workflow.jobs.deploy.steps) {
            if (s.uses?.startsWith("aws-actions/configure-aws-credentials@")) {
              s.with["aws-access-key-id"] = gh("secrets.KEY");
            }
          }
        },
        "no diff": (ctx) => {
          ctx.workflow.jobs.deploy.steps =
            ctx.workflow.jobs.deploy.steps.filter(
              (s) => !s.run?.includes("cdk diff"),
            );
        },
        "another region": (ctx) => {
          for (const s of ctx.workflow.jobs.deploy.steps) {
            if (s.uses?.startsWith("aws-actions/configure-aws-credentials@")) {
              s.with["aws-region"] = "us-east-1";
            }
          }
        },
      },
    },
    {
      name: "before the owner's one-time setup the deploy skips with a notice, and stays green",
      check: (ctx) => {
        const steps = job(ctx, "deploy").steps ?? [];
        const ready = steps.findIndex((s) => s.id === "ready");
        assert.ok(ready >= 0, "no step with the id ready");
        const check = steps[ready];
        assert.equal(check.env?.ROLE, gh("vars.AWS_DEPLOY_ROLE_ARN"));
        assert.match(check.run ?? "", /if \[ -z "\$ROLE" \]/);
        assert.match(check.run ?? "", /::notice /);
        assert.match(check.run ?? "", /ready=false/);
        assert.match(check.run ?? "", /ready=true/);
        assert.ok(
          !/exit 1/.test(check.run ?? ""),
          "the check must not fail the job",
        );
        const later = steps.filter(
          (s, i) => i !== ready && !s.uses?.startsWith("actions/checkout@"),
        );
        const assume = steps.findIndex((s) =>
          s.uses?.startsWith("aws-actions/configure-aws-credentials@"),
        );
        assert.ok(ready < assume, "the check comes before the OIDC step");
        for (const s of later) {
          assert.equal(
            s.if,
            "steps.ready.outputs.ready == 'true'",
            `${s.name ?? s.uses ?? s.run} runs before the setup`,
          );
        }
      },
      mutants: {
        "no check": (ctx) => {
          ctx.workflow.jobs.deploy.steps =
            ctx.workflow.jobs.deploy.steps.filter((s) => s.id !== "ready");
        },
        "the OIDC step runs anyway": (ctx) => {
          for (const s of ctx.workflow.jobs.deploy.steps) {
            if (s.uses?.startsWith("aws-actions/configure-aws-credentials@")) {
              delete s.if;
            }
          }
        },
        "the deploy runs anyway": (ctx) => {
          for (const s of ctx.workflow.jobs.deploy.steps) {
            if (s.run?.includes("cdk deploy")) {
              delete s.if;
            }
          }
        },
        "the check fails red": (ctx) => {
          for (const s of ctx.workflow.jobs.deploy.steps) {
            if (s.id === "ready") {
              s.run =
                'if [ -z "$ROLE" ]; then echo "::notice ::x"; echo ready=false; echo ready=true; exit 1; fi';
            }
          }
        },
      },
    },
    {
      name: "the deployment's values reach the shell as variables, never spliced into it",
      check: (ctx) => {
        for (const [id, j] of Object.entries(workflow(ctx).jobs)) {
          assert.ok(
            !/\$\{\{[^}]*\}\}/.test(runs(j)),
            `job ${id} splices an expression into a command`,
          );
        }
        const env = job(ctx, "deploy").env ?? {};
        assert.equal(env.RELAY_ID, gh("vars.RELAY_ID"));
        assert.equal(env.ALLOWED_ACCOUNTS, gh("vars.ALLOWED_ACCOUNTS"));
        assert.equal(env.ALARM_EMAIL, gh("vars.ALARM_EMAIL"));
      },
      mutants: {
        "spliced into the deploy": (ctx) => {
          for (const s of ctx.workflow.jobs.deploy.steps) {
            if (s.run?.includes("cdk deploy")) {
              s.run = s.run.replace('"$RELAY_ID"', `"${gh("vars.RELAY_ID")}"`);
            }
          }
        },
        "the values from secrets": (ctx) => {
          ctx.workflow.jobs.deploy.env.ALARM_EMAIL = gh("secrets.ALARM_EMAIL");
        },
      },
    },
    {
      name: "every action is pinned by a full commit SHA",
      check: (ctx) => {
        const uses = Object.values(workflow(ctx).jobs).flatMap((j) =>
          (j.steps ?? []).map((s) => s.uses).filter(Boolean),
        );
        assert.ok(uses.length > 0);
        for (const action of uses) {
          assert.match(
            action,
            /@[0-9a-f]{40}$/,
            `${action} is not pinned by SHA`,
          );
        }
      },
      mutants: {
        "a tag": (ctx) => {
          ctx.workflow.jobs.build.steps[0].uses = "actions/checkout@v7";
        },
      },
    },
  ],
});
