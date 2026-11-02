#!/usr/bin/env node
// The release workflow's commit to main after a published beta or release
// (docs/release-management.md, "Release"). Run in a checkout of main, from
// the repository root; the job `commit-back` of .github/workflows/release.yml
// runs it, and a maintainer runs it by hand when that job failed. Rules:
//
// C0 It starts only from a clean working tree whose HEAD is <main>: anything
//    staged, modified or untracked, or a local commit not on <main>, is
//    exit 1 with "nothing committed", and nothing is touched. So the commit
//    holds only what this script writes, and the push carries only it.
// C1 Both stages first add the entry to release-notes/index.json (`index
//    add`). It refuses a tag that is not v<version> for a release or
//    v<version>-beta.N for a beta, a stage other than those two and a
//    version that is not X.Y.Z, before anything is written; an identical
//    entry already indexed changes nothing, a differing one fails.
// C2 A release also archives release-notes/<version>.md as it stands at
//    <tag> (git show <tag>:<path>), and removes the fragments it shipped
//    (`shipped-fragments --tag <tag> --main <main> --delete`: a fragment on
//    main whose content differs from the shipped one is kept, and named).
// C3 Before anything is committed, the index passes validateIndex and every
//    fragment left in changes/ passes `check-fragments`; a failure there, or
//    in any step before, is exit 1 with "nothing committed", and the working
//    tree is reset to HEAD (it was clean, C0).
// C4 Nothing staged is "nothing to commit", exit 0: a re-run after a
//    successful one makes no commit and pushes nothing.
// C5 Otherwise one commit, "Release <tag>: index, notes archive and shipped
//    fragments" (beta: "Beta <tag>: index"), with only those paths, pushed
//    to the branch of <main> on its remote (default origin/main), unless
//    --no-push.
// C6 A failed push is retried once: fetch, rebase onto <main>, C3 again,
//    push. A second failure, a failed rebase (aborted) or a failed C3 is
//    exit 1 naming the runbook section that redoes the commit by hand.
//
// Exit 0 when the commit is pushed (or made, with --no-push) or there was
// nothing to commit, 1 with `commit-back: <message>`, 2 on a usage error.
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { validateIndex } from "./index.mjs";
import { cli } from "./lib/cli.mjs";
import { git } from "./lib/git.mjs";

const USAGE = `usage: commit-back --tag <tag> --stage <beta|release> --version <X.Y.Z> --date <YYYY-MM-DD> [--main <remote/branch>] [--no-push]
Commits the published beta or release <tag> back to main, in a checkout of
main: the release-notes/index.json entry; for a release also
release-notes/<X.Y.Z>.md as it stands at <tag> and the removal of the
fragments it shipped. Checks the index and the fragments first, commits only
when something changed, and pushes to <main> (default origin/main), retrying
once after a rebase.`;

const RUNBOOK = "docs/release-management.md#if-the-commit-to-main-failed";
const INDEX = "release-notes/index.json";

const { fail, usageError, parseArgs } = cli("commit-back", USAGE);

const ARGS = {
  flags: ["--no-push"],
  values: ["--tag", "--stage", "--version", "--date", "--main"],
  defaults: { main: "origin/main", noPush: false },
};

const here = dirname(fileURLToPath(import.meta.url));

/** Runs a sibling release script with inherited stdio; throws when it fails. */
const sibling = (name, ...args) => {
  try {
    execFileSync(process.execPath, [join(here, name), ...args], {
      stdio: "inherit",
    });
  } catch {
    throw new Error(`${name} ${args.join(" ")} failed`);
  }
};

/** C3: the index has its shape and every fragment left passes the schema. */
const checkTree = () => {
  let index;
  try {
    index = JSON.parse(readFileSync(INDEX, "utf8"));
  } catch (error) {
    throw new Error(`${INDEX}: ${error.message}`);
  }
  const errors = validateIndex(index);
  if (errors.length > 0) throw new Error(`${INDEX}: ${errors.join("; ")}`);
  sibling("check-fragments.mjs");
};

