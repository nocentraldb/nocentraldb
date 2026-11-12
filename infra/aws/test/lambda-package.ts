import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

/// A directory standing in for the function's built package.
export function lambdaPackage(): string {
  const dir = mkdtempSync(join(tmpdir(), "ncdb-relay-lambda-"));
  writeFileSync(join(dir, "bootstrap"), "not a real binary\n");
  return dir;
}
