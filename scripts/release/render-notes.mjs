#!/usr/bin/env node
// Renders the body of a GitHub release (docs/release-management.md, "Release
// notes"). The alpha job of .github/workflows/release.yml runs it in alpha
// mode, the publish job in beta and release mode. Every file is read as it
// stands at <head> (git), never from the working tree. Rules:
//
// A1 Alpha: the notes are the fragments added since <tag> (every fragment
//    present when it is none), every type included: alpha notes are raw
//    fragments, the curated-notes filter does not apply to them.
// A2 Alpha: in slug order, each as `## <slug>`, a metadata line
//    `*<type>* · components: a, b · tickets: x, y`, and its body with its
//    sections (`## ` headings outside code fences) demoted to `### `.
// A3 Alpha: then a link to the previous alpha's GitHub release of
//    <owner/name>, or `First alpha.` when there is none.
// A4 Alpha: <tag> is none or an alpha tag that exists.
// B1 Beta: the curated file release-notes/<X.Y.Z>.md as it is, then, when
//    <tag> is not none, a section `## Changes since <tag>` with the fragments
//    added between <tag> and <head>, unfiltered (a cycle fix appears here),
//    rendered as in A2 one level down (`### <slug>`, sections `#### `), or a
//    line saying no fragment was added.
// B2 Beta: <tag> is none or an existing beta tag of the same X.Y.Z.
// R1 Release: the curated file release-notes/<X.Y.Z>.md only.
// F1 The curated file must exist at <head>, and every fragment rendered must
//    be valid. Otherwise exit 1 naming it, and nothing on stdout.
//
// Exit 0 with the notes on stdout, 1 with one line per violation, 2 on a
// usage error.
import { cli } from "./lib/cli.mjs";
import { demoteSections, fragmentsAt, fragmentsIn } from "./lib/fragments.mjs";
import { addedPaths, fileAt, git } from "./lib/git.mjs";
import { classifyTag, PLAIN, plain } from "./lib/semver.mjs";

const USAGE = `usage: render-notes --alpha --since <tag|none> --repo <owner/name> [--head <ref>]
       render-notes --beta <X.Y.Z> --since <tag|none> [--head <ref>]
       render-notes --release <X.Y.Z> [--head <ref>]
Prints the GitHub release body of a stage, read at <ref> (default HEAD).
--alpha: every change fragment added between the previous alpha <tag> and
<ref>, or every fragment present with --since none, then a link to the
previous alpha's release on github.com/<owner/name>.
--beta: release-notes/<X.Y.Z>.md, then, unless --since none, the fragments
added since the previous beta <tag> of the same version, unfiltered.
--release: release-notes/<X.Y.Z>.md only.`;

const { fail, usageError, parseArgs } = cli("render-notes", USAGE);

const REPO = /^[\w.-]+\/[\w.-]+$/;

const ARGS = {
  flags: ["--alpha"],
  values: ["--beta", "--release", "--since", "--repo", "--head"],
  defaults: { head: "HEAD" },
};

/** The parsed options, with `mode` set; a usage error unless exactly one mode fits. */
const options = (args) => {
  const opts = parseArgs(args, ARGS);
  const modes = ["alpha", "beta", "release"].filter(
    (m) => opts[m] !== undefined,
  );
  if (modes.length !== 1) usageError();
  const [mode] = modes;
  const valid = {
    alpha: () => opts.since !== undefined && REPO.test(opts.repo ?? ""),
    beta: () =>
      PLAIN.test(opts.beta) &&
      opts.since !== undefined &&
      opts.repo === undefined,
    release: () =>
      PLAIN.test(opts.release) &&
      opts.since === undefined &&
      opts.repo === undefined,
  };
  if (!valid[mode]()) usageError();
  return { ...opts, mode };
};

/** Whether `tag` exists as a tag; a missing tag is reported by the caller. */
const tagExists = (tag) => {
  try {
    git(["rev-parse", "--verify", "--quiet", `refs/tags/${tag}`]);
    return true;
  } catch {
    return false;
  }
};

/**
 * The fragments at `head` added since `since` (all present when since is
 * null); null after reporting an invalid one.
 */
const fragmentsSince = (since, head) => {
  const added =
    since === null
      ? null
      : new Set(
          fragmentsIn(addedPaths(since, head, "changes/")).map((f) => f.file),
        );
  const covered = fragmentsAt(head).filter(
    (f) => added === null || added.has(f.file),
  );
  for (const { file, errors } of covered)
    for (const error of errors) fail(`${file}: ${error}`);
  return covered.some((f) => f.errors.length > 0) ? null : covered;
};

/** One fragment as a release-notes entry, its heading at `level`. */
const entry = ({ slug, front, body }, level) =>
  [
    `${"#".repeat(level)} ${slug}`,
    "",
    `*${front.type}* · components: ${front.components.join(", ")} · tickets: ${front.tickets.join(", ")}`,
    "",
    demoteSections(body.trim(), level - 1),
  ].join("\n");

/** The curated file of `version` at `head`; null after reporting it missing. */
const curated = (version, head) => {
  const path = `release-notes/${version}.md`;
  const text = fileAt(head, path);
  if (text === null) fail(`${path} does not exist at ${head}`);
  return text;
};

const alpha = ({ since, head, repo }) => {
  if (since !== "none") {
    if (classifyTag(since)?.stage !== "alpha" || !tagExists(since))
      return fail(
        `--since ${since} is not an existing alpha tag (vX.Y.Z-alpha.N)`,
      );
  }
  const covered = fragmentsSince(since === "none" ? null : since, head);
  if (covered === null) return;
  const footer =
    since === "none"
      ? "First alpha."
      : `Previous alpha: [${since}](https://github.com/${repo}/releases/tag/${since})`;
  process.stdout.write(
    `${[...covered.map((f) => entry(f, 2)), footer].join("\n\n")}\n`,
  );
};

const beta = ({ beta: version, since, head }) => {
  if (since !== "none") {
    const t = classifyTag(since);
    if (t?.stage !== "beta" || plain(t) !== version || !tagExists(since))
      return fail(
        `--since ${since} is not an existing beta tag of ${version} (v${version}-beta.N)`,
      );
  }
  const text = curated(version, head);
  if (text === null) return;
  if (since === "none") return process.stdout.write(text);
  const covered = fragmentsSince(since, head);
  if (covered === null) return;
  const changes =
    covered.length === 0
      ? `No change fragment was added since ${since}.`
      : covered.map((f) => entry(f, 3)).join("\n\n");
  process.stdout.write(
    `${text.trimEnd()}\n\n## Changes since ${since}\n\n${changes}\n`,
  );
};

const release = ({ release: version, head }) => {
  const text = curated(version, head);
  if (text !== null) process.stdout.write(text);
};

const main = () => {
  const opts = options(process.argv.slice(2));
  try {
    ({ alpha, beta, release })[opts.mode](opts);
  } catch (error) {
    fail(error.message);
  }
};

main();
