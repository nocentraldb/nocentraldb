#!/usr/bin/env node
// Computes the next alpha of the commit <head> (docs/release-management.md,
// "Versioning" and "Alpha"); the alpha job of .github/workflows/release.yml
// runs it and tags what it prints. It commits and tags nothing itself. Every
// file is read as it stands at <head> (git show), so the answer depends on
// that commit and the repository's tags only, never on the working tree.
// Rules:
//
// N1 The last release is the highest entry with stage "release" in the
//    index file (release-notes/index.json) when it exists; never the
//    existence of a vX.Y.Z tag, because a refused release tag stays in
//    place. Without one there is no last release.
// N2 The previous alpha is the nearest tag v*-alpha.* reachable from <head>
//    (git describe), across targets; a tag on another branch is not one.
// N3 The fragments of the alpha are the changes/<slug>.md added since the
//    previous alpha, or every fragment present when there is none. When none
//    of them has a type other than internal, the alpha is skipped (exit 0,
//    "skip": true, the reason names the previous alpha).
// N4 The target is the higher of the workspace version (Cargo.toml
//    [workspace.package], a plain X.Y.Z) and the last release bumped for the
//    types of ALL present fragments; without a last release it is the
//    workspace version.
// N5 N is one above the highest N among the tags v<target>-alpha.N, on any
//    branch: alphas are never in the index, and a gap after a failed run is
//    harmless.
// N6 Every fragment present at <head> must be valid; the workspace version
//    must exist and be plain; the index must parse and hold versions. Any of
//    these failing is exit 1 naming the file, never a guess.
//
// Exit 0 with the answer (also when skipping), 1 with one line per
// violation, 2 on a usage error.
import { parseFragment, validateFragment } from "./lib/fragments.mjs";
import { addedPaths, describeNearest, git, tags, zList } from "./lib/git.mjs";
import { readWorkspaceVersion } from "./lib/manifests.mjs";
import {
  bumpFor,
  classifyTag,
  compare,
  PLAIN,
  parseVersion,
} from "./lib/semver.mjs";

const USAGE = `usage: next-version [--json] [--head <ref>] [--index <file>]
Prints the next alpha tag of <ref> (default HEAD), or "skip: <reason>" when
no fragment other than internal was added since the previous alpha. With
--json prints the whole answer as JSON. <file> is the release index as it
stands at <ref> (default release-notes/index.json); the last release is read
from it, never from tags.`;

const fail = (message) => {
  console.error(`next-version: ${message}`);
  process.exitCode = 1;
};

const usageError = () => {
  console.error(USAGE);
  process.exit(2);
};

const parseArgs = (args) => {
  const opts = {
    json: false,
    head: "HEAD",
    index: "release-notes/index.json",
  };
  for (let i = 0; i < args.length; i++) {
    const value = () => (args[i + 1] ? args[++i] : usageError());
    if (args[i] === "--help") {
      console.log(USAGE);
      process.exit(0);
    } else if (args[i] === "--json") opts.json = true;
    else if (args[i] === "--head") opts.head = value();
    else if (args[i] === "--index") opts.index = value();
    else usageError();
  }
  return opts;
};

/** A file as it stands at `head`, or null when it does not exist there. */
const readAt = (head, path) =>
  git(["ls-tree", "--name-only", head, "--", path]).trim() === ""
    ? null
    : git(["show", `${head}:${path}`]);

const isFragment = (path) =>
  /^changes\/[^/]+\.md$/.test(path) && path !== "changes/README.md";
const slugOf = (path) => path.slice("changes/".length, -".md".length);

/** Every fragment at `head`, sorted, with its type; null after reporting an invalid one. */
const presentFragments = (head) => {
  const paths = zList(git(["ls-tree", "-z", "--name-only", head, "changes/"]))
    .filter(isFragment)
    .sort();
  const fragments = [];
  for (const path of paths) {
    const text = readAt(head, path);
    const errors = validateFragment(slugOf(path), text);
    for (const error of errors) fail(`${path}: ${error}`);
    if (errors.length === 0)
      fragments.push({ path, type: parseFragment(text).front.type });
  }
  return process.exitCode ? null : fragments;
};

/** The plain workspace version at `head`; null after reporting the problem. */
const workspaceVersion = (head) => {
  const text = readAt(head, "Cargo.toml");
  if (text === null) {
    fail(`Cargo.toml: missing at ${head}`);
    return null;
  }
  try {
    const version = readWorkspaceVersion(text);
    if (PLAIN.test(version)) return version;
    fail(
      `Cargo.toml: [workspace.package] version "${version}" is not X.Y.Z: committed manifests never carry a stage suffix`,
    );
  } catch (error) {
    fail(`Cargo.toml: ${error.message}`);
  }
  return null;
};

/** The highest release version of the index at `head`; undefined after reporting a problem. */
const lastRelease = (head, file) => {
  const text = readAt(head, file);
  if (text === null) return null;
  try {
    const { releases } = JSON.parse(text);
    if (!Array.isArray(releases)) throw new Error('has no "releases" list');
    let best = null;
    for (const { version, stage } of releases) {
      if (stage !== "release") continue;
      if (typeof version !== "string" || !PLAIN.test(version))
        throw new Error(
          `release entry version ${JSON.stringify(version)} is not X.Y.Z`,
        );
      if (
        best === null ||
        compare(parseVersion(version), parseVersion(best)) > 0
      )
        best = version;
    }
    return best;
  } catch (error) {
    fail(`${file}: ${error.message}`);
    return undefined;
  }
};

const higher = (a, b) =>
  compare(parseVersion(a), parseVersion(b)) >= 0 ? a : b;

const nextNumber = (target) =>
  1 +
  Math.max(
    0,
    ...tags(`v${target}-alpha.*`)
      .map(classifyTag)
      .filter((t) => t !== null)
      .map((t) => t.n),
  );

const compute = ({ head, index }) => {
  git(["rev-parse", "--verify", "--quiet", `${head}^{commit}`]);
  const present = presentFragments(head);
  const workspace = workspaceVersion(head);
  const released = lastRelease(head, index);
  if (present === null || workspace === null || released === undefined)
    return null;

  const previousAlpha = describeNearest("v*-alpha.*", head);
  const added =
    previousAlpha === null
      ? present
      : addedPaths(previousAlpha, head, "changes/")
          .filter(isFragment)
          .sort()
          .map((path) => present.find((f) => f.path === path));
  if (!added.some((f) => f.type !== "internal")) {
    return {
      skip: true,
      reason: `no fragment other than internal since ${previousAlpha ?? "the start of the history"}`,
      previousAlpha,
      fragments: [],
    };
  }

  const target =
    released === null
      ? workspace
      : higher(
          bumpFor(
            released,
            present.map((f) => f.type),
          ),
          workspace,
        );
  const n = nextNumber(target);
  const version = `${target}-alpha.${n}`;
  return {
    skip: false,
    target,
    n,
    version,
    tag: `v${version}`,
    previousAlpha,
    lastRelease: released,
    fragments: added.map((f) => f.path),
  };
};

const main = () => {
  const opts = parseArgs(process.argv.slice(2));
  let answer;
  try {
    answer = compute(opts);
  } catch (error) {
    return fail(error.message);
  }
  if (answer === null) return;
  if (opts.json) console.log(JSON.stringify(answer, null, 2));
  else console.log(answer.skip ? `skip: ${answer.reason}` : answer.tag);
};

main();
