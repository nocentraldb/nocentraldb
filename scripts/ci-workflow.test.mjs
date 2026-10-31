// Asserts the invariants of .github/workflows/ci.yml that the design review
// made binding: the full uncached gate, the supply-chain rules, the cache
// scope and bounds, the job timeout and the concurrency policy.
// Text checks on the committed workflow, no YAML parser needed.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (file) => readFileSync(join(root, file), "utf8");

const workflow = read(".github/workflows/ci.yml");
const lines = workflow.split("\n");
/** The workflow without comments, so prose in a comment never satisfies or trips a check. */
const code = lines
  .filter((line) => !/^\s*#/.test(line))
  .map((line) => line.replace(/\s+#\s.*$/, ""))
  .join("\n");

test("runs on pull_request and on push to main, never pull_request_target", () => {
  assert.match(
    code,
    /^on:\n {2}pull_request:\n {2}push:\n {4}branches: \[main\]$/m,
  );
  assert.doesNotMatch(workflow, /pull_request_target/);
});

test("grants contents: read and nothing else", () => {
  assert.match(code, /^permissions:\n {2}contents: read\n(?! )/m);
  assert.equal(code.match(/permissions:/g).length, 1);
});

test("pins every action by full commit SHA with its version as a comment", () => {
  const uses = lines.filter((line) => /^\s*(- )?uses:/.test(line));
  assert.ok(uses.length > 0, "the workflow uses no action");
  for (const line of uses) {
    assert.match(
      line,
      /uses: [\w.-]+\/[\w.-]+@[0-9a-f]{40} # v\d+\.\d+\.\d+$/,
      line,
    );
  }
});

test("uses no secrets", () => {
  assert.doesNotMatch(code, /secrets\./);
  assert.doesNotMatch(code, /GITHUB_TOKEN/);
});

test("installs from the frozen lockfile and runs the uncached project gate", () => {
  assert.match(code, /run: pnpm install --frozen-lockfile$/m);
  assert.match(code, /run: pnpm gates$/m);
  assert.doesNotMatch(code, /moon ci|gates:dev|moon run/);
});

test("states the checkout depth decision and keeps the default depth", () => {
  assert.doesNotMatch(code, /fetch-depth/);
  assert.ok(
    lines.some((line) => /^\s*#.*depth/i.test(line)),
    "no comment states the checkout depth decision",
  );
});

test("never caches or restores moon's task cache", () => {
  assert.doesNotMatch(code, /\.moon/);
});

test("keys the Cargo cache on rust-toolchain.toml and Cargo.lock", () => {
  const key = code.match(/^\s*key: cargo-.*$/m)?.[0] ?? "";
  assert.match(key, /hashFiles\([^)]*'rust-toolchain\.toml'/);
  assert.match(key, /hashFiles\([^)]*'Cargo\.lock'/);
});

test("keys the pnpm store cache on .prototools and pnpm-lock.yaml", () => {
  const key = code.match(/^\s*key: pnpm-.*$/m)?.[0] ?? "";
  assert.match(key, /hashFiles\([^)]*'\.prototools'/);
  assert.match(key, /hashFiles\([^)]*'pnpm-lock\.yaml'/);
});

test("pnpm build scripts stay an explicit allow-list", () => {
  const workspace = read("pnpm-workspace.yaml");
  assert.match(workspace, /^onlyBuiltDependencies:/m);
  assert.doesNotMatch(workspace, /dangerouslyAllowAllBuilds/);
});

// `moon exec` queries changed files on every run, even with --force. In a
// GitHub pull request it takes the base from GITHUB_BASE_REF (`main`), which a
// shallow checkout does not contain, and git fails before any task runs.
// Pinning base and head to HEAD keeps that query resolvable in any checkout.
test("pnpm gates forces every task and diffs HEAD against itself", () => {
  const gates = JSON.parse(read("package.json")).scripts.gates;
  const args = gates.split(/\s+/);
  assert.deepEqual(args.slice(0, 3), ["moon", "exec", ":gates"]);
  assert.ok(args.includes("--force"), gates);
  assert.match(gates, /--base HEAD --head HEAD/);
  assert.doesNotMatch(gates, /--affected|--query/);
});

// A hung step must not burn the free-plan Actions minutes until GitHub's
// 6-hour default job timeout.
test("bounds the gates job with timeout-minutes", () => {
  const job = code.match(/^ {2}gates:\n((?: {4}.*\n|\n)*)/m)?.[1] ?? "";
  const minutes = job.match(/^ {4}timeout-minutes: (\d+)$/m)?.[1];
  assert.ok(minutes, "the gates job declares no timeout-minutes");
  assert.ok(Number(minutes) >= 10 && Number(minutes) <= 60, minutes);
});

// Superseded pull-request runs are cancelled; every push to main runs to the
// end. A shared group would still cancel a *pending* main run when the next
// push queues behind it, so push runs get a group of their own per commit.
test("cancels superseded pull-request runs only, never a run on main", () => {
  const block = code.match(/^concurrency:\n((?: {2}.*\n)+)/m)?.[1] ?? "";
  assert.match(
    block,
    /^ {2}cancel-in-progress: \$\{\{ github\.event_name == 'pull_request' \}\}$/m,
  );
  const group = block.match(/^ {2}group: (.*)$/m)?.[1] ?? "";
  assert.match(group, /github\.event_name == 'pull_request'/, group);
  assert.match(group, /github\.sha/, group);
});

// actions/cache saves only on a key miss, so one entry holds at most one
// build per toolchain and lockfile. A restore-keys fallback would seed every
// new entry with the previous target/ and let it grow across entries.
test("bounds the Cargo target cache: exact key, no restore-keys fallback", () => {
  const steps = code.split(/\n(?= {6}- )/);
  const cargo = steps.filter((step) => /^\s*target$/m.test(step));
  assert.equal(cargo.length, 1, "exactly one step caches target/");
  assert.match(cargo[0], /uses: actions\/cache@/);
  assert.doesNotMatch(cargo[0], /restore-keys/);
});
