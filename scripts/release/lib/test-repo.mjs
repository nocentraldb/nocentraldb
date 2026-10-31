// Small, isolated git repositories for the tests of the release tooling.
// GIT_ENV cuts git off from the user's and the system's configuration
// (GIT_CONFIG_GLOBAL=/dev/null, GIT_CONFIG_NOSYSTEM=1, HOME in the temporary
// directory) and fixes the identity and the dates, so a commit's SHA does not
// depend on the machine or the clock. Signing is switched off per repository.
// Spawn every script under test with `env: GIT_ENV` for the same reasons.
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

const DATE = "2026-01-01T00:00:00Z";

export const GIT_ENV = {
  ...process.env,
  GIT_CONFIG_GLOBAL: "/dev/null",
  GIT_CONFIG_NOSYSTEM: "1",
  GIT_AUTHOR_NAME: "Test",
  GIT_AUTHOR_EMAIL: "test@example.invalid",
  GIT_AUTHOR_DATE: DATE,
  GIT_COMMITTER_NAME: "Test",
  GIT_COMMITTER_EMAIL: "test@example.invalid",
  GIT_COMMITTER_DATE: DATE,
  HOME: tmpdir(),
};

/**
 * A fresh repository on branch `main` in a temporary directory. `commit`
 * writes the given files (path -> text), stages everything and returns the
 * new commit's SHA; `dispose` removes the directory.
 */
export function makeRepo() {
  const dir = mkdtempSync(join(tmpdir(), "release-repo-"));
  const run = (...args) =>
    execFileSync("git", args, {
      cwd: dir,
      env: GIT_ENV,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }).trim();
  const write = (file, text) => {
    mkdirSync(join(dir, dirname(file)), { recursive: true });
    writeFileSync(join(dir, file), text);
  };
  run("init", "-q", "-b", "main");
  run("config", "commit.gpgsign", "false");
  run("config", "tag.gpgsign", "false");
  return {
    dir,
    run,
    write,
    commit(message, files = {}) {
      for (const [file, text] of Object.entries(files)) write(file, text);
      run("add", "-A");
      run("commit", "-q", "--allow-empty", "-m", message);
      return run("rev-parse", "HEAD");
    },
    remove(file) {
      rmSync(join(dir, file));
    },
    tag(name, ref = "HEAD") {
      run("tag", name, ref);
    },
    dispose() {
      rmSync(dir, { recursive: true, force: true });
    },
  };
}
