#!/usr/bin/env node
// Cuts a release branch (docs/release-management.md, "Branches" and "Beta"):
// the PM or an agent runs it after the release-notes pull request merged.
// Rules:
//
// C1 <X.Y.Z> is a plain version; anything else is a usage error.
// C2 Refused before anything changes: a detached HEAD (there would be no
//    branch to return to) and a working tree that is not clean, untracked
//    files included.
// C3 X.Y.0, the first cut of a minor: release/X.Y must exist neither
//    locally nor on origin. It is created at --from (default HEAD).
// C4 X.Y.Z with Z > 0, a patch: release/X.Y must exist locally, equal
//    origin's when origin has it, and --from, when given, must be its head.
// C5 On release/X.Y, set-version.mjs writes X.Y.Z. When that changed
//    anything, one commit "Set the version to X.Y.Z on release/X.Y";
//    otherwise none (the first cut of a minor: main already says X.Y.0).
// C6 --push pushes release/X.Y to origin; otherwise the push command is
//    printed: the owner pushes, or the PM on the owner's word.
// C7 cut always switches back to the branch it started from, also when a
//    step after the switch failed; a failure first resets the release
//    branch's working tree, which was clean before the cut. So nothing that
//    follows the cut, such as a NEXUS_MEMORY.md commit, lands on a release
//    branch.
// C8 Then it prints the next steps: for X.Y.0 the pull request that raises
//    main to X.(Y+1).0 with the skip-changes label, and the owner's beta tag.
//
// Test-only seam: CUT_NO_LOCK=1 passes --no-lock to set-version, so the
// tests need no cargo.
//
// Exit 0 on success, 1 with `cut: <message>` naming the branch or the
// version, 2 on a usage error.
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { cli } from "./lib/cli.mjs";
import { git } from "./lib/git.mjs";
import { PLAIN, parseVersion } from "./lib/semver.mjs";

const USAGE = `usage: cut <X.Y.Z> [--from <commit>] [--push]
Cuts release/X.Y and sets its manifests to <X.Y.Z>, committing only when
they change. For X.Y.0 the branch is created at <commit> (default HEAD) and
must not exist yet; for a patch X.Y.Z it must exist and is used at its head.
Pushes the branch to origin with --push, otherwise prints the command. Always
returns to the branch it started from, and prints the next steps.`;

const { fail, usageError, parseArgs } = cli("cut", USAGE);

const setVersion = join(
  dirname(fileURLToPath(import.meta.url)),
  "set-version.mjs",
);

class Refused extends Error {}
const refuse = (message) => {
  throw new Refused(message);
};

const options = (args) => {
  const [version, ...rest] = args;
  if (version === "--help") parseArgs([version]);
  if (!PLAIN.test(version ?? "")) usageError();
  return {
    ...parseArgs(rest, { flags: ["--push"], values: ["--from"] }),
    version,
  };
};

const commitOf = (ref) => {
  try {
    return git(["rev-parse", "--verify", "--quiet", `${ref}^{commit}`]).trim();
  } catch {
    return null;
  }
};

const currentBranch = () => {
  try {
    return git(["symbolic-ref", "--quiet", "--short", "HEAD"]).trim();
  } catch {
    return refuse(
      "HEAD is detached; check out the branch cut should return to first",
    );
  }
};

/** The commit of release/X.Y on origin, or null when origin has none. */
const onOrigin = (branch) =>
  git(["ls-remote", "--heads", "origin", `refs/heads/${branch}`]).split(
    "\t",
  )[0] || null;

