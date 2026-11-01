#!/usr/bin/env node
// Renders the body of a GitHub release (docs/release-management.md, "Release
// notes"). The alpha job of .github/workflows/release.yml runs it in alpha
// mode; beta and release mode come with the publish side. Fragments are read
// as they stand at <head> (git), never from the working tree. Rules of the
// alpha mode:
//
// A1 The notes are the fragments added since <tag> (every fragment present
//    when it is none), every type included: alpha notes are raw fragments,
//    the curated-notes filter does not apply to them.
// A2 In slug order, each as `## <slug>`, a metadata line
//    `*<type>* · components: a, b · tickets: x, y`, and its body with its
//    sections (`## ` headings outside code fences) demoted to `### `.
// A3 Then a link to the previous alpha's GitHub release of <owner/name>, or
//    `First alpha.` when there is none.
// A4 <tag> is none or an alpha tag that exists; every fragment rendered must
//    be valid. Otherwise exit 1 naming it, and nothing on stdout.
//
// Exit 0 with the notes on stdout, 1 with one line per violation, 2 on a
// usage error (and for --beta/--release, not implemented in this ticket).
import { demoteSections, fragmentsAt, fragmentsIn } from "./lib/fragments.mjs";
import { addedPaths, git } from "./lib/git.mjs";
import { classifyTag } from "./lib/semver.mjs";

const USAGE = `usage: render-notes --alpha --since <tag|none> --repo <owner/name> [--head <ref>]
Prints the GitHub release body of an alpha: every change fragment added
between the previous alpha <tag> and <ref> (default HEAD), or every fragment
present at <ref> with --since none, then a link to the previous alpha's
release on github.com/<owner/name>.`;

const fail = (message) => {
  console.error(`render-notes: ${message}`);
  process.exitCode = 1;
};

const usageError = () => {
  console.error(USAGE);
  process.exit(2);
};

const REPO = /^[\w.-]+\/[\w.-]+$/;

const parseArgs = (args) => {
  const opts = { head: "HEAD" };
  for (let i = 0; i < args.length; i++) {
    const value = () => (args[i + 1] ? args[++i] : usageError());
    if (args[i] === "--help") {
      console.log(USAGE);
      process.exit(0);
    } else if (args[i] === "--beta" || args[i] === "--release") {
      console.error(
        `render-notes: ${args[i]} is not implemented in this ticket (alpha mode only)`,
      );
      process.exit(2);
    } else if (args[i] === "--alpha") opts.alpha = true;
    else if (args[i] === "--since") opts.since = value();
    else if (args[i] === "--repo") opts.repo = value();
    else if (args[i] === "--head") opts.head = value();
    else usageError();
  }
  if (!opts.alpha || opts.since === undefined || !REPO.test(opts.repo ?? ""))
    usageError();
  return opts;
};

/** The fragments the notes cover; null after reporting why there are none. */
const coveredFragments = ({ since, head }) => {
  let added = null;
  if (since !== "none") {
    if (classifyTag(since)?.stage !== "alpha")
      return fail(`--since ${since} is not an alpha tag (vX.Y.Z-alpha.N)`);
    git(["rev-parse", "--verify", "--quiet", `refs/tags/${since}`]);
    added = new Set(
      fragmentsIn(addedPaths(since, head, "changes/")).map((f) => f.file),
    );
  }
  const covered = fragmentsAt(head).filter(
    (f) => added === null || added.has(f.file),
  );
  for (const { file, errors } of covered)
    for (const error of errors) fail(`${file}: ${error}`);
  return covered.some((f) => f.errors.length > 0) ? null : covered;
};

const entry = ({ slug, front, body }) =>
  [
    `## ${slug}`,
    "",
    `*${front.type}* · components: ${front.components.join(", ")} · tickets: ${front.tickets.join(", ")}`,
    "",
    demoteSections(body.trim()),
  ].join("\n");

const footer = ({ since, repo }) =>
  since === "none"
    ? "First alpha."
    : `Previous alpha: [${since}](https://github.com/${repo}/releases/tag/${since})`;

const main = () => {
  const opts = parseArgs(process.argv.slice(2));
  let covered;
  try {
    covered = coveredFragments(opts);
  } catch (error) {
    return fail(error.message);
  }
  if (!covered) return;
  console.log([...covered.map(entry), footer(opts)].join("\n\n"));
};

main();
