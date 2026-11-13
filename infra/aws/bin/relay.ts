// The CDK app of the relay: one stack, in eu-central-1, in the account the
// deploy's credentials name (NoCentralDb-Prod). Its values come as context,
// `cdk synth -c relayId=... -c allowedAccounts=... -c alarmEmail=...`, which
// the deploy workflow passes from the GitHub environment `aws-prod`; none
// of them is a secret, and none is kept in the repository. The function's
// package is built by `cargo lambda build --release --arm64 -p
// ncdb-relay-lambda --features aws` into target/lambda/ncdb-relay-lambda.

import { App } from "aws-cdk-lib";
import { RelayStack } from "../src/relay-stack.ts";

const app = new App();

function context(name: string): string {
  const value = app.node.tryGetContext(name);
  if (typeof value !== "string" || value.trim() === "") {
    throw new Error(
      `the context value ${name} is missing (cdk synth -c ${name}=...)`,
    );
  }
  return value.trim();
}

const stage = app.node.tryGetContext("stage") ?? "prod";

new RelayStack(app, `NoCentralDbRelay-${stage}`, {
  stage,
  relayId: context("relayId"),
  allowedAccounts: context("allowedAccounts")
    .split(",")
    .map((key) => key.trim())
    .filter((key) => key !== ""),
  alarmEmail: context("alarmEmail"),
  lambdaPackage:
    app.node.tryGetContext("lambdaPackage") ??
    "../../target/lambda/ncdb-relay-lambda",
  env: {
    region: "eu-central-1",
    ...(process.env.CDK_DEFAULT_ACCOUNT
      ? { account: process.env.CDK_DEFAULT_ACCOUNT }
      : {}),
  },
});
