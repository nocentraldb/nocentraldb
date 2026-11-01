#!/usr/bin/env node
// The fragments a release shipped (docs/release-management.md, "Release"):
// the commit back to main after a release removes them, so the next
// release's notes never repeat them. Rules:
//
// S1 The shipped fragments are the changes/<slug>.md deleted on the release
//    branch between the merge base of <tag> and <main>, and <tag>; each is
//    listed as `<blob> <path>`, the blob being the content that shipped (the
//    deleted side of the last deletion), in path order. README.md and
//    nested paths are not fragments.
// S2 With --delete, for each of them in the working tree (a checkout of
//    main, run from the repository root): content equal to the shipped blob
//    (git hash-object) is removed with git rm, printing `removed <path>`; a
//    different content is kept, printing `kept <path>: differs from the
//    shipped fragment (slug reused?)`; a missing file prints `absent <path>`.
//    A fragment is never deleted by its name alone.
// S3 <tag> is an existing release tag vX.Y.Z.
//
// Exit 0 with the list or the actions (nothing at all when nothing shipped),
// 1 with `shipped-fragments: <message>`, 2 on a usage error.
import { existsSync } from "node:fs";
import { cli } from "./lib/cli.mjs";
import { fragmentsIn } from "./lib/fragments.mjs";
import { git, mergeBase } from "./lib/git.mjs";
import { classifyTag } from "./lib/semver.mjs";

const USAGE = `usage: shipped-fragments --tag <vX.Y.Z> [--main <ref>] [--delete]
Lists the change fragments the release <tag> shipped: those deleted on the
release branch since its merge base with <ref> (default origin/main), each as
"<blob> <path>". With --delete, removes each from the working tree and the
index whose content is that blob, and keeps (and names) one that differs.`;

const { fail, usageError, parseArgs } = cli("shipped-fragments", USAGE);

const ARGS = {
  flags: ["--delete"],
  values: ["--tag", "--main"],
  defaults: { main: "origin/main", delete: false },
};

/** path -> blob of every path deleted under changes/ in `range`, newest deletion first. */
const deletions = (range) => {
  const out = git([
    "log",
    "--diff-filter=D",
    "--no-renames",
    "--raw",
    "--no-abbrev",
    "-z",
    "--format=",
    range,
    "--",
    "changes/",
  ]);
  const deleted = new Map();
  const fields = out.split("\0");
  for (let i = 0; i < fields.length; i++) {
    const meta = fields[i].trim();
    if (!meta.startsWith(":")) continue;
    const [, , oldBlob] = meta.split(" ");
    const path = fields[++i];
    if (!deleted.has(path)) deleted.set(path, oldBlob);
  }
  return deleted;
};

const shipped = ({ tag, main }) => {
  if (classifyTag(tag)?.stage !== "release")
    throw new Error(`--tag ${tag} is not a release tag vX.Y.Z`);
  try {
    git(["rev-parse", "--verify", "--quiet", `refs/tags/${tag}`]);
  } catch {
    throw new Error(`--tag ${tag} does not exist`);
  }
  const deleted = deletions(`${mergeBase(tag, main)}..${tag}`);
  return fragmentsIn([...deleted.keys()]).map(({ file }) => ({
    path: file,
    blob: deleted.get(file),
  }));
};

const remove = ({ path, blob }) => {
  if (!existsSync(path)) return `absent ${path}`;
  if (git(["hash-object", "--", path]).trim() !== blob)
    return `kept ${path}: differs from the shipped fragment (slug reused?)`;
  git(["rm", "-q", "--", path]);
  return `removed ${path}`;
};

const main = () => {
  const opts = parseArgs(process.argv.slice(2), ARGS);
  if (opts.tag === undefined) usageError();
  try {
    for (const fragment of shipped(opts))
      console.log(
        opts.delete ? remove(fragment) : `${fragment.blob} ${fragment.path}`,
      );
  } catch (error) {
    fail(error.message);
  }
};

main();
