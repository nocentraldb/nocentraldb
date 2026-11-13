import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

/// A directory standing in for the function's built package.
export function lambdaPackage(): string {
  const dir = mkdtempSync(join(tmpdir(), "ncdb-relay-lambda-"));
  writeFileSync(join(dir, "bootstrap"), "not a real binary\n");
  return dir;
}

/// The app's context from cdk.json, as `cdk synth` hands it to bin/relay.ts:
/// the feature flags the stack is synthesized with.
export function flags(): Record<string, unknown> {
  const cdk = JSON.parse(
    readFileSync(join(import.meta.dirname, "../cdk.json"), "utf8"),
  ) as { context?: Record<string, unknown> };
  return cdk.context ?? {};
}
