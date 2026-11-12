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
import { parse } from "yaml";
import { runWorkflowChecks } from "./lib/workflow-checks.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const WORKFLOW = ".github/workflows/relay-deploy.yml";
const text = existsSync(join(root, WORKFLOW))
  ? readFileSync(join(root, WORKFLOW), "utf8")
  : null;

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
  "crates/ncdb-relay/**",
  "crates/ncdb-relay-aws/**",
  "crates/ncdb-relay-lambda/**",
];

runWorkflowChecks({
  files: { workflow: text === null ? null : parse(text), text },
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
