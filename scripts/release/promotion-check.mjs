#!/usr/bin/env node
// Reports the promotion conditions of a release (docs/release-management.md,
// "Release") with evidence, as a Markdown table the PM pastes into the
// proposal to the owner. It first fetches <remote> with its tags, then reads
// the release branch as <remote>/release/X.Y. Published facts come from
// GitHub releases, never from a tag's existence or date. Rules:
//
// P1 Soak: the latest beta is the highest vX.Y.Z-beta.N tag, by number. Its
//    GitHub release (gh release view <tag> --json publishedAt) must have
//    been published at least 7 days before --now. FAIL when it is younger,
//    when there is no beta tag, when gh has no release for it (a tag whose
//    publish was refused or half done is not a published beta), when the
//    release is a draft, or when gh is missing.
// P2 No open regressions: MANUAL, on the board.
// P3 Upgrade tested: MANUAL until the relay and its upgrade test exist.
// P4 Same product code: every path that differs between the latest beta and
//    <remote>/release/X.Y is under release-notes/ or changes/; FAIL lists
//    the others. Renames count as delete plus add (--no-renames), so a
//    product file moved under release-notes/ is named by its old path.
// P5 Security: cargo audit and pnpm audit --audit-level low, run in a
//    temporary checkout of <remote>/release/X.Y, both exit 0. FAIL with the
//    last lines of a non-zero exit; a missing tool is a FAIL with the
//    install hint, never a pass.
// P6 Notes approved: MANUAL (the owner approves the notes in the pull
//    request), plus a mechanical part: every breaking fragment at
//    <remote>/release/X.Y has at least one of its tickets mentioned (as a
//    whole id, not a prefix of a longer one) in
//    release-notes/X.Y.Z.md there, otherwise FAIL naming the fragment. When
//    changes/ holds no fragment there any more, the PM checked it.
// P7 Real use: MANUAL.
//
// Test seams: PROMOTION_CHECK_GH is the command used instead of gh;
// PROMOTION_CHECK_CARGO_AUDIT and PROMOTION_CHECK_PNPM_AUDIT are commands
// run, with no arguments, instead of cargo audit and pnpm audit; --now
// injects the clock.
//
// Exit 0 when no mechanical condition (1, 4, 5, the mechanical part of 6)
// failed, 1 when one did or the fetch failed, 2 on a usage error.
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { cli } from "./lib/cli.mjs";
import { fragmentsAt } from "./lib/fragments.mjs";
import { fileAt, git, tags, zList } from "./lib/git.mjs";
import { classifyTag, PLAIN, parseVersion } from "./lib/semver.mjs";

const USAGE = `usage: promotion-check <X.Y.Z> [--now <ISO date>] [--remote <name>]
Fetches <remote> (default origin) and reports the promotion conditions of
<X.Y.Z> as a table, PASS, FAIL or MANUAL with evidence: the soak of the
latest beta's GitHub release, the product code since that beta on
<remote>/release/X.Y, cargo audit and pnpm audit on that branch, and the
breaking changes' tickets in the curated notes. Exit 1 when a mechanical
condition fails.`;

const { fail, usageError, parseArgs } = cli("promotion-check", USAGE);

const SOAK_DAYS = 7;
const HOUR = 60 * 60 * 1000;
const PASS = "PASS";
const FAIL = "FAIL";
const MANUAL = "MANUAL";

const options = (args) => {
  const [version, ...rest] = args;
  if (version === "--help") parseArgs([version]);
  if (!PLAIN.test(version ?? "")) usageError();
  const opts = parseArgs(rest, {
    values: ["--now", "--remote"],
    defaults: { remote: "origin" },
  });
  const now = opts.now === undefined ? Date.now() : Date.parse(opts.now);
  if (Number.isNaN(now)) usageError();
  return { ...opts, version, now, nowGiven: opts.now !== undefined };
};

/** Runs a command; { status, stdout, stderr, missing }. */
const exec = (command, args, cwd) => {
  const r = spawnSync(command, args, { cwd, encoding: "utf8" });
  return {
    status: r.status,
    stdout: r.stdout ?? "",
    stderr: r.stderr ?? "",
    missing: r.error?.code === "ENOENT",
  };
};

const lastLines = (text, n = 3) =>
  text.trim().split("\n").filter(Boolean).slice(-n).join("<br>");

const duration = (ms) => {
  const hours = Math.floor(ms / HOUR);
  return `${Math.floor(hours / 24)}d ${hours % 24}h`;
};

