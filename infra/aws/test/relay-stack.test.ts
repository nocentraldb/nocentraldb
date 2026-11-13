// The relay's stack against RFC 0007, "The AWS backend": its tables and
// keys, the bucket, the function, the WebSocket API, the logs and the
// budget alarm. It synthesizes with no AWS credentials. The template is
// also kept as a snapshot, so a pull request shows how it changes the
// deployed infrastructure without credentials (NOC-264, NOC-272).

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { App } from "aws-cdk-lib";
import { Match, Template } from "aws-cdk-lib/assertions";
import { CURRENTLY_RECOMMENDED_FLAGS } from "aws-cdk-lib/cx-api";
import { describe, expect, test } from "vitest";
import { RelayStack } from "../src/relay-stack.ts";
import { flags, lambdaPackage } from "./support.ts";

function synth(): Template {
  const app = new App({ context: flags() });
  const stack = new RelayStack(app, "NoCentralDbRelay", {
    stage: "prod",
    relayId: "5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e",
    allowedAccounts: [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ],
    alarmEmail: "owner@example.invalid",
    lambdaPackage: lambdaPackage(),
  });
  return Template.fromStack(stack);
}

const template = synth();

type Key = [name: string, type: "B" | "N" | "S", kind: "HASH" | "RANGE"];

/// RFC 0007's tables ("Tables and keys"), NOC-270's `uploads`, and the
/// `connections` table of the Lambda crate.
const tables: Record<string, { keys: Key[]; ttl?: string }> = {
  log: {
    keys: [
      ["space", "B", "HASH"],
      ["position", "N", "RANGE"],
    ],
  },
  changes: {
    keys: [
      ["space", "B", "HASH"],
      ["change", "B", "RANGE"],
    ],
  },
  spaces: { keys: [["space", "B", "HASH"]] },
  members: {
    keys: [
      ["space", "B", "HASH"],
      ["member", "B", "RANGE"],
    ],
  },
  "person-keys": {
    keys: [
      ["space", "B", "HASH"],
      ["id", "B", "RANGE"],
    ],
  },
  uploads: { keys: [["id", "B", "HASH"]], ttl: "ttl" },
  connections: {
    keys: [
      ["connection", "S", "HASH"],
      ["item", "S", "RANGE"],
    ],
    ttl: "ttl",
  },
};

describe("the tables", () => {
  test("there are exactly the seven tables, and no tokens table (NOC-295 A)", () => {
    template.resourceCountIs("AWS::DynamoDB::GlobalTable", 7);
  });

  for (const [name, { keys, ttl }] of Object.entries(tables)) {
    test(`${name}: its keys, on demand, point-in-time recovery for 35 days`, () => {
      template.hasResourceProperties("AWS::DynamoDB::GlobalTable", {
        TableName: `ncdb-prod-${name}`,
        BillingMode: "PAY_PER_REQUEST",
        KeySchema: keys.map(([AttributeName, , KeyType]) => ({
          AttributeName,
          KeyType,
        })),
        AttributeDefinitions: Match.arrayWith(
          keys.map(([AttributeName, AttributeType]) => ({
            AttributeName,
            AttributeType,
          })),
        ),
        Replicas: [
          Match.objectLike({
            PointInTimeRecoverySpecification: {
              PointInTimeRecoveryEnabled: true,
              RecoveryPeriodInDays: 35,
            },
          }),
        ],
        ...(ttl
          ? {
              TimeToLiveSpecification: { AttributeName: ttl, Enabled: true },
            }
          : {}),
      });
    });
  }

  test("no table is named tokens", () => {
    const names = Object.values(
      template.findResources("AWS::DynamoDB::GlobalTable"),
    ).map((t) => t.Properties.TableName);
    expect(names).not.toContain("ncdb-prod-tokens");
  });

  test("members: the index on member id, sorted by space, with current", () => {
    template.hasResourceProperties("AWS::DynamoDB::GlobalTable", {
      TableName: "ncdb-prod-members",
      GlobalSecondaryIndexes: [
        {
          IndexName: "by_member",
          KeySchema: [
            { AttributeName: "member", KeyType: "HASH" },
            { AttributeName: "space", KeyType: "RANGE" },
          ],
          Projection: {
            ProjectionType: "INCLUDE",
            NonKeyAttributes: ["current"],
          },
        },
      ],
    });
  });

  test("connections: the indexes by space and by member", () => {
    template.hasResourceProperties("AWS::DynamoDB::GlobalTable", {
      TableName: "ncdb-prod-connections",
      GlobalSecondaryIndexes: Match.arrayWith([
        {
          IndexName: "by_space",
          KeySchema: [{ AttributeName: "space", KeyType: "HASH" }],
          Projection: { ProjectionType: "ALL" },
        },
        {
          IndexName: "by_member",
          KeySchema: [{ AttributeName: "member", KeyType: "HASH" }],
          Projection: {
            ProjectionType: "INCLUDE",
            NonKeyAttributes: ["expires_at"],
          },
        },
      ]),
    });
  });
});

