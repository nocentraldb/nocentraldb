// Thin wrappers around the `git` command for the release tooling. Every path
// list is read with -z and split on NUL, so a path is never C-quoted or split
// on a space or newline. A failing git command throws, with git's stderr in
// the message; nothing here turns a git error into an empty answer, except
// describeNearest, for which "no matching tag" is a legitimate answer.
import { execFileSync } from "node:child_process";

export function git(args, { cwd, env } = {}) {
  try {
    return execFileSync("git", args, {
      cwd,
      env,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
  } catch (error) {
    const wrapped = new Error(
      `git ${args.join(" ")} failed: ${String(error.stderr ?? error.message).trim()}`,
    );
    wrapped.status = error.status;
    wrapped.stderr = error.stderr;
    throw wrapped;
  }
}

/** Splits the output of a `-z` command into its entries. */
export const zList = (out) => out.split("\0").filter(Boolean);

export const mergeBase = (a, b, opts) => git(["merge-base", a, b], opts).trim();

/**
 * The paths a pull request from `base` to `head` changes: diffed from their
 * merge base, so commits on the base branch after the fork do not count.
 * Renames are split into a deletion and an addition (--no-renames), so both
 * the old and the new path are listed. `filter` is git's --diff-filter.
 */
export function changedPaths(base, head, { filter = "ACMRD", ...opts } = {}) {
  const from = mergeBase(base, head, opts);
  return zList(
    git(
      [
        "diff",
        "-z",
        "--name-only",
        "--no-renames",
        `--diff-filter=${filter}`,
        from,
        head,
      ],
      opts,
    ),
  );
}

/**
 * The paths under `pathspec` that exist at `to` and did not at `from`. A
 * rename is detected (--find-renames) and is not an addition.
 */
export const addedPaths = (from, to, pathspec, opts) =>
  zList(
    git(
      [
        "diff",
        "-z",
        "--name-only",
        "--find-renames",
        "--diff-filter=A",
        from,
        to,
        "--",
        pathspec,
      ],
      opts,
    ),
  );

export const tags = (pattern, opts) =>
  git(["tag", "--list", pattern], opts).split("\n").filter(Boolean);

/**
 * The nearest tag matching `match` reachable from `ref`; null when there is
 * none. The ref is verified first, because git describe answers "No names
 * found" for a ref that does not exist as well.
 */
export function describeNearest(match, ref, opts) {
  git(["rev-parse", "--verify", "--quiet", `${ref}^{commit}`], opts);
  try {
    return git(
      ["describe", "--tags", "--abbrev=0", `--match=${match}`, ref],
      opts,
    ).trim();
  } catch (error) {
    if (
      error.status === 128 &&
      /No names found|No tags can describe/.test(String(error.stderr))
    ) {
      return null;
    }
    throw error;
  }
}
