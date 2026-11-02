#!/usr/bin/env node
// Writes the skeleton of the curated release notes release-notes/<X.Y.Z>.md
// (docs/release-management.md, "Release notes" and "Beta"), which the PM or
// an agent then edits into the notes the owner approves. The fragments are
// read from changes/ in the working tree: on main, every fragment present
// has not shipped yet, because the release's commit to main removes the
// shipped ones. Rules:
//
// D1 The file starts with `# <X.Y.Z>` and a summary placeholder comment.
// D2 Then only the sections that have entries, in this order: Breaking
//    changes, New features, Deprecations, Fixes, Security; then Known issues,
//    always, with a placeholder comment (Known issues come from the board).
// D3 Each entry is `### <first line of its first section, or its slug>`, a
//    comment naming its slug and tickets (promotion-check looks the tickets
//    of a breaking change up in the notes), a line `Issues: #42, #57` when
//    the fragment lists issues (GitHub links #N in a release body), then its
//    sections as bold labels (`**Problem.** …`), in slug order within a
//    section.
// D4 The filter: an `internal` fragment and a `fix` with `introduced: cycle`
//    never appear in the body; a trailing comment lists them by slug.
// D5 Stale cycle fixes: with a previous release (--previous <vA.B.C>, or by
//    default the highest release in release-notes/index.json), a cycle fix
//    whose file was first added in a commit that is an ancestor of
//    merge-base(HEAD, <vA.B.C>) may have shipped in A.B.C. Each is a warning
//    on stderr naming the file and the version to set, and is listed in the
//    trailing comment. The fragment is not changed: whether the bug reached
//    the release branch is not derivable from it, so the PM decides.
// D6 Exit 1 without writing when the out file exists (unless --force), a
//    fragment is invalid, or the previous release tag does not exist.
//
// Exit 0 (also with warnings), 1 with `draft-notes: <message>` naming the
// file or the tag, 2 on a usage error.
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { cli } from "./lib/cli.mjs";
import { readFragments, splitSections } from "./lib/fragments.mjs";
import { git, mergeBase } from "./lib/git.mjs";
import { classifyTag, compare, PLAIN } from "./lib/semver.mjs";

const USAGE = `usage: draft-notes <X.Y.Z> [--out <file>] [--previous <vA.B.C>|auto] [--force]
Writes the skeleton of the curated notes for <X.Y.Z> to <file> (default
release-notes/<X.Y.Z>.md) from the change fragments in changes/: the
sections in order, internal fragments and cycle fixes left out. Warns for
every cycle fix that was added before the previous release <vA.B.C> was cut
(default auto: the highest release in release-notes/index.json). Refuses an
existing <file> unless --force.`;

const { fail, usageError, parseArgs } = cli("draft-notes", USAGE);

const ORDER = [
  ["breaking", "Breaking changes"],
  ["feature", "New features"],
  ["deprecation", "Deprecations"],
  ["fix", "Fixes"],
  ["security", "Security"],
];

const isCycleFix = (f) =>
  f.front.type === "fix" && f.front.introduced === "cycle";

