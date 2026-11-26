// What the relay keeps, and for how long (RFC 0007, "What the relay sees";
// NOC-532). Connection records go when the connection closes, unless the
// operator keeps them for a number of days: `keepConnectionRecordsDays`
// becomes the function's NCDB_KEEP_CONNECTIONS_DAYS. The function's logs
// are kept 14 days, or the longer period `logRetentionDays` names.

import { App, Stack } from "aws-cdk-lib";
import { Template } from "aws-cdk-lib/assertions";
import { describe, expect, test } from "vitest";
import { Relay } from "../src/relay.ts";
import { type RelaySettings, relayEnvironment } from "../src/relay-settings.ts";
import { flags, lambdaPackage } from "./support.ts";

const relayId = "5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e";
const base: RelaySettings = { relayId, allowedAccounts: [] };

function synthesized(settings: Partial<RelaySettings>) {
  const stack = new Stack(new App({ context: flags() }), "AppStack");
  new Relay(stack, "Relay", {
    stage: "beta",
    settings: { ...base, ...settings },
    lambdaPackage: lambdaPackage(),
  });
  const template = Template.fromStack(stack);
  const [fn] = Object.values(template.findResources("AWS::Lambda::Function"));
  const [logs] = Object.values(template.findResources("AWS::Logs::LogGroup"));
  return {
    variables: fn?.Properties.Environment.Variables as Record<string, unknown>,
    retention: logs?.Properties.RetentionInDays as unknown,
  };
}

describe("the relay's retention", () => {
  // Synthesized once, at collection, as the other infra tests do: a stack's
  // first synthesis takes seconds, more than a test's own time limit.
  const defaults = synthesized({});
  const longer = synthesized({
    keepConnectionRecordsDays: 30,
    logRetentionDays: 90,
  });

  test("by default, connection records go on close and logs stay 14 days", () => {
    expect(defaults.variables).not.toHaveProperty(
      "NCDB_KEEP_CONNECTIONS_DAYS",
    );
    expect(defaults.retention).toBe(14);
  });

  test("an operator keeps connection records and logs for the days it sets", () => {
    expect(longer.variables).toMatchObject({
      NCDB_KEEP_CONNECTIONS_DAYS: "30",
    });
    expect(longer.retention).toBe(90);
  });

  test("records are kept for a whole number of days, at least one", () => {
    for (const bad of [0, -1, 1.5, Number.NaN]) {
      expect(() =>
        relayEnvironment({ ...base, keepConnectionRecordsDays: bad }),
      ).toThrow("keepConnectionRecordsDays is a whole number of days, at least 1");
    }
  });

  test("logs are kept no shorter than 14 days, for a period CloudWatch offers", () => {
    for (const bad of [1, 7, 13, 15, 100, 9999]) {
      expect(() => synthesized({ logRetentionDays: bad })).toThrow(
        "logRetentionDays is 14 or a longer period CloudWatch Logs offers",
      );
    }
  });
});
