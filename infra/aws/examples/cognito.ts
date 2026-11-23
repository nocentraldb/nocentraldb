// The example of docs/relay-deploy.md, "Protect the endpoint": an app's
// stack with the relay, whose `$connect` route only lets a connection in
// that carries a valid access token of the app's Cognito user pool. The
// token travels in the relay URL's query string, `?token=…`, because a
// browser's WebSocket cannot send an Authorization header. The authorizer's
// code is the app's (the guide shows one); test/relay-construct.test.ts
// synthesizes this stack.

import { Duration, Stack, type StackProps } from "aws-cdk-lib";
import type { CfnRoute } from "aws-cdk-lib/aws-apigatewayv2";
import { WebSocketLambdaAuthorizer } from "aws-cdk-lib/aws-apigatewayv2-authorizers";
import {
  Architecture,
  Code,
  Function as LambdaFunction,
  Runtime,
} from "aws-cdk-lib/aws-lambda";
import { LogGroup, RetentionDays } from "aws-cdk-lib/aws-logs";
import type { Construct } from "constructs";
import { Relay } from "../src/relay.ts";

export interface CognitoProtectedRelayProps extends StackProps {
  readonly relayId: string;
  readonly allowedAccounts: readonly string[];
  /// The user pool whose access tokens open a connection.
  readonly userPoolId: string;
  /// The app client the tokens must be issued to.
  readonly userPoolClientId: string;
  /// The relay's function, built with cargo-lambda.
  readonly lambdaPackage: string;
  /// The authorizer's code, built by the app.
  readonly authorizerPackage: string;
}

export class CognitoProtectedRelay extends Stack {
  constructor(scope: Construct, id: string, props: CognitoProtectedRelayProps) {
    super(scope, id, props);

    const relay = new Relay(this, "Relay", {
      stage: "prod",
      settings: {
        relayId: props.relayId,
        allowedAccounts: props.allowedAccounts,
      },
      lambdaPackage: props.lambdaPackage,
    });

    // The authorizer checks the token and logs nothing of the request.
    const authorize = new LambdaFunction(this, "Authorize", {
      runtime: Runtime.NODEJS_22_X,
      architecture: Architecture.ARM_64,
      handler: "index.handler",
      code: Code.fromAsset(props.authorizerPackage),
      timeout: Duration.seconds(5),
      logGroup: new LogGroup(this, "AuthorizeLogs", {
        retention: RetentionDays.TWO_WEEKS,
      }),
      environment: {
        USER_POOL_ID: props.userPoolId,
        USER_POOL_CLIENT_ID: props.userPoolClientId,
      },
    });

    // Plain CDK: bind the authorizer to the relay's own `$connect` route.
    // API Gateway refuses a connection without `token` before it reaches
    // either function.
    const authorizer = new WebSocketLambdaAuthorizer("Cognito", authorize, {
      identitySource: ["route.request.querystring.token"],
    });
    const { authorizerId, authorizationType } = authorizer.bind({
      route: relay.connectRoute,
      scope: this,
    });
    const connect = relay.connectRoute.node.defaultChild as CfnRoute;
    connect.authorizationType = authorizationType;
    connect.authorizerId = authorizerId;
  }
}
