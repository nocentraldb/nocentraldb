// The owner's review of the texts before a beta or a stable release, `pnpm
// texts` (Carsten, 2026-11-15, NOC-329): moved out of scripts/texts.mjs,
// which re-exports it, to keep that file under 700 lines (T3).

import { execFileSync } from "node:child_process";
import { readTexts, readTextsAt } from "./texts-files.mjs";

/** A release tag, `vX.Y.Z` or `vX.Y.Z-beta.N`: never an alpha. */
const RELEASE_TAG = /^v(\d+)\.(\d+)\.(\d+)(?:-beta\.(\d+))?$/;

/** The newest release tag of `tags`, compared by number; a stable release is above its betas. */
export const lastReleaseTag = (tags) => {
  const key = (tag) => {
    const [, x, y, z, beta] = tag.match(RELEASE_TAG);
    return [
      Number(x),
      Number(y),
      Number(z),
      beta === undefined ? Infinity : Number(beta),
    ];
  };
  const newer = (a, b) => {
    const [ka, kb] = [key(a), key(b)];
    for (let i = 0; i < ka.length; i++)
      if (ka[i] !== kb[i]) return ka[i] > kb[i];
    return false;
  };
  return tags
    .filter((tag) => RELEASE_TAG.test(tag))
    .reduce(
      (best, tag) => (best === null || newer(tag, best) ? tag : best),
      null,
    );
};

/** The entries new, changed and removed between two lists of texts. */
export const diffTexts = (before, after) => {
  const old = new Map(before.map((t) => [t.id, t]));
  const now = new Map(after.map((t) => [t.id, t]));
  const same = (a, b) =>
    ["kind", "code", "text", "when"].every((f) => a[f] === b[f]);
  return {
    added: after.filter((t) => !old.has(t.id)),
    changed: after
      .filter((t) => old.has(t.id) && !same(old.get(t.id), t))
      .map((t) => ({ before: old.get(t.id), after: t })),
    removed: before.filter((t) => !now.has(t.id)),
  };
};

const kindAndCode = (t) => `${t.kind}${t.code ? ` · ${t.code}` : ""}`;

/** The review as Markdown, for the owner and pm. */
export const formatReview = (diff, tag, total) => {
  const { added, changed, removed } = diff;
  const out = [
    tag
      ? `# Texts new or changed since ${tag}`
      : "# Texts: no beta or stable release yet, so every text is new",
    "",
    `${added.length} new, ${changed.length} changed, ${removed.length} removed, of ${total} texts in texts/.`,
  ];
  if (added.length > 0) {
    out.push("", "## New");
    for (const t of added) {
      out.push(
        "",
        `### ${t.id}`,
        "",
        `- ${kindAndCode(t)}`,
        `- when: ${t.when}`,
        `- text: "${t.text}"`,
      );
    }
  }
  if (changed.length > 0) {
    out.push("", "## Changed");
    for (const { before, after } of changed) {
      out.push("", `### ${after.id}`, "", `- ${kindAndCode(after)}`);
      if (kindAndCode(before) !== kindAndCode(after))
        out.push(`- was: ${kindAndCode(before)}`);
      out.push(`- when: ${after.when}`);
      if (before.when !== after.when) out.push(`- when was: ${before.when}`);
      if (before.text !== after.text) {
        out.push(`- text was: "${before.text}"`, `- text now: "${after.text}"`);
      } else out.push(`- text (unchanged): "${after.text}"`);
    }
  }
  if (removed.length > 0) {
    out.push("", "## Removed");
    for (const t of removed) {
      out.push(
        "",
        `### ${t.id}`,
        "",
        `- ${kindAndCode(t)}`,
        `- when: ${t.when}`,
        `- text: "${t.text}"`,
      );
    }
  }
  return `${out.join("\n")}\n`;
};

/** The review of the texts in the repository at `cwd`, against `since` or the last release tag. */
export const review = (cwd, since) => {
  const git = (...args) =>
    execFileSync("git", ["-C", cwd, ...args], { encoding: "utf8" });
  const tag =
    since ??
    lastReleaseTag(git("tag", "--list", "v*").split("\n").filter(Boolean));
  const now = readTexts(cwd).texts;
  // A tag from before the texts has none: every text is new since it.
  const before = tag ? readTextsAt(cwd, tag) : [];
  return formatReview(diffTexts(before, now), tag, now.length);
};
