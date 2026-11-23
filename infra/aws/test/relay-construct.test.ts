// The `Relay` construct in a stack of an app's own (NOC-417): the same
// relay as `RelayStack`, with its settings as the function's `NCDB_*`
// variables, and its WebSocket API open for the app's own `$connect`
// authorizer. The project's budget is not part of it, and its alarms come
// only when asked for. The Cognito example of docs/relay-deploy.md is
// synthesized here, so the guide shows code that works.

import { App, Stack } from "aws-cdk-lib";
import { Match, Template } from "aws-cdk-lib/assertions";
import { describe, expect, test } from "vitest";
import { CognitoProtectedRelay } from "../examples/cognito.ts";
import { Relay } from "../src/relay.ts";
import { flags, lambdaPackage } from "./support.ts";

const relayId = "5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e";
const account =
  "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

function appStack(): Stack {
  return new Stack(new App({ context: flags() }), "AppStack");
}

function relayIn(
  stack: Stack,
  settings = { relayId, allowedAccounts: [account] },
) {
  return new Relay(stack, "Relay", {
    stage: "beta",
    settings,
    lambdaPackage: lambdaPackage(),
  });
}

describe("in a stack of an app's own", () => {
  const stack = appStack();
  const relay = relayIn(stack);
  const template = Template.fromStack(stack);

  test("it holds the relay: seven tables, the bucket, the function, three routes and the sweep", () => {
    template.resourceCountIs("AWS::DynamoDB::GlobalTable", 7);
    template.hasResourceProperties("AWS::DynamoDB::GlobalTable", {
      TableName: "ncdb-beta-log",
    });
    template.resourceCountIs("AWS::S3::Bucket", 1);
    template.resourceCountIs("AWS::Lambda::Function", 1);
    template.resourceCountIs("AWS::ApiGatewayV2::Route", 3);
    template.hasResourceProperties("AWS::ApiGatewayV2::Stage", {
      StageName: "beta",
    });
    template.resourceCountIs("AWS::Events::Rule", 1);
  });

  test("its settings become the function's NCDB_* variables", () => {
    template.hasResourceProperties("AWS::Lambda::Function", {
      Environment: {
        Variables: {
          NCDB_RELAY_ID: relayId,
          NCDB_ALLOWED_ACCOUNTS: account,
          NCDB_TABLE_PREFIX: "ncdb-beta-",
          NCDB_BUCKET: Match.anyValue(),
          NCDB_CONNECTIONS_URL: Match.anyValue(),
        },
      },
    });
  });

  test("it adds no budget, no alarm and no topic of its own", () => {
    template.resourceCountIs("AWS::Budgets::Budget", 0);
    template.resourceCountIs("AWS::CloudWatch::Alarm", 0);
    template.resourceCountIs("AWS::SNS::Topic", 0);
  });

  test("it exposes the API, its stage, the $connect route, the function and the URL", () => {
    expect(relay.api.apiId).toBeDefined();
    expect(relay.stage.stageName).toBe("beta");
    expect(relay.connectRoute.routeKey).toBe("$connect");
    expect(relay.handler.functionName).toBeDefined();
    expect(relay.url).toBe(relay.stage.url);
  });
});

describe("its settings", () => {
  test("refuse a relay id that is not 32 lowercase hex digits", () => {
    expect(() =>
      relayIn(appStack(), { relayId: "XYZ", allowedAccounts: [account] }),
    ).toThrow("relayId is 32 lowercase hex digits");
  });

  test("refuse an allowed account that is not 64 lowercase hex digits", () => {
    expect(() =>
      relayIn(appStack(), { relayId, allowedAccounts: ["abc"] }),
    ).toThrow("an allowed account is 64 lowercase hex digits");
  });
});

describe("its alarms, when asked for", () => {
  test("the four alarms notify the topic it returns", () => {
    const stack = appStack();
    const topic = relayIn(stack).addAlarms();
    const template = Template.fromStack(stack);
    template.resourceCountIs("AWS::CloudWatch::Alarm", 4);
    template.resourceCountIs("AWS::SNS::Topic", 1);
    const topicId = stack.getLogicalId(
      topic.node.defaultChild as import("aws-cdk-lib").CfnElement,
    );
    for (const alarm of Object.values(
      template.findResources("AWS::CloudWatch::Alarm"),
    )) {
      expect(alarm.Properties.AlarmActions).toEqual([{ Ref: topicId }]);
    }
  });
});

describe("the Cognito example of docs/relay-deploy.md", () => {
  const app = new App({ context: flags() });
  const stack = new CognitoProtectedRelay(app, "CognitoExample", {
    relayId,
    allowedAccounts: [account],
    userPoolId: "eu-central-1_EXAMPLE",
    userPoolClientId: "exampleclientid",
    lambdaPackage: lambdaPackage(),
    authorizerPackage: lambdaPackage(),
  });
  const template = Template.fromStack(stack);

  test("puts a Lambda authorizer on the relay's own $connect route, which reads the token from the query string", () => {
    template.resourceCountIs("AWS::ApiGatewayV2::Route", 3);
    template.hasResourceProperties("AWS::ApiGatewayV2::Authorizer", {
      AuthorizerType: "REQUEST",
      IdentitySource: ["route.request.querystring.token"],
    });
    const [authorizer] = Object.keys(
      template.findResources("AWS::ApiGatewayV2::Authorizer"),
    );
    template.hasResourceProperties("AWS::ApiGatewayV2::Route", {
      RouteKey: "$connect",
      AuthorizationType: "CUSTOM",
      AuthorizerId: { Ref: authorizer },
    });
    for (const route of ["$disconnect", "$default"]) {
      template.hasResourceProperties("AWS::ApiGatewayV2::Route", {
        RouteKey: route,
        AuthorizationType: "NONE",
      });
    }
  });

  test("the authorizer knows the user pool and its client, and its logs are kept 14 days", () => {
    template.hasResourceProperties("AWS::Lambda::Function", {
      Environment: {
        Variables: {
          USER_POOL_ID: "eu-central-1_EXAMPLE",
          USER_POOL_CLIENT_ID: "exampleclientid",
        },
      },
    });
    template.resourceCountIs("AWS::Logs::LogGroup", 2);
    template.allResourcesProperties("AWS::Logs::LogGroup", {
      RetentionInDays: 14,
    });
  });

  test("API Gateway may invoke the authorizer", () => {
    template.hasResourceProperties("AWS::Lambda::Permission", {
      Action: "lambda:InvokeFunction",
      Principal: "apigateway.amazonaws.com",
    });
  });
});