/** C1, C2: writes and stages the commit's content. */
const prepare = ({ tag, stage, version, date, main }) => {
  sibling(
    "index.mjs",
    "add",
    "--version",
    version,
    "--stage",
    stage,
    "--tag",
    tag,
    "--date",
    date,
  );
  git(["add", "--", INDEX]);
  if (stage === "release") {
    const notes = `release-notes/${version}.md`;
    writeFileSync(notes, git(["show", `${tag}:${notes}`]));
    git(["add", "--", notes]);
    sibling("shipped-fragments.mjs", "--tag", tag, "--main", main, "--delete");
  }
};

/** C0: a clean tree on <main>; throws otherwise. */
const checkStart = (main) => {
  const dirty = git(["status", "--porcelain"])
    .split("\n")
    .filter(Boolean)
    .map((line) => line.slice(3));
  if (dirty.length > 0)
    throw new Error(`the working tree is not clean (${dirty.join(", ")})`);
  const head = git(["rev-parse", "HEAD"]).trim();
  let base;
  try {
    base = git(["rev-parse", "--verify", `${main}^{commit}`]).trim();
  } catch {
    throw new Error(`--main ${main} is not a commit`);
  }
  if (head !== base) throw new Error(`HEAD ${head} is not ${main} ${base}`);
};

/** Undoes what prepare wrote; safe because the tree was clean (C0). */
const restore = () => {
  git(["reset", "-q", "--hard", "HEAD"]);
  git(["clean", "-fdq", "--", "release-notes", "changes"]);
};

const nothingStaged = () => {
  try {
    git(["diff", "--cached", "--quiet"]);
    return true;
  } catch (error) {
    if (error.status === 1) return false;
    throw error;
  }
};

/** C5, C6: pushes HEAD to the branch of `main`, retrying once after a rebase. */
const push = (main) => {
  const slash = main.indexOf("/");
  if (slash <= 0) throw new Error(`--main ${main} is not <remote>/<branch>`);
  const remote = main.slice(0, slash);
  const target = `HEAD:refs/heads/${main.slice(slash + 1)}`;
  const tryPush = () => {
    try {
      execFileSync("git", ["push", remote, target], { stdio: "inherit" });
      return true;
    } catch {
      return false;
    }
  };
  if (tryPush()) return;
  // Rejected because main moved, or refused for another reason (rights, a
  // protection rule, the network): git said which above. One retry either
  // way; a refusal fails again and ends in the runbook.
  console.log(
    `commit-back: the push to ${main} failed; fetching, rebasing onto it and retrying once`,
  );
  try {
    git(["fetch", remote]);
    git(["rebase", main]);
  } catch (error) {
    console.error(error.message);
    try {
      git(["rebase", "--abort"]);
    } catch {}
    throw new Error(
      `the rebase onto ${main} failed and was aborted; the commit is not on ${main}, make it by hand: ${RUNBOOK}`,
    );
  }
  try {
    checkTree();
  } catch (error) {
    throw new Error(
      `${error.message} after the rebase onto ${main}; the commit is not pushed, make it by hand: ${RUNBOOK}`,
    );
  }
  if (!tryPush())
    throw new Error(
      `the push to ${main} failed again after the rebase; make the commit by hand: ${RUNBOOK}`,
    );
};

const main = () => {
  const opts = parseArgs(process.argv.slice(2), ARGS);
  for (const key of ["tag", "stage", "version", "date"])
    if (opts[key] === undefined) usageError();
  try {
    checkStart(opts.main);
  } catch (error) {
    return fail(`${error.message}; nothing committed`);
  }
  try {
    prepare(opts);
    checkTree();
  } catch (error) {
    restore();
    return fail(`${error.message}; nothing committed`);
  }
  if (nothingStaged()) return console.log("commit-back: nothing to commit");
  const message =
    opts.stage === "release"
      ? `Release ${opts.tag}: index, notes archive and shipped fragments`
      : `Beta ${opts.tag}: index`;
  git(["commit", "-q", "-m", message, "--", "release-notes", "changes"]);
  console.log(`commit-back: committed "${message}"`);
  if (opts.noPush) return;
  try {
    push(opts.main);
  } catch (error) {
    return fail(error.message);
  }
  console.log(`commit-back: pushed to ${opts.main}`);
};

main();
