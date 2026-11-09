// Installs the dependencies of a cloud session's checkout at session start
// (NOC-65).
//
// A cloud session's container is restored from a snapshot taken when the
// environment was built; its node_modules holds what `pnpm install` installed
// then. A dependency added to pnpm-lock.yaml on main since is missing, and
// the tests that import it fail. This script runs after session-start-git.mjs
// has put the checkout on HEAD. When pnpm-lock.yaml differs from the lockfile
// node_modules was last installed from, it runs
// `pnpm install --frozen-lockfile`. It records the lockfile's hash in
// node_modules/.session-start-deps after a successful install; no record (a
// fresh snapshot) counts as a difference. It runs only in cloud sessions
// (CLAUDE_CODE_REMOTE=true) and always exits 0: a failed install is reported
// and tried again at the next session start.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";

const NAME = "session-start-deps";
const LOCKFILE = "pnpm-lock.yaml";
const RECORD = "node_modules/.session-start-deps";

if (process.env.CLAUDE_CODE_REMOTE !== "true") process.exit(0);
if (!existsSync(LOCKFILE)) process.exit(0);

const hash = createHash("sha256").update(readFileSync(LOCKFILE)).digest("hex");
const installed = existsSync(RECORD) ? readFileSync(RECORD, "utf8").trim() : "";
if (installed === hash) process.exit(0);

const run = spawnSync("pnpm", ["install", "--frozen-lockfile"], {
  encoding: "utf8",
});
if (run.status !== 0) {
  const output = `${run.stdout ?? ""}${run.stderr ?? ""}`.trim();
  const reason = run.error ? run.error.message : `exit code ${run.status}`;
  console.log(
    `${NAME}: \`pnpm install --frozen-lockfile\` failed (${reason}); node_modules may miss dependencies. Run \`pnpm install\` once if a test fails with a missing module.`,
  );
  if (output) console.log(output.split("\n").slice(-20).join("\n"));
  process.exit(0);
}

mkdirSync("node_modules", { recursive: true });
writeFileSync(RECORD, `${hash}\n`);
console.log(
  `${NAME}: ${LOCKFILE} differed from the lockfile node_modules was installed from; ran \`pnpm install --frozen-lockfile\`.`,
);
