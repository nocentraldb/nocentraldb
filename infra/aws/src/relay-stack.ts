// The relay on AWS, as RFC 0007, "The AWS backend", describes it: one
// WebSocket API in front of one Lambda function, which runs the relay core
// on DynamoDB and S3, in eu-central-1, in the account NoCentralDb-Prod.
//
// - The WebSocket API: the routes `$connect`, `$disconnect` and `$default`,
//   one stage.
// - The function: Rust on arm64 with 256 MB, the binary of
//   crates/ncdb-relay-lambda, built by cargo-lambda into `bootstrap`. Its
//   settings are `NCDB_*` variables (ncdb_relay_lambda::settings), and its
//   logs are kept 14 days.
// - The tables, on demand, with point-in-time recovery for 35 days:
//   - RFC 0007's `log`, `changes`, `spaces`, `members` and `person-keys`;
//   - NOC-270's `uploads` (no `tokens`: behind API Gateway the URLs of
//     large changes are presigned S3 URLs; NOC-295, NOC-303);
//   - `connections`, with its TTL and indexes.
//   Their keys are those the backend creates in tests (ncdb_relay_aws::schema,
//   ncdb_relay_lambda::dynamo).
// - The bucket: all public access blocked, S3-managed encryption, no
//   versions, TLS only, uploads deleted after a day, and CORS for any origin
//   (NOC-245 A).
// - A rule that runs the orphan sweep every hour in bounded runs, an alarm
//   when a run starts and never ends (NOC-290), and one when no pass
//   completed in two days (NOC-293).
// - Alarms on the function's errors and throttles, and a budget of 20 USD a
//   month with mails at 50, 80 and 100 percent (NOC-264 point 5).

import {
  CfnOutput,
  Duration,
  RemovalPolicy,
  Stack,
  type StackProps,
} from "aws-cdk-lib";
import { WebSocketApi, WebSocketStage } from "aws-cdk-lib/aws-apigatewayv2";
import { WebSocketLambdaIntegration } from "aws-cdk-lib/aws-apigatewayv2-integrations";
import { CfnBudget } from "aws-cdk-lib/aws-budgets";
import {
  Alarm,
  ComparisonOperator,
  MathExpression,
  TreatMissingData,
} from "aws-cdk-lib/aws-cloudwatch";
import { SnsAction } from "aws-cdk-lib/aws-cloudwatch-actions";
import {
  type Attribute,
  AttributeType,
  ProjectionType,
  TableV2,
} from "aws-cdk-lib/aws-dynamodb";
import { Rule, RuleTargetInput, Schedule } from "aws-cdk-lib/aws-events";
import { LambdaFunction as LambdaTarget } from "aws-cdk-lib/aws-events-targets";
import {
  Architecture,
  Code,
  Function as LambdaFunction,
  Runtime,
} from "aws-cdk-lib/aws-lambda";
import {
  FilterPattern,
  LogGroup,
  MetricFilter,
  RetentionDays,
} from "aws-cdk-lib/aws-logs";
import {
  BlockPublicAccess,
  Bucket,
  BucketEncryption,
  HttpMethods,
} from "aws-cdk-lib/aws-s3";
import { Topic } from "aws-cdk-lib/aws-sns";
import { EmailSubscription } from "aws-cdk-lib/aws-sns-subscriptions";
import type { Construct } from "constructs";

/// What a deployment of the relay sets.
export interface RelayProps extends StackProps {
  /// The stage, which names the tables and the API stage: `prod`.
  readonly stage: string;
  /// The relay's stable id, 32 hex digits, drawn once per deployment.
  readonly relayId: string;
  /// The account keys whose members may create spaces, 64 hex digits each.
  readonly allowedAccounts: readonly string[];
  /// Where the budget and the alarms send their mails.
  readonly alarmEmail: string;
  /// The directory of the function's built package, which holds `bootstrap`.
  readonly lambdaPackage: string;
}

const binary = (name: string): Attribute => ({
  name,
  type: AttributeType.BINARY,
});

