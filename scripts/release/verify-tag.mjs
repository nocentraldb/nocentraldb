#!/usr/bin/env node
// The refusal rules of the publish job (docs/release-management.md, "Beta"
// and "Release"): whether a pushed beta or release tag may be published.
// .github/workflows/release.yml runs it on the tagged commit before anything
// is built. Everything is read with git at <commit> and from the remote-
// tracking branch <remote>/release/X.Y, never from the working tree. The
// checks run in this order and stop at the first failure:
//
// V1 The tag is vX.Y.Z or vX.Y.Z-beta.N; alpha tags are not published here.
// V2 X.Y.Z is the workspace version (Cargo.toml [workspace.package]) at
//    <commit>, as check-version --tag requires of the working tree.
// V3 <commit> is reachable from <remote>/release/X.Y.
// V4 (release) a vX.Y.Z-beta.N tag exists: every release passes through beta.
// V5 (release) <commit> is the head of <remote>/release/X.Y.
// V6 (release) since the last beta (the highest vX.Y.Z-beta.N of the same
//    X.Y.Z, compared numerically) only release-notes/ and changes/ changed.
// V7 (release) changes/ holds no fragment at <commit>: the final notes pull
//    request removes the release's fragments.
// V8 release-notes/X.Y.Z.md exists at <commit>.
//
// On success it prints one line, or with --json { stage, version, lastBeta }:
// version is the plain X.Y.Z; lastBeta is, for a release, the beta V6
// compared against and, for a beta, the highest beta of the same X.Y.Z below
// it (the --since of render-notes --beta), or null.
//
// Exit 0 when the tag may be published, 1 with `verify-tag: <message>`, 2 on
// a usage error.
import { cli } from "./lib/cli.mjs";
import { fragmentsIn } from "./lib/fragments.mjs";
import { fileAt, filesAt, git, tags, zList } from "./lib/git.mjs";
import { readWorkspaceVersion } from "./lib/manifests.mjs";
import { classifyTag, compare, plain } from "./lib/semver.mjs";

const USAGE = `usage: verify-tag --tag <vX.Y.Z | vX.Y.Z-beta.N> --commit <sha> [--remote <name>] [--json]
Checks that the beta or release <tag> on <commit> may be published: its
version is the workspace version, it is on <remote>/release/X.Y (default
remote: origin), and a release is on the branch head, after a beta, with only
release-notes/ and changes/ changed since that beta and no fragment left in
changes/. Both need release-notes/X.Y.Z.md. With --json prints
{ stage, version, lastBeta }.`;

const { fail, usageError, parseArgs } = cli("verify-tag", USAGE);

const ARGS = {
  flags: ["--json"],
  values: ["--tag", "--commit", "--remote"],
  defaults: { remote: "origin", json: false },
};

/** Stops the checks with the message verify-tag fails with. */
const refuse = (message) => {
  throw new Error(message);
};

/** The full SHA of `ref`, or null when it names no commit. */
const resolve = (ref) => {
  try {
    return git(["rev-parse", "--verify", "--quiet", `${ref}^{commit}`]).trim();
  } catch {
    return null;
  }
};

/** git merge-base --is-ancestor: exit 0 is yes, 1 is no, anything else throws. */
const isAncestor = (a, b) => {
  try {
    git(["merge-base", "--is-ancestor", a, b]);
    return true;
  } catch (error) {
    if (error.status === 1) return false;
    throw error;
  }
};

/** The beta tags of `version` (the glob names the version), highest first. */
const betasOf = (version) =>
  tags(`v${version}-beta.*`)
    .map((tag) => ({ tag, t: classifyTag(tag) }))
    .filter(({ t }) => t?.stage === "beta")
    .sort((a, b) => compare(b.t, a.t));

const ONLY_NOTES = /^(?:release-notes|changes)\//;

const verify = ({ tag, commit: ref, remote }) => {
  // V1
  const t = classifyTag(tag);
  if (t === null)
    refuse(`unknown tag shape ${tag}; expected vX.Y.Z or vX.Y.Z-beta.N`);
  if (t.stage === "alpha")
    refuse(`${tag}: alpha tags are not published by this job`);
  const version = plain(t);
  const commit = resolve(ref);
  if (commit === null) refuse(`--commit ${ref} is not a commit`);

  // V2
  const cargoToml = fileAt(commit, "Cargo.toml");
  if (cargoToml === null) refuse(`${tag}: Cargo.toml does not exist at ${ref}`);
  let workspace;
  try {
    workspace = readWorkspaceVersion(cargoToml);
  } catch (error) {
    refuse(`${tag}: Cargo.toml at ${ref}: ${error.message}`);
  }
  if (workspace !== version)
    refuse(
      `${tag}: version ${version} is not the workspace version ${workspace} in Cargo.toml at ${ref}`,
    );

  // V3
  const line = `release/${t.major}.${t.minor}`;
  const branch = resolve(`refs/remotes/${remote}/${line}`);
  if (branch === null)
    refuse(`${tag} is not on ${line}: ${remote}/${line} does not exist`);
  if (!isAncestor(commit, branch)) refuse(`${tag} is not on ${line}`);

  const betas = betasOf(version);
  let lastBeta = null;
  if (t.stage === "release") {
    // V4
    if (betas.length === 0)
      refuse(`no beta of ${version} exists; every release passes through beta`);
    lastBeta = betas[0].tag;
    // V5
    if (commit !== branch)
      refuse(`a release is tagged on the head of ${line}, which is ${branch}`);
    // V6
    const outside = zList(
      git(["diff", "-z", "--name-only", "--no-renames", lastBeta, commit]),
    ).filter((path) => !ONLY_NOTES.test(path));
    if (outside.length > 0)
      refuse(
        `since ${lastBeta} the following paths changed outside release-notes/ and changes/: ${outside.join(", ")}`,
      );
    // V7
    const left = fragmentsIn(filesAt(commit, "changes")).map((f) => f.file);
    if (left.length > 0)
      refuse(
        `${left.length} ${left.length === 1 ? "fragment is" : "fragments are"} still in changes/ at ${tag}; the final notes PR removes them: ${left.join(", ")}`,
      );
  } else {
    lastBeta = betas.find(({ t: b }) => compare(b, t) < 0)?.tag ?? null;
  }

  // V8
  const notes = `release-notes/${version}.md`;
  if (fileAt(commit, notes) === null)
    refuse(`${notes} does not exist at ${tag}`);

  return { stage: t.stage, version, lastBeta };
};

const main = () => {
  const opts = parseArgs(process.argv.slice(2), ARGS);
  if (opts.tag === undefined || opts.commit === undefined) usageError();
  let answer;
  try {
    answer = verify(opts);
  } catch (error) {
    return fail(error.message);
  }
  if (opts.json) console.log(JSON.stringify(answer));
  else
    console.log(
      `verify-tag: ${opts.tag} may be published (${answer.stage} of ${answer.version})`,
    );
};

main();