/** The highest vX.Y.Z-beta.N tag, or null. */
const latestBeta = (version) =>
  tags(`v${version}-beta.*`)
    .map(classifyTag)
    .filter((t) => t?.stage === "beta" && t.version.startsWith(`${version}-`))
    .sort((a, b) => a.n - b.n)
    .map((t) => `v${t.version}`)
    .at(-1) ?? null;

const soak = (ctx) => {
  const [result, evidence] = soakRow(ctx);
  return [
    result,
    process.env.PROMOTION_CHECK_GH ? `gh (override): ${evidence}` : evidence,
  ];
};

const soakRow = ({ beta, version, now }) => {
  if (beta === null) return [FAIL, `no beta tag v${version}-beta.N`];
  const gh = exec(process.env.PROMOTION_CHECK_GH || "gh", [
    "release",
    "view",
    beta,
    "--json",
    "publishedAt",
  ]);
  if (gh.missing) return [FAIL, "gh is not installed (https://cli.github.com)"];
  if (gh.status !== 0)
    return [
      FAIL,
      `no GitHub release for ${beta} (gh: ${lastLines(gh.stderr, 1)})`,
    ];
  let publishedAt;
  try {
    ({ publishedAt } = JSON.parse(gh.stdout));
  } catch {
    return [FAIL, `gh release view ${beta} printed no JSON`];
  }
  const published = Date.parse(publishedAt ?? "");
  if (Number.isNaN(published))
    return [FAIL, `the GitHub release for ${beta} is not published (draft)`];
  const age = now - published;
  return age >= SOAK_DAYS * 24 * HOUR
    ? [PASS, `${beta} published ${publishedAt}, ${duration(age)} ago`]
    : [
        FAIL,
        `${beta} published ${publishedAt}, only ${duration(Math.max(age, 0))} ago (${SOAK_DAYS}d needed)`,
      ];
};

const sameCode = ({ beta, version, ref, head }) => {
  if (head === null) return [FAIL, `${ref} does not exist`];
  if (beta === null) return [FAIL, `no beta tag v${version}-beta.N`];
  const paths = zList(
    git(["diff", "-z", "--no-renames", "--name-only", beta, head]),
  );
  const product = paths.filter(
    (p) => !p.startsWith("release-notes/") && !p.startsWith("changes/"),
  );
  return product.length === 0
    ? [
        PASS,
        `since ${beta}, only release-notes/ and changes/ changed on ${ref} (${paths.length} path${paths.length === 1 ? "" : "s"})`,
      ]
    : [
        FAIL,
        `changed on ${ref} since ${beta} outside release-notes/ and changes/: ${product.join(", ")}`,
      ];
};

/** One audit: [ok, evidence]. */
const audit = (tool, seam, command, args, missingHint, cwd) => {
  const override = process.env[seam];
  const name = override ? `${tool} (override)` : tool;
  const r = override ? exec(override, [], cwd) : exec(command, args, cwd);
  if (r.missing || /no such (sub)?command/.test(r.stderr))
    return [false, missingHint];
  if (r.status !== 0)
    return [
      false,
      `${name} exited ${r.status}: ${lastLines(`${r.stdout}\n${r.stderr}`)}`,
    ];
  return [true, `${name}: clean`];
};

const security = ({ ref, head }) => {
  if (head === null) return [FAIL, `${ref} does not exist`];
  const dir = mkdtempSync(join(tmpdir(), "promotion-check-"));
  let added = false;
  try {
    git(["worktree", "add", "-q", "--detach", dir, head]);
    added = true;
    const results = [
      audit(
        "cargo audit",
        "PROMOTION_CHECK_CARGO_AUDIT",
        "cargo",
        ["audit"],
        "cargo-audit is not installed (cargo install cargo-audit)",
        dir,
      ),
      audit(
        "pnpm audit",
        "PROMOTION_CHECK_PNPM_AUDIT",
        "pnpm",
        ["audit", "--audit-level", "low"],
        "pnpm is not installed (https://pnpm.io/installation)",
        dir,
      ),
    ];
    const evidence = `${results.map(([, e]) => e).join("; ")}; at ${head.slice(0, 12)}; open security advisories are checked by hand`;
    return [results.every(([ok]) => ok) ? PASS : FAIL, evidence];
  } finally {
    // Only a worktree that was added is removed, so a failed add keeps its
    // own error instead of being replaced by the remove's.
    if (added) git(["worktree", "remove", "--force", dir]);
    rmSync(dir, { recursive: true, force: true });
  }
};

/**
 * Whether `text` names the ticket id `ticket` as a whole id: `mt08.brk` is
 * not mentioned by `mt08.brk2`.
 */