export class RelayStack extends Stack {
  constructor(scope: Construct, id: string, props: RelayProps) {
    super(scope, id, props);
    if (!/^[0-9a-f]{32}$/.test(props.relayId)) {
      throw new Error("relayId is 32 lowercase hex digits");
    }
    for (const key of props.allowedAccounts) {
      if (!/^[0-9a-f]{64}$/.test(key)) {
        throw new Error("an allowed account is 64 lowercase hex digits");
      }
    }
    const prefix = `ncdb-${props.stage}-`;

    // Every table: on demand, point-in-time recovery for 35 days (RFC 0007,
    // "Restores and the relay epoch"), and kept when the stack goes.
    const table = (
      name: string,
      partitionKey: Attribute,
      sortKey?: Attribute,
      timeToLiveAttribute?: string,
    ): TableV2 =>
      new TableV2(this, `Table-${name}`, {
        tableName: `${prefix}${name}`,
        partitionKey,
        ...(sortKey ? { sortKey } : {}),
        ...(timeToLiveAttribute ? { timeToLiveAttribute } : {}),
        pointInTimeRecoverySpecification: {
          pointInTimeRecoveryEnabled: true,
          recoveryPeriodInDays: 35,
        },
        removalPolicy: RemovalPolicy.RETAIN,
      });

    const tables = [
      table("log", binary("space"), {
        name: "position",
        type: AttributeType.NUMBER,
      }),
      table("changes", binary("space"), binary("change")),
      table("spaces", binary("space")),
      table("members", binary("space"), binary("member")),
      table("person-keys", binary("space"), binary("id")),
      table("uploads", binary("id"), undefined, "ttl"),
    ];
    const members = tables[3];
    members?.addGlobalSecondaryIndex({
      indexName: "by_member",
      partitionKey: binary("member"),
      sortKey: binary("space"),
      projectionType: ProjectionType.INCLUDE,
      nonKeyAttributes: ["current"],
    });
    const connections = table(
      "connections",
      { name: "connection", type: AttributeType.STRING },
      { name: "item", type: AttributeType.STRING },
      "ttl",
    );
    connections.addGlobalSecondaryIndex({
      indexName: "by_space",
      partitionKey: binary("space"),
      projectionType: ProjectionType.ALL,
    });
    connections.addGlobalSecondaryIndex({
      indexName: "by_member",
      partitionKey: binary("member"),
      projectionType: ProjectionType.INCLUDE,
      nonKeyAttributes: ["expires_at"],
    });
    tables.push(connections);

    const bucket = new Bucket(this, "Frames", {
      blockPublicAccess: BlockPublicAccess.BLOCK_ALL,
      encryption: BucketEncryption.S3_MANAGED,
      enforceSSL: true,
      versioned: false,
      removalPolicy: RemovalPolicy.RETAIN,
      lifecycleRules: [{ prefix: "uploads/", expiration: Duration.days(1) }],
      // A browser uses the presigned URLs directly (NOC-245 A): PUT to
      // uploads/, GET from frames/. A CORS rule cannot name a prefix; the
      // presigned signature decides what each URL may do.
      cors: [
        {
          allowedOrigins: ["*"],
          allowedMethods: [HttpMethods.GET, HttpMethods.PUT],
          allowedHeaders: ["*"],
          maxAge: 600,
        },
      ],
    });

    const logs = new LogGroup(this, "Logs", {
      retention: RetentionDays.TWO_WEEKS,
      removalPolicy: RemovalPolicy.DESTROY,
    });

    const api = new WebSocketApi(this, "Api", {
      apiName: `ncdb-${props.stage}-relay`,
    });
    const stage = new WebSocketStage(this, "Stage", {
      webSocketApi: api,
      stageName: props.stage,
      autoDeploy: true,
    });

    const relay = new LambdaFunction(this, "Relay", {
      runtime: Runtime.PROVIDED_AL2023,
      architecture: Architecture.ARM_64,
      memorySize: 256,
      timeout: Duration.seconds(29),
      handler: "bootstrap",
      code: Code.fromAsset(props.lambdaPackage),
      logGroup: logs,
      environment: {
        NCDB_RELAY_ID: props.relayId,
        NCDB_ALLOWED_ACCOUNTS: props.allowedAccounts.join(","),
        NCDB_TABLE_PREFIX: prefix,
        NCDB_BUCKET: bucket.bucketName,
        NCDB_CONNECTIONS_URL: stage.callbackUrl,
      },
    });
    for (const t of tables) {
      t.grantReadWriteData(relay);
    }
    bucket.grantReadWrite(relay);
    api.grantManageConnections(relay);

    const integration = new WebSocketLambdaIntegration("Relay", relay);
    api.addRoute("$connect", { integration });
    api.addRoute("$disconnect", { integration });
    api.addRoute("$default", { integration });

    // The orphan sweep (NOC-290; RFC 0007, "How large changes go to S3"):
    // the function runs it for exactly this input
    // (ncdb_relay_lambda::invocation::SWEEP_INPUT). It runs every hour, in
    // bounded runs that continue where the last stopped, and starts a new
    // pass at most once a day (NOC-293), so a pass finishes however many
    // frames the bucket holds. A failed sweep fails the invocation, which
    // the error alarm reports.
    new Rule(this, "SweepOrphans", {
      schedule: Schedule.cron({ minute: "17" }),
      targets: [
        new LambdaTarget(relay, {
          event: RuleTargetInput.fromObject({ "ncdb-relay": "sweep-orphans" }),
          retryAttempts: 2,
        }),
      ],
    });

    const owner = new Topic(this, "Alarms");
    owner.addSubscription(new EmailSubscription(props.alarmEmail));
    const alarm = (
      id: string,
      metric: ReturnType<LambdaFunction["metricErrors"]> | MathExpression,
    ) =>
      new Alarm(this, id, {
        metric,
        threshold: 1,
        evaluationPeriods: 1,
        comparisonOperator:
          ComparisonOperator.GREATER_THAN_OR_EQUAL_TO_THRESHOLD,
        treatMissingData: TreatMissingData.NOT_BREACHING,
      }).addAlarmAction(new SnsAction(owner));
    alarm("Errors", relay.metricErrors({ period: Duration.minutes(5) }));
    alarm("Throttles", relay.metricThrottles({ period: Duration.minutes(5) }));

    // A sweep logs its start before it runs and its end after; a run that
    // Lambda's timeout cut off, or that crashed, has a start without an
    // end. The alarm fires within the hour, before orphans pile up.
    const sweepPhase = (phase: string, metricName: string) =>
      new MetricFilter(this, metricName, {
        logGroup: logs,
        filterPattern: FilterPattern.all(
          FilterPattern.stringValue("$.sweep", "=", "orphans"),
          FilterPattern.stringValue("$.phase", "=", phase),
        ),
        metricNamespace: "NoCentralDb/Relay",
        metricName,
        metricValue: "1",
      }).metric({ period: Duration.hours(1), statistic: "Sum" });
    alarm(
      "SweepUnfinished",
      new MathExpression({
        expression: "FILL(started, 0) - FILL(ended, 0)",
        usingMetrics: {
          started: sweepPhase("start", "SweepStarted"),
          ended: sweepPhase("end", "SweepEnded"),
        },
        period: Duration.hours(1),
      }),
    );

    // A run that reaches the end of the frames logs its pass as completed.
    // Two days without a completed pass raise an alarm: a pass that takes
    // longer than a day, or a sweep that stopped running (NOC-293).
    const passes = new MetricFilter(this, "SweepPassCompleted", {
      logGroup: logs,
      filterPattern: FilterPattern.all(
        FilterPattern.stringValue("$.sweep", "=", "orphans"),
        FilterPattern.stringValue("$.phase", "=", "end"),
        FilterPattern.stringValue("$.pass", "=", "completed"),
      ),
      metricNamespace: "NoCentralDb/Relay",
      metricName: "SweepPassCompleted",
      metricValue: "1",
    }).metric({ period: Duration.days(1), statistic: "Sum" });
    new Alarm(this, "SweepPassOverdue", {
      metric: passes,
      threshold: 1,
      evaluationPeriods: 2,
      datapointsToAlarm: 2,
      comparisonOperator: ComparisonOperator.LESS_THAN_THRESHOLD,
      treatMissingData: TreatMissingData.BREACHING,
    }).addAlarmAction(new SnsAction(owner));

    new CfnBudget(this, "Budget", {
      budget: {
        budgetName: `ncdb-${props.stage}-relay`,
        budgetType: "COST",
        timeUnit: "MONTHLY",
        budgetLimit: { amount: 20, unit: "USD" },
      },
      notificationsWithSubscribers: [50, 80, 100].map((threshold) => ({
        notification: {
          notificationType: "ACTUAL",
          comparisonOperator: "GREATER_THAN",
          threshold,
          thresholdType: "PERCENTAGE",
        },
        subscribers: [{ subscriptionType: "EMAIL", address: props.alarmEmail }],
      })),
    });

    new CfnOutput(this, "Url", {
      value: stage.url,
      description: "The relay's URL, which an app opens its database with",
    });
  }
}