/** A section as a bold label, inline when its text starts with a paragraph. */
const label = ({ name, text }) => {
  const head = `**${name}.**`;
  if (text === "") return head;
  return /^(\s|`{3}|~{3}|[-*+>|#]|\d+[.)]\s)/.test(text)
    ? `${head}\n\n${text}`
    : `${head} ${text}`;
};

/** One fragment as a curated-notes entry. */
const entry = ({ slug, front, body }) => {
  const { preamble, sections } = splitSections(body);
  const first = sections[0]?.text.split("\n")[0] ?? "";
  const title = first === "" || /^(`{3}|~{3})/.test(first) ? slug : first;
  return [
    `### ${title}`,
    `<!-- ${slug} · tickets: ${front.tickets.join(", ")} -->`,
    ...(front.issues === undefined
      ? []
      : [`Issues: ${front.issues.map((n) => `#${n}`).join(", ")}`]),
    ...(preamble === "" ? [] : [preamble]),
    ...sections.map(label),
  ].join("\n\n");
};

/** The tag of the highest release in the index, or null. */
const lastIndexedRelease = () => {
  const file = "release-notes/index.json";
  const { releases } = JSON.parse(readFileSync(file, "utf8"));
  const tags = releases
    .filter((r) => r.stage === "release")
    .map((r) => r.tag)
    .sort((a, b) => compare(classifyTag(a), classifyTag(b)));
  return tags.at(-1) ?? null;
};

const isAncestor = (a, b) => {
  try {
    git(["merge-base", "--is-ancestor", a, b]);
    return true;
  } catch (error) {
    if (error.status === 1) return false;
    throw error;
  }
};

/** The cycle fixes first added before merge-base(HEAD, previous). */
const staleCycleFixes = (fixes, previous) => {
  const base = mergeBase("HEAD", previous);
  return fixes.filter((f) => {
    const adds = git([
      "log",
      "--diff-filter=A",
      "--format=%H",
      "--",
      f.file,
    ]).trim();
    const firstAdd = adds.split("\n").at(-1);
    return firstAdd !== "" && isAncestor(firstAdd, base);
  });
};

const tagExists = (tag) => {
  try {
    git(["rev-parse", "--verify", "--quiet", `refs/tags/${tag}`]);
    return true;
  } catch {
    return false;
  }
};

const options = (args) => {
  const [version, ...rest] = args;
  if (version === "--help") parseArgs([version]);
  if (!PLAIN.test(version ?? "")) usageError();
  const opts = parseArgs(rest, {
    flags: ["--force"],
    values: ["--out", "--previous"],
    defaults: { out: `release-notes/${version}.md`, previous: "auto" },
  });
  if (
    opts.previous !== "auto" &&
    classifyTag(opts.previous)?.stage !== "release"
  )
    usageError();
  return { ...opts, version };
};

const main = () => {
  const {
    version,
    out,
    previous: asked,
    force,
  } = options(process.argv.slice(2));
  try {
    if (existsSync(out) && !force)
      return fail(`${out} exists; pass --force to overwrite it`);
    const fragments = readFragments("changes");
    const previous = asked === "auto" ? lastIndexedRelease() : asked;
    if (previous !== null && !tagExists(previous))
      return fail(`${previous} does not exist as a tag`);
    const cycle = fragments.filter(isCycleFix);
    const internal = fragments.filter((f) => f.front.type === "internal");
    const stale = previous === null ? [] : staleCycleFixes(cycle, previous);
    const shown = fragments.filter(
      (f) => !isCycleFix(f) && f.front.type !== "internal",
    );

    const parts = [
      `# ${version}`,
      "<!-- Summary: two or three sentences on what this release is for. -->",
    ];
    for (const [type, heading] of ORDER) {
      const entries = shown.filter((f) => f.front.type === type);
      if (entries.length > 0)
        parts.push(`## ${heading}`, ...entries.map(entry));
    }
    parts.push(
      "## Known issues",
      "<!-- Bugs introduced in this cycle that are known and not fixed, from the board; each with a workaround if there is one. -->",
    );
    const slugs = (list) => list.map((f) => f.slug).join(", ");
    const trailer = [
      [cycle, "Filtered (cycle fixes, not shown to users)"],
      [internal, "Filtered (internal, not shown to users)"],
      [stale, `Check introduced (added before ${previous} shipped)`],
    ]
      .filter(([list]) => list.length > 0)
      .map(([list, what]) => `<!-- ${what}: ${slugs(list)} -->`);
    if (trailer.length > 0) parts.push(trailer.join("\n"));
    writeFileSync(out, `${parts.join("\n\n")}\n`);

    const released = previous === null ? null : classifyTag(previous).version;
    for (const f of stale)
      console.error(
        `draft-notes: ${f.file} says introduced: cycle but was added before ${previous} shipped — if the bug is in ${released}, change it to introduced: ${released}`,
      );
    console.log(
      `draft-notes: wrote ${out} with ${shown.length} entries (${cycle.length + internal.length} filtered)`,
    );
  } catch (error) {
    fail(error.message);
  }
};

main();
