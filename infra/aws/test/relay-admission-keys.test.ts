// The relay's admission keys (RFC 0015, "Admission tokens"; NOC-469): the
// public half of each app's admission key, 64 lowercase hex digits, becomes
// the function's NCDB_ADMISSION_KEYS, comma-separated. A relay given none
// sets no such variable, so a stack synthesized before keeps its template.

import { App, Stack } from "aws-cdk-lib";
import { Template } from "aws-cdk-lib/assertions";
import { describe, expect, test } from "vitest";
import { Relay } from "../src/relay.ts";
import { relayEnvironment } from "../src/relay-settings.ts";
import { flags, lambdaPackage } from "./support.ts";

const relayId = "5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e";
const account = "a".repeat(64);
const first = "c".repeat(64);
const second = "d".repeat(64);

function variables(admissionKeys?: readonly string[]) {
  const stack = new Stack(new App({ context: flags() }), "AppStack");
  new Relay(stack, "Relay", {
    stage: "beta",
    settings: { relayId, allowedAccounts: [account], admissionKeys },
    lambdaPackage: lambdaPackage(),
  });
  const [fn] = Object.values(
    Template.fromStack(stack).findResources("AWS::Lambda::Function"),
  );
  return fn?.Properties.Environment.Variables as Record<string, unknown>;
}

describe("the admission keys", () => {
  // Synthesized once, at collection, as the other infra tests do: a stack's
  // first synthesis takes seconds, more than a test's own time limit.
  const given = variables([first, second]);
  const none = variables();

  test("become NCDB_ADMISSION_KEYS, comma-separated", () => {
    expect(given).toMatchObject({
      NCDB_ADMISSION_KEYS: `${first},${second}`,
      NCDB_ALLOWED_ACCOUNTS: account,
    });
  });

  test("set no variable when the relay takes none", () => {
    expect(none).not.toHaveProperty("NCDB_ADMISSION_KEYS");
    expect(none).toMatchObject({ NCDB_RELAY_ID: relayId });
    expect(
      relayEnvironment({ relayId, allowedAccounts: [], admissionKeys: [] }),
    ).not.toHaveProperty("NCDB_ADMISSION_KEYS");
  });

  test("refuse a key that is not 64 lowercase hex digits", () => {
    for (const bad of ["abc", "C".repeat(64), `${first},${second}`]) {
      expect(() =>
        relayEnvironment({
          relayId,
          allowedAccounts: [],
          admissionKeys: [bad],
        }),
      ).toThrow("an admission key is 64 lowercase hex digits");
    }
  });
});
