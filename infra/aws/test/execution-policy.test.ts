// CloudFormation's execution role against the relay's stack (NOC-272; NOC-264
// point 4, least privilege). The owner creates the two managed policies in
// infra/aws/execution-policy once and passes them to `cdk bootstrap
// --cloudformation-execution-policies` (docs/relay-deploy.md), instead of the
// default AdministratorAccess.
//
// What a resource type needs is what CloudFormation's own registry says its
// handlers call: test/cloudformation-handlers.json holds the create, read,
// update and delete permissions of each type the stack uses, from the
// registry's schemas for eu-central-1
// (https://schema.cloudformation.eu-central-1.amazonaws.com/CloudformationSchema.zip,
// `handlers.<op>.permissions`, 2026-11-12). A handler names every action any
// configuration of its type might call; `unused` lists, with the reason, the
// ones this stack's configuration never reaches.
//
// The tests: every resource type and parameter type of the synthesized
// template is known; every action it needs is allowed; the policies allow
// nothing it does not need; and each statement names the stack's own
// resources, except the few actions IAM gives no resource for.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { App } from "aws-cdk-lib";
import { describe, expect, test } from "vitest";
import { RelayStack } from "../src/relay-stack.ts";
import { lambdaPackage } from "./lambda-package.ts";

const here = import.meta.dirname;
const read = (path: string): unknown =>
  JSON.parse(readFileSync(join(here, path), "utf8"));

type Statement = {
  Sid: string;
  Effect: string;
  Action: string | string[];
  Resource: string | string[];
  Condition?: Record<string, Record<string, string>>;
};
type Policy = { Version: string; Statement: Statement[] };

const policies = Object.fromEntries(
  ["storage", "service"].map((name) => [
    name,
    read(`../execution-policy/${name}.json`) as Policy,
  ]),
);
const statements = Object.values(policies).flatMap((p) => p.Statement);
const list = (v: string | string[]) => (Array.isArray(v) ? v : [v]);

const handlers = read("cloudformation-handlers.json") as Record<
  string,
  string[]
>;

/// What the registry has no handler permissions for.
const extra: Record<string, string[]> = {
  // A legacy resource type, with no handlers in the registry.
  "AWS::Budgets::Budget": ["budgets:ModifyBudget", "budgets:ViewBudget"],
  // CDK's check of the bootstrap version, a template parameter.
  "AWS::SSM::Parameter::Value<String>": ["ssm:GetParameters"],
};

/// Actions a handler names that this stack's configuration never reaches.
const unused: [pattern: RegExp, reason: string][] = [
  [/^kinesis:/, "no table streams to Kinesis"],
  [/^dynamodb:\w*KinesisStreamingDestination$/, "no Kinesis streaming"],
  [
    /^dynamodb:(CreateTableReplica|DeleteTableReplica|DisassociateTableReplica|UpdateTableReplicaAutoScaling)$/,
    "each table has one replica, in eu-central-1",
  ],
  [
    /^application-autoscaling:(?!Describe)/,
    "the tables are on demand, with no scaling",
  ],
  [/^iam:CreateServiceLinkedRole$/, "no replica in a second region"],
  [
    /^kms:/,
    "no customer managed key: the tables, the bucket and the function use AWS's keys",
  ],
  [
    /^(ec2|elasticfilesystem|s3files):/,
    "the function is in no VPC and mounts no file system",
  ],
  [/^lambda:GetLayerVersion$/, "the function has no layers"],
  [/^lambda:GetCodeSigningConfig$/, "the function has no code signing config"],
  [/^lambda:PassCapacityProvider$/, "the function has no capacity provider"],
  [/^(firehose:|s3:REST\.PUT\.OBJECT$)/, "the logs are delivered nowhere else"],
  [
    /^logs:(PutResourcePolicy|DeleteResourcePolicy)$/,
    "the log group has no resource policy",
  ],
  [/^s3tables:/, "the bucket has no metadata tables"],
  [
    /^s3:(PutBucketReplication|PutReplicationConfiguration)$/,
    "the bucket is not replicated",
  ],
  [/^iam:\w+(Group|User)Policy$/, "the stack's one policy is a role's"],
];

const isUnused = (action: string) =>
  unused.some(([pattern]) => pattern.test(action));

/// IAM's match of an action against a pattern: case-insensitive, `*` for any.
const matches = (pattern: string, action: string) =>
  new RegExp(
    `^${pattern.replace(/[.+?^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*")}$`,
    "i",
  ).test(action);

const allowedBy = (action: string) =>
  statements.filter(
    (s) =>
      s.Effect === "Allow" && list(s.Action).some((a) => matches(a, action)),
  );

function template(): Record<string, Record<string, { Type: string }>> {
  const app = new App();
  const stack = new RelayStack(app, "NoCentralDbRelay-prod", {
    stage: "prod",
    relayId: "5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e",
    allowedAccounts: [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ],
    alarmEmail: "owner@example.invalid",
    lambdaPackage: lambdaPackage(),
    env: { region: "eu-central-1" },
  });
  return app.synth().getStackByName(stack.stackName).template;
}

const synthesized = template();
const types = new Set(
  [
    ...Object.values(synthesized.Resources ?? {}),
    ...Object.values(synthesized.Parameters ?? {}),
  ].map((r) => r.Type),
);
const needs = (type: string) => [
  ...(handlers[type] ?? []),
  ...(extra[type] ?? []),
];
const needed = [...types].flatMap(needs).filter((action) => !isUnused(action));