describe("the bucket", () => {
  test("blocks all public access, is encrypted, keeps no versions and needs TLS", () => {
    template.resourceCountIs("AWS::S3::Bucket", 1);
    template.hasResourceProperties("AWS::S3::Bucket", {
      PublicAccessBlockConfiguration: {
        BlockPublicAcls: true,
        BlockPublicPolicy: true,
        IgnorePublicAcls: true,
        RestrictPublicBuckets: true,
      },
      BucketEncryption: {
        ServerSideEncryptionConfiguration: [
          {
            ServerSideEncryptionByDefault: { SSEAlgorithm: "AES256" },
          },
        ],
      },
      VersioningConfiguration: Match.absent(),
    });
    template.hasResourceProperties("AWS::S3::BucketPolicy", {
      PolicyDocument: {
        Statement: Match.arrayWith([
          Match.objectLike({
            Effect: "Deny",
            Condition: { Bool: { "aws:SecureTransport": "false" } },
          }),
        ]),
      },
    });
  });

  test("deletes uploads after a day", () => {
    template.hasResourceProperties("AWS::S3::Bucket", {
      LifecycleConfiguration: {
        Rules: [
          Match.objectLike({
            Prefix: "uploads/",
            ExpirationInDays: 1,
            Status: "Enabled",
          }),
        ],
      },
    });
  });

  test("lets any origin PUT and GET through presigned URLs (NOC-245 A)", () => {
    template.hasResourceProperties("AWS::S3::Bucket", {
      CorsConfiguration: {
        CorsRules: [
          Match.objectLike({
            AllowedOrigins: ["*"],
            AllowedMethods: ["GET", "PUT"],
          }),
        ],
      },
    });
  });
});

describe("the function and the API", () => {
  test("Rust on arm64 with 256 MB, its settings from the environment", () => {
    template.hasResourceProperties("AWS::Lambda::Function", {
      Runtime: "provided.al2023",
      Architectures: ["arm64"],
      MemorySize: 256,
      Handler: "bootstrap",
      Environment: {
        Variables: {
          NCDB_RELAY_ID: "5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e",
          NCDB_ALLOWED_ACCOUNTS:
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
          NCDB_TABLE_PREFIX: "ncdb-prod-",
          NCDB_BUCKET: Match.anyValue(),
          NCDB_CONNECTIONS_URL: Match.anyValue(),
        },
      },
    });
  });

  test("a WebSocket API with the three routes and the stage prod", () => {
    template.hasResourceProperties("AWS::ApiGatewayV2::Api", {
      ProtocolType: "WEBSOCKET",
    });
    for (const route of ["$connect", "$disconnect", "$default"]) {
      template.hasResourceProperties("AWS::ApiGatewayV2::Route", {
        RouteKey: route,
      });
    }
    template.resourceCountIs("AWS::ApiGatewayV2::Route", 3);
    template.hasResourceProperties("AWS::ApiGatewayV2::Stage", {
      StageName: "prod",
      AutoDeploy: true,
    });
  });

  test("the function's role may post to connections and use its tables and bucket, nothing wider", () => {
    const policies = template.findResources("AWS::IAM::Policy");
    const statements = Object.values(policies).flatMap(
      (p) => p.Properties.PolicyDocument.Statement as { Action: unknown }[],
    );
    const actions = statements.flatMap((s) =>
      Array.isArray(s.Action) ? s.Action : [s.Action],
    );
    expect(actions).toContain("execute-api:ManageConnections");
    expect(actions.some((a) => String(a).startsWith("dynamodb:"))).toBe(true);
    expect(actions.some((a) => String(a).startsWith("s3:"))).toBe(true);
    expect(actions).not.toContain("*");
    expect(actions.some((a) => String(a).startsWith("iam:"))).toBe(false);
  });

  test('its logs are kept 14 days (RFC 0007, "What the relay sees")', () => {
    template.hasResourceProperties("AWS::Logs::LogGroup", {
      RetentionInDays: 14,
    });
  });
});