/** Checks C2-C4 and returns the commit the branch starts at. */
const prepare = ({ version, from }, branch) => {
  if (git(["status", "--porcelain"]).trim() !== "")
    refuse("the working tree is not clean; commit or stash first");
  const local = commitOf(`refs/heads/${branch}`);
  const remote = onOrigin(branch);
  if (parseVersion(version).patch === 0) {
    if (local)
      refuse(
        `${branch} already exists locally; a patch of it is X.Y.Z with Z > 0`,
      );
    if (remote)
      refuse(
        `${branch} already exists on origin; a patch of it is X.Y.Z with Z > 0`,
      );
    const start = commitOf(from ?? "HEAD");
    if (!start) refuse(`--from ${from} is not a commit`);
    git(["branch", branch, start]);
    return { start, created: true };
  }
  if (!local)
    refuse(
      `${branch} does not exist locally; fetch it with git fetch origin ${branch}:${branch}`,
    );
  if (remote && remote !== local)
    refuse(
      `${branch} differs from origin/${branch}; bring the local branch up to date first`,
    );
  if (from !== undefined && commitOf(from) !== local)
    refuse(`a patch is cut on the head of ${branch}, not on ${from}`);
  return { start: local, created: false };
};

/** C5 and C6, on the release branch; returns what happened. */
const onBranch = ({ version, push }, branch) => {
  const args = [setVersion, version];
  if (process.env.CUT_NO_LOCK === "1") args.push("--no-lock");
  const r = spawnSync(process.execPath, args, { encoding: "utf8" });
  if (r.status !== 0) {
    const reason = String(r.stderr || r.error?.message || "").trim();
    throw new Error(
      `node ${args.join(" ")} failed: ${reason.split("\n").at(-1)}`,
    );
  }
  let committed = false;
  if (git(["status", "--porcelain"]).trim() !== "") {
    git(["add", "-A"]);
    git(["commit", "-q", "-m", `Set the version to ${version} on ${branch}`]);
    committed = true;
  }
  if (push) git(["push", "-q", "-u", "origin", branch]);
  return committed;
};

const nextSteps = ({ version, push }, branch) => {
  const v = parseVersion(version);
  const raise = `${v.major}.${v.minor + 1}.0`;
  const lines = ["", "Next steps:"];
  lines.push(
    push
      ? `1. ${branch} is pushed to origin.`
      : "1. Push the branch (the owner, or the PM on the owner's word):",
  );
  if (!push) lines.push(`  git push -u origin ${branch}`);
  if (v.patch === 0) {
    lines.push(
      `2. Raise main to ${raise} in a pull request with the skip-changes label:`,
      `  git switch main && git pull --ff-only && node scripts/release/set-version.mjs ${raise} && git switch -c release-${v.major}.${v.minor}-raise && git commit -am "Raise main to ${raise} after cutting ${branch}" && gh pr create --label skip-changes --title "Raise main to ${raise}" --body "Sets the manifests on main to ${raise} after ${branch} was cut, so the alphas on main sort above the release line. Nothing a user gets changes."`,
    );
  } else {
    lines.push("2. main needs no raise for a patch release.");
  }
  lines.push(
    "3. The owner tags the beta on the branch head:",
    `  git tag v${version}-beta.1 ${branch} && git push origin refs/tags/v${version}-beta.1`,
  );
  console.log(lines.join("\n"));
};

const main = () => {
  const opts = options(process.argv.slice(2));
  const { major, minor } = parseVersion(opts.version);
  const branch = `release/${major}.${minor}`;
  let original;
  let switched = false;
  let created = false;
  try {
    original = currentBranch();
    const prepared = prepare(opts, branch);
    created = prepared.created;
    git(["switch", "-q", branch]);
    switched = true;
    const committed = onBranch(opts, branch);
    console.log(
      `cut: ${branch} ${created ? "created" : "used"} at ${prepared.start.slice(0, 12)}`,
    );
    console.log(
      committed
        ? `cut: committed "Set the version to ${opts.version} on ${branch}"`
        : `cut: manifests already at ${opts.version}, no commit`,
    );
  } catch (error) {
    if (switched) {
      git(["reset", "-q", "--hard"]);
      git(["clean", "-q", "-fd"]);
    }
    fail(error.message);
    if (created)
      fail(
        `${branch} was created locally; delete it with git branch -D ${branch} before you try again`,
      );
  } finally {
    if (switched) git(["switch", "-q", original]);
  }
  if (process.exitCode !== 1) nextSteps(opts, branch);
};

main();