const mentions = (text, ticket) => {
  const id = ticket.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`(?<![\\w.])${id}(?![\\w.])`).test(text);
};

const notesApproved = ({ version, ref, head }) => {
  if (head === null) return [FAIL, `${ref} does not exist`];
  const fragments = fragmentsAt(head);
  if (fragments.length === 0)
    return [
      MANUAL,
      `fragments already removed, checked by the PM; the owner approves the notes in the final notes pull request`,
    ];
  const invalid = fragments.filter((f) => f.errors.length > 0);
  if (invalid.length > 0)
    return [FAIL, invalid.map((f) => `${f.file}: ${f.errors[0]}`).join("; ")];
  const path = `release-notes/${version}.md`;
  const notes = fileAt(head, path);
  if (notes === null) return [FAIL, `${path} does not exist at ${ref}`];
  const breaking = fragments.filter((f) => f.front.type === "breaking");
  const missing = breaking.filter(
    (f) => !f.front.tickets.some((ticket) => mentions(notes, ticket)),
  );
  if (missing.length > 0)
    return [
      FAIL,
      missing
        .map(
          (f) =>
            `${f.file} (breaking): none of ${f.front.tickets.join(", ")} is in ${path}`,
        )
        .join("; "),
    ];
  const named =
    breaking.length === 0
      ? "no breaking fragment"
      : `every breaking fragment has a ticket in ${path}: ${breaking.map((f) => f.file).join(", ")}`;
  return [
    MANUAL,
    `${named}; the owner approves the notes, each breaking change with its Migration, in the notes pull request`,
  ];
};

const cell = (text) => text.replaceAll("|", "\\|").replaceAll("\n", "<br>");

const main = () => {
  const opts = options(process.argv.slice(2));
  const { version, remote, now } = opts;
  const { major, minor } = parseVersion(version);
  const ref = `${remote}/release/${major}.${minor}`;
  try {
    git(["fetch", "--quiet", "--tags", remote]);
  } catch (error) {
    return fail(`${error.message}; nothing was checked`);
  }
  let head = null;
  try {
    head = git(["rev-parse", "--verify", "--quiet", `${ref}^{commit}`]).trim();
  } catch {}
  const ctx = { version, now, ref, head, beta: latestBeta(version) };
  const guarded = (check) => {
    try {
      return check(ctx);
    } catch (error) {
      return [FAIL, error.message];
    }
  };
  const rows = [
    [1, "Soak", guarded(soak), true],
    [
      2,
      "No open regressions",
      [
        MANUAL,
        "on the board: nxf list --type bug --status open; no P0 or P1 bug whose introduced is this cycle",
      ],
      false,
    ],
    [
      3,
      "Upgrade tested",
      [
        MANUAL,
        "waits on the relay and its upgrade test (WAIT: the relay exists, mt08.7wjb)",
      ],
      false,
    ],
    [4, "Same product code", guarded(sameCode), true],
    [5, "Security", guarded(security), true],
    [6, "Notes approved", guarded(notesApproved), true],
    [
      7,
      "Real use",
      [
        MANUAL,
        `at least one of our own projects runs ${ctx.beta ?? "the beta"} with no new bug report, on the board`,
      ],
      false,
    ],
  ];
  const overrides = [
    "PROMOTION_CHECK_GH",
    "PROMOTION_CHECK_CARGO_AUDIT",
    "PROMOTION_CHECK_PNPM_AUDIT",
  ].filter((name) => process.env[name]);
  if (opts.nowGiven) overrides.push("--now");
  const at =
    head === null ? `${ref} (missing)` : `${ref} at ${head.slice(0, 12)}`;
  console.log(
    [
      `promotion-check ${version}: ${at}, latest beta ${ctx.beta ?? "none"}, now ${new Date(now).toISOString()}`,
      ...(overrides.length === 0
        ? []
        : [
            `overrides in effect: ${overrides.join(", ")}; this table is not evidence for a release`,
          ]),
      "",
      "| # | Condition | Result | Evidence |",
      "| --- | --- | --- | --- |",
      ...rows.map(
        ([n, name, [result, evidence]]) =>
          `| ${n} | ${name} | ${result} | ${cell(evidence)} |`,
      ),
    ].join("\n"),
  );
  const failed = rows.filter(
    ([, , [result], mechanical]) => mechanical && result === FAIL,
  );
  if (failed.length > 0)
    fail(
      `${failed.length} mechanical condition${failed.length === 1 ? "" : "s"} failed: ${failed.map(([n]) => n).join(", ")}`,
    );
};

main();