describe("the daily orphan sweep (NOC-290)", () => {
  // The function runs the sweep only for exactly this input
  // (ncdb_relay_lambda::invocation::SWEEP_INPUT).
  const source = readFileSync(
    join(
      import.meta.dirname,
      "../../../crates/ncdb-relay-lambda/src/invocation.rs",
    ),
    "utf8",
  );
  const input = /pub const SWEEP_INPUT: &str = r#"(.*)"#;/.exec(source)?.[1];

  test("a rule invokes the function once a day with the sweep's input", () => {
    expect(input).toBe('{"ncdb-relay":"sweep-orphans"}');
    template.resourceCountIs("AWS::Events::Rule", 1);
    const rules = template.findResources("AWS::Events::Rule");
    const [rule] = Object.values(rules);
    expect(rule?.Properties.ScheduleExpression).toBe("cron(17 3 * * ? *)");
    expect(rule?.Properties.State).toBe("ENABLED");
    expect(rule?.Properties.Targets).toHaveLength(1);
    const [target] = rule?.Properties.Targets ?? [];
    const [relay] = Object.keys(
      template.findResources("AWS::Lambda::Function"),
    );
    expect(target.Arn).toEqual({ "Fn::GetAtt": [relay, "Arn"] });
    expect(JSON.parse(target.Input)).toEqual(JSON.parse(input ?? ""));
  });

  test("its log lines count each run's start and end", () => {
    template.resourceCountIs("AWS::Logs::MetricFilter", 2);
    const [logs] = Object.keys(template.findResources("AWS::Logs::LogGroup"));
    for (const [phase, metric] of [
      ["start", "SweepStarted"],
      ["end", "SweepEnded"],
    ]) {
      template.hasResourceProperties("AWS::Logs::MetricFilter", {
        LogGroupName: { Ref: logs },
        FilterPattern: `{ ($.sweep = "orphans") && ($.phase = "${phase}") }`,
        MetricTransformations: [
          {
            MetricNamespace: "NoCentralDb/Relay",
            MetricName: metric,
            MetricValue: "1",
          },
        ],
      });
    }
  });

  test("a run that started and never ended raises an alarm that mails the owner", () => {
    const [topic] = Object.keys(template.findResources("AWS::SNS::Topic"));
    template.hasResourceProperties("AWS::CloudWatch::Alarm", {
      ComparisonOperator: "GreaterThanOrEqualToThreshold",
      Threshold: 1,
      EvaluationPeriods: 1,
      TreatMissingData: "notBreaching",
      AlarmActions: [{ Ref: topic }],
      Metrics: Match.arrayWith([
        Match.objectLike({
          Expression: "FILL(started, 0) - FILL(ended, 0)",
          ReturnData: true,
        }),
        Match.objectLike({
          Id: "started",
          MetricStat: Match.objectLike({
            Metric: {
              MetricName: "SweepStarted",
              Namespace: "NoCentralDb/Relay",
            },
            Period: 3600,
            Stat: "Sum",
          }),
        }),
        Match.objectLike({
          Id: "ended",
          MetricStat: Match.objectLike({
            Metric: {
              MetricName: "SweepEnded",
              Namespace: "NoCentralDb/Relay",
            },
            Period: 3600,
            Stat: "Sum",
          }),
        }),
      ]),
    });
  });

  test("only that rule may invoke the function from EventBridge", () => {
    const [ruleId] = Object.keys(template.findResources("AWS::Events::Rule"));
    const permissions = Object.values(
      template.findResources("AWS::Lambda::Permission"),
    ).filter((p) => p.Properties.Principal === "events.amazonaws.com");
    expect(permissions).toHaveLength(1);
    expect(permissions[0]?.Properties.Action).toBe("lambda:InvokeFunction");
    expect(permissions[0]?.Properties.SourceArn).toEqual({
      "Fn::GetAtt": [ruleId, "Arn"],
    });
  });
});

describe("alarms and the budget", () => {
  test("a budget of 20 USD a month mails at 50, 80 and 100 percent (NOC-264 point 5)", () => {
    template.hasResourceProperties("AWS::Budgets::Budget", {
      Budget: {
        BudgetType: "COST",
        TimeUnit: "MONTHLY",
        BudgetLimit: { Amount: 20, Unit: "USD" },
      },
      NotificationsWithSubscribers: [50, 80, 100].map((threshold) => ({
        Notification: {
          NotificationType: "ACTUAL",
          ComparisonOperator: "GREATER_THAN",
          Threshold: threshold,
          ThresholdType: "PERCENTAGE",
        },
        Subscribers: [
          { SubscriptionType: "EMAIL", Address: "owner@example.invalid" },
        ],
      })),
    });
  });

  test("the function's errors and throttles raise an alarm that mails the owner", () => {
    template.resourceCountIs("AWS::CloudWatch::Alarm", 3);
    template.hasResourceProperties("AWS::SNS::Subscription", {
      Protocol: "email",
      Endpoint: "owner@example.invalid",
    });
  });
});

test("the app sets every feature flag aws-cdk-lib recommends, as the owner decided", () => {
  // Carsten, 2026-11-13, on #293: set before the first deploy, so no flag
  // changes a deployed resource later. The tests synthesize with the same
  // context as bin/relay.ts, which cdk reads from cdk.json.
  expect(flags()).toMatchObject(CURRENTLY_RECOMMENDED_FLAGS);
});

test("the template matches its committed snapshot", () => {
  const json = template.toJSON() as Record<string, unknown>;
  // The asset's hash depends on the stand-in package, not the stack.
  const text = JSON.stringify(json, null, 2).replace(
    /[0-9a-f]{64}\.zip/g,
    "<asset>.zip",
  );
  expect(text).toMatchSnapshot();
});
