// Which workflows run in GitHub's merge queue (NOC-511). The queue sends
// merge_group for each queued pull request and merges once the required
// checks, `gates` (ci.yml) and `changes` (changes.yml), pass on the queue's
// commit. Those two run on merge_group, and nothing else does: a release,
// deploy or pages workflow must never run on a commit that is not on main
// yet. The alpha release follows ci through workflow_run, limited to main, so
// a ci run in the queue, on a gh-readonly-queue/main/... branch, starts no
// release either.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { parse } from "yaml";

const dir = join(
  dirname(fileURLToPath(import.meta.url)),
  "../.github/workflows",
);
const workflows = Object.fromEntries(
  readdirSync(dir)
    .filter((file) => file.endsWith(".yml"))
    .map((file) => [file, parse(readFileSync(join(dir, file), "utf8"))]),
);

test("only ci.yml and changes.yml run in the merge queue", () => {
  const queued = Object.entries(workflows)
    .filter(([, workflow]) => "merge_group" in (workflow.on ?? {}))
    .map(([file]) => file)
    .sort();
  assert.deepEqual(queued, ["changes.yml", "ci.yml"]);
});

test("the alpha release follows ci only on main, never on a queue branch", () => {
  const { workflow_run } = workflows["release.yml"].on;
  assert.deepEqual(workflow_run.branches, ["main"]);
  assert.deepEqual(workflow_run.workflows, ["ci"]);
});
