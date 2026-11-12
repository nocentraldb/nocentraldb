// The relay's stack against RFC 0007, "The AWS backend": its tables and
// keys, the bucket, the function, the WebSocket API, the logs and the
// budget alarm. It synthesizes with no AWS credentials. The template is
// also kept as a snapshot, so a pull request shows how it changes the
// deployed infrastructure without credentials (NOC-264, NOC-272).

import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { App } from "aws-cdk-lib";
import { Match, Template } from "aws-cdk-lib/assertions";
import { describe, expect, test } from "vitest";
import { RelayStack } from "../src/relay-stack.ts";

/// A directory standing in for the function's built package.
function lambdaPackage(): string {
  const dir = mkdtempSync(join(tmpdir(), "ncdb-relay-lambda-"));
  writeFileSync(join(dir, "bootstrap"), "not a real binary\n");
  return dir;
}

function synth(): Template {
  const app = new App();
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

/// RFC 0007's tables ("Tables and keys"), NOC-270's `uploads` and
/// `tokens`, and the `connections` table of the Lambda crate.
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
  tokens: { keys: [["token", "B", "HASH"]], ttl: "ttl" },
  connections: {
    keys: [
      ["connection", "S", "HASH"],
      ["item", "S", "RANGE"],
    ],
    ttl: "ttl",
  },
};

describe("the tables", () => {
  test("there are exactly the eight tables", () => {
    template.resourceCountIs("AWS::DynamoDB::GlobalTable", 8);
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
    template.resourceCountIs("AWS::CloudWatch::Alarm", 2);
    template.hasResourceProperties("AWS::SNS::Subscription", {
      Protocol: "email",
      Endpoint: "owner@example.invalid",
    });
  });
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