describe("CloudFormation's execution policies", () => {
  test("every resource and parameter type of the stack is known", () => {
    for (const type of types) {
      expect(needs(type), type).not.toEqual([]);
    }
  });

  test("allow every action the stack's resource types need", () => {
    const missing = needed.filter((action) => allowedBy(action).length === 0);
    expect(missing).toEqual([]);
  });

  test("allow no action the stack does not need", () => {
    const granted = statements.flatMap((s) => list(s.Action));
    const spare = granted.filter(
      (pattern) => !needed.some((action) => matches(pattern, action)),
    );
    expect(spare).toEqual([]);
  });

  test("leave out only actions some handler of the stack names", () => {
    const all = [...types].flatMap(needs);
    for (const [pattern, reason] of unused) {
      expect(
        all.some((action) => pattern.test(action)),
        reason,
      ).toBe(true);
    }
  });

  test("grant no service's every action, and nothing outside eu-central-1", () => {
    for (const s of statements) {
      expect(s.Effect).toBe("Allow");
      for (const action of list(s.Action)) {
        expect(action).not.toMatch(/(^\*$|:\*$)/);
      }
      for (const resource of list(s.Resource)) {
        if (resource === "*") continue;
        expect(resource).toMatch(
          /^arn:aws:(s3|iam|budgets)::|^arn:aws:[a-z0-9-]+:eu-central-1:/,
        );
      }
    }
  });

  test("name the stack's own resources, except the actions IAM gives no resource for", () => {
    const anywhere = statements
      .filter((s) => list(s.Resource).includes("*"))
      .flatMap((s) => list(s.Action))
      .sort();
    expect(anywhere).toEqual([
      "application-autoscaling:Describe*",
      "cloudwatch:DescribeAlarms",
      "cloudwatch:PutMetricData",
      "logs:DescribeLogGroups",
      "logs:DescribeResourcePolicies",
    ]);
  });

  test("touch IAM only for the stack's role, which goes only to Lambda with the basic policy", () => {
    for (const s of statements) {
      if (!list(s.Action).some((a) => a.startsWith("iam:"))) continue;
      expect(list(s.Action).every((a) => a.startsWith("iam:"))).toBe(true);
      expect(s.Resource).toBe(
        `arn:aws:iam::\${aws:PrincipalAccount}:role/NoCentralDbRelay-*`,
      );
    }
    const [pass] = allowedBy("iam:PassRole");
    expect(allowedBy("iam:PassRole")).toHaveLength(1);
    expect(pass?.Condition).toEqual({
      StringEquals: { "iam:PassedToService": "lambda.amazonaws.com" },
    });
    for (const action of ["iam:AttachRolePolicy", "iam:DetachRolePolicy"]) {
      expect(allowedBy(action).map((s) => s.Condition)).toEqual([
        {
          ArnEquals: {
            "iam:PolicyARN":
              "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole",
          },
        },
      ]);
    }
  });

  test("the function's role gets only the basic Lambda policy attached", () => {
    const roles = Object.values(synthesized.Resources ?? {}).filter(
      (r) => r.Type === "AWS::IAM::Role",
    ) as unknown as { Properties: { ManagedPolicyArns?: unknown[] } }[];
    for (const role of roles) {
      expect(JSON.stringify(role.Properties.ManagedPolicyArns ?? [])).toMatch(
        /^\[\{"Fn::Join":\["",\["arn:",\{"Ref":"AWS::Partition"\},":iam::aws:policy\/service-role\/AWSLambdaBasicExecutionRole"\]\]\}\]$/,
      );
    }
  });

  test("the resources the policies name by prefix get those names", () => {
    // CloudFormation names a resource with no name of its own after the
    // stack, `NoCentralDbRelay-prod-…` (a bucket in lower case); the tables
    // and the budget carry the stage's `ncdb-` names.
    const named = Object.values(synthesized.Resources ?? {}).flatMap((r) =>
      Object.entries(
        (r as unknown as { Properties?: Record<string, unknown> }).Properties ??
          {},
      )
        // A name the stack sets itself; a reference to a resource is fine.
        .filter(
          ([key, value]) =>
            typeof value === "string" &&
            (/^(FunctionName|RoleName|TopicName|BucketName|AlarmName|LogGroupName)$/.test(
              key,
            ) ||
              (r.Type === "AWS::Events::Rule" && key === "Name")),
        )
        .map(([key]) => `${r.Type}.${key}`),
    );
    expect(named).toEqual([]);
    for (const r of Object.values(synthesized.Resources ?? {})) {
      const props = (r as unknown as { Properties: Record<string, unknown> })
        .Properties;
      if (r.Type === "AWS::DynamoDB::GlobalTable") {
        expect(props.TableName).toMatch(/^ncdb-/);
      }
      if (r.Type === "AWS::Budgets::Budget") {
        expect((props.Budget as { BudgetName: string }).BudgetName).toMatch(
          /^ncdb-/,
        );
      }
    }
    const app = readFileSync(join(here, "../bin/relay.ts"), "utf8");
    expect(app).toContain(`new RelayStack(app, \`NoCentralDbRelay-\${stage}\``);
  });

  test("each fits IAM's limit for a managed policy", () => {
    for (const [name, policy] of Object.entries(policies)) {
      expect(policy.Version).toBe("2012-10-17");
      const size = JSON.stringify(policy).replace(/\s/g, "").length;
      expect(size, name).toBeLessThanOrEqual(6144);
    }
  });
});
