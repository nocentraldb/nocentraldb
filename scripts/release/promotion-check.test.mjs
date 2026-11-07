import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { GIT_ENV, makeRepo } from "./lib/test-repo.mjs";

const script = join(
  dirname(fileURLToPath(import.meta.url)),
  "promotion-check.mjs",
);

const PUBLISHED = "2026-03-01T12:00:00Z";
const DAY = 24 * 60 * 60 * 1000;
const after = (ms) => new Date(Date.parse(PUBLISHED) + ms).toISOString();

const BREAKING = `---\ntype: breaking\ntickets: [mt08.brk1, mt08.brk2]\ncomponents: [core]\n---\n## Problem\n\np\n\n## Use\n\nu\n\n## Migration\n\nm\n`;
const INTERNAL = `---\ntype: internal\ntickets: [mt08.int1]\ncomponents: [core]\n---\nTidied.\n`;
const NOTES_WITH =
  "# 0.1.0\n\n### Open takes a URL\n\n<!-- open-url · tickets: mt08.brk2 -->\n";
const NOTES_WITHOUT = "# 0.1.0\n\n### Open takes a URL\n";

/**
 * A repository with a bare origin: main, then release/0.1 with the notes and
 * the fragments, tagged v0.1.0-beta.1 and, after a notes edit,
 * v0.1.0-beta.2; everything pushed. `afterBeta(repo)` runs on the
 * branch after the last beta, before the push.
 */
const releaseRepo = (
  t,
  {
    notes = NOTES_WITH,
    fragments = {
      "changes/open-url.md": BREAKING,
      "changes/tidy.md": INTERNAL,
    },
    betas = ["v0.1.0-beta.1", "v0.1.0-beta.2"],
    afterBeta = null,
  } = {},
) => {
  const repo = makeRepo();
  const origin = mkdtempSync(join(tmpdir(), "promotion-origin-"));
  t.after(() => {
    repo.dispose();
    rmSync(origin, { recursive: true, force: true });
  });
  repo.commit("start", {
    "changes/README.md": "# guide\n",
    "src/lib.rs": "// v1\n",
    ...fragments,
  });
  repo.run("switch", "-q", "-c", "release/0.1");
  repo.commit("notes", { "release-notes/0.1.0.md": "# 0.1.0\n" });
  for (const [i, tag] of betas.entries()) {
    if (i === betas.length - 1)
      repo.commit("final notes", { "release-notes/0.1.0.md": notes });
    repo.tag(tag);
  }
  if (afterBeta) afterBeta(repo);
  repo.run("switch", "-q", "main");
  spawnSync("git", ["init", "-q", "--bare", origin], { env: GIT_ENV });
  repo.run("remote", "add", "origin", origin);
  repo.run("push", "-q", "--tags", "origin", "main", "release/0.1");
  return repo;
};

/** Executables in a temporary directory, by name -> shell script body. */
const fakes = (t, scripts) => {
  const bin = mkdtempSync(join(tmpdir(), "promotion-bin-"));
  t.after(() => rmSync(bin, { recursive: true, force: true }));
  const paths = {};
  for (const [name, body] of Object.entries(scripts)) {
    paths[name] = join(bin, name);
    writeFileSync(paths[name], `#!/bin/sh\n${body}\n`);
    chmodSync(paths[name], 0o755);
  }
  return paths;
};

/** A gh that knows one published release, `tag`, and no other. */
const ghFor = (tag, published = PUBLISHED) =>
  `if [ "$1 $2 $3 $4 $5" = "release view ${tag} --json publishedAt" ]; then echo '{"publishedAt":"${published}"}'; exit 0; fi\necho "release not found" >&2\nexit 1`;

const CLEAN = "echo 'no vulnerabilities found'\nexit 0";

const run = (repo, t, { gh, cargo = CLEAN, pnpm = CLEAN, now }, ...args) => {
  const bin = fakes(t, { gh, cargo, pnpm });
  return spawnSync(
    process.execPath,
    [script, "0.1.0", "--now", now ?? after(7 * DAY), ...args],
    {
      cwd: repo.dir,
      env: {
        ...GIT_ENV,
        PROMOTION_CHECK_GH: bin.gh,
        PROMOTION_CHECK_CARGO_AUDIT: bin.cargo,
        PROMOTION_CHECK_PNPM_AUDIT: bin.pnpm,
      },
      encoding: "utf8",
    },
  );
};

/** The table row of condition `n`: { result, evidence }. */
const row = (stdout, n) => {
  const line = stdout.split("\n").find((l) => l.startsWith(`| ${n} |`));
  assert.ok(line, `no row ${n} in:\n${stdout}`);
  const [, , result, ...rest] = line.split(" | ");
  return { result, evidence: rest.join(" | ").replace(/ \|$/, "") };
};

test("every mechanical condition passes at exactly 7 days: exit 0, the rest MANUAL", (t) => {
  const repo = releaseRepo(t);
  const r = run(repo, t, { gh: ghFor("v0.1.0-beta.2") });
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.match(r.stdout, /^\| # \| Condition \| Result \| Evidence \|$/m);
  assert.equal(row(r.stdout, 1).result, "PASS");
  assert.match(
    row(r.stdout, 1).evidence,
    /v0\.1\.0-beta\.2 published 2026-03-01T12:00:00Z, 7d 0h ago/,
  );
  assert.equal(row(r.stdout, 2).result, "MANUAL");
  assert.match(row(r.stdout, 2).evidence, /nxf list --type bug/);
  assert.equal(row(r.stdout, 3).result, "MANUAL");
  assert.match(row(r.stdout, 3).evidence, /WAIT: the relay exists/);
  assert.equal(row(r.stdout, 4).result, "PASS");
  assert.equal(row(r.stdout, 5).result, "PASS");
  assert.equal(row(r.stdout, 6).result, "MANUAL");
  assert.match(row(r.stdout, 6).evidence, /changes\/open-url\.md/);
  assert.equal(row(r.stdout, 7).result, "MANUAL");
});

test("soak FAILs at 6 days 23 hours, and the exit code is 1", (t) => {
  const repo = releaseRepo(t);
  const r = run(repo, t, {
    gh: ghFor("v0.1.0-beta.2"),
    now: after(7 * DAY - 60 * 60 * 1000),
  });
  assert.equal(r.status, 1);
  assert.equal(row(r.stdout, 1).result, "FAIL");
  assert.match(row(r.stdout, 1).evidence, /only 6d 23h ago/);
  assert.match(r.stderr, /promotion-check: 1 mechanical condition failed: 1\n/);
});

test("the latest beta is the highest by number, beta.10 above beta.2", (t) => {
  const repo = releaseRepo(t, {
    betas: ["v0.1.0-beta.2", "v0.1.0-beta.10"],
  });
  const r = run(repo, t, { gh: ghFor("v0.1.0-beta.10") });
  assert.equal(row(r.stdout, 1).result, "PASS", r.stdout);
  assert.match(row(r.stdout, 1).evidence, /v0\.1\.0-beta\.10 published/);
});

test("a beta tag without a GitHub release FAILs soak: the tag alone is not a published beta", (t) => {
  const repo = releaseRepo(t);
  const r = run(repo, t, { gh: ghFor("v0.1.0-beta.1") });
  assert.equal(r.status, 1);
  assert.equal(row(r.stdout, 1).result, "FAIL");
  assert.match(
    row(r.stdout, 1).evidence,
    /no GitHub release for v0\.1\.0-beta\.2 \(gh: release not found\)/,
  );
});

test("a draft release (no publishedAt) FAILs soak", (t) => {
  const repo = releaseRepo(t);
  const r = run(repo, t, {
    gh: `echo '{"publishedAt":null}'`,
  });
  assert.equal(row(r.stdout, 1).result, "FAIL");
  assert.match(row(r.stdout, 1).evidence, /is not published/);
});

test("a missing gh FAILs soak with the reason, never a pass", (t) => {
  const repo = releaseRepo(t);
  const bin = fakes(t, { cargo: CLEAN, pnpm: CLEAN });
  const r = spawnSync(
    process.execPath,
    [script, "0.1.0", "--now", after(30 * DAY)],
    {
      cwd: repo.dir,
      env: {
        ...GIT_ENV,
        PROMOTION_CHECK_GH: join(repo.dir, "no-such-gh"),
        PROMOTION_CHECK_CARGO_AUDIT: bin.cargo,
        PROMOTION_CHECK_PNPM_AUDIT: bin.pnpm,
      },
      encoding: "utf8",
    },
  );
  assert.equal(r.status, 1);
  assert.equal(row(r.stdout, 1).result, "FAIL");
  assert.match(row(r.stdout, 1).evidence, /gh is not installed/);
});

test("no beta tag FAILs soak and the same-code rule", (t) => {
  const repo = releaseRepo(t, { betas: [] });
  const r = run(repo, t, { gh: ghFor("v0.1.0-beta.1") });
  assert.equal(r.status, 1);
  for (const n of [1, 4]) {
    assert.equal(row(r.stdout, n).result, "FAIL");
    assert.match(row(r.stdout, n).evidence, /no beta tag v0\.1\.0-beta\.N/);
  }
});

test("a product change on the branch since the last beta FAILs condition 4, naming the path", (t) => {
  const repo = releaseRepo(t, {
    afterBeta: (repo) => {
      repo.remove("changes/tidy.md");
      repo.commit("after the beta", {
        "src/lib.rs": "// v2\n",
        "release-notes/0.1.0.md": `${NOTES_WITH}more\n`,
      });
    },
  });
  const r = run(repo, t, { gh: ghFor("v0.1.0-beta.2") });
  assert.equal(r.status, 1);
  assert.equal(row(r.stdout, 4).result, "FAIL");
  assert.match(row(r.stdout, 4).evidence, /changes\/: src\/lib\.rs$/);
});

test("a product file renamed into release-notes/ FAILs condition 4, naming the old path", (t) => {
  const repo = releaseRepo(t, {
    afterBeta: (repo) => {
      repo.run("mv", "src/lib.rs", "release-notes/lib.rs");
      repo.commit("move the product code under release-notes/");
    },
  });
  const r = run(repo, t, { gh: ghFor("v0.1.0-beta.2") });
  assert.equal(r.status, 1);
  assert.equal(row(r.stdout, 4).result, "FAIL", r.stdout);
  assert.match(row(r.stdout, 4).evidence, /changes\/: src\/lib\.rs$/);
});

test("only release-notes/ and changes/ changed since the last beta PASSes condition 4", (t) => {
  const repo = releaseRepo(t, {
    afterBeta: (repo) =>
      repo.commit("after the beta", {
        "release-notes/0.1.0.md": `${NOTES_WITH}more\n`,
      }),
  });
  const r = run(repo, t, { gh: ghFor("v0.1.0-beta.2") });
  assert.equal(row(r.stdout, 4).result, "PASS", r.stdout);
  assert.match(row(r.stdout, 4).evidence, /1 path/);
});

test("an audit that reports something FAILs condition 5 with its last lines", (t) => {
  const repo = releaseRepo(t);
  const r = run(repo, t, {
    gh: ghFor("v0.1.0-beta.2"),
    cargo: "echo 'Crate: foo'\necho 'error: 1 vulnerability found!'\nexit 1",
  });
  assert.equal(r.status, 1);
  assert.equal(row(r.stdout, 5).result, "FAIL");
  assert.match(
    row(r.stdout, 5).evidence,
    /cargo audit \(override\) exited 1: Crate: foo<br>error: 1 vulnerability found!/,
  );
  assert.match(row(r.stdout, 5).evidence, /pnpm audit \(override\): clean/);
});

test("a missing cargo-audit FAILs condition 5 with the install hint, whether cargo or the subcommand is missing", (t) => {
  const repo = releaseRepo(t);
  const subcommand = run(repo, t, {
    gh: ghFor("v0.1.0-beta.2"),
    cargo: "echo 'error: no such command: `audit`' >&2\nexit 101",
  });
  assert.equal(subcommand.status, 1);
  assert.equal(row(subcommand.stdout, 5).result, "FAIL");
  assert.match(
    row(subcommand.stdout, 5).evidence,
    /cargo-audit is not installed \(cargo install cargo-audit\)/,
  );
  const bin = fakes(t, { gh: ghFor("v0.1.0-beta.2"), pnpm: CLEAN });
  const missing = spawnSync(
    process.execPath,
    [script, "0.1.0", "--now", after(7 * DAY)],
    {
      cwd: repo.dir,
      env: {
        ...GIT_ENV,
        PROMOTION_CHECK_GH: bin.gh,
        PROMOTION_CHECK_CARGO_AUDIT: join(repo.dir, "no-such-cargo"),
        PROMOTION_CHECK_PNPM_AUDIT: bin.pnpm,
      },
      encoding: "utf8",
    },
  );
  assert.equal(missing.status, 1);
  assert.match(
    row(missing.stdout, 5).evidence,
    /cargo-audit is not installed \(cargo install cargo-audit\)/,
  );
});

test("a missing pnpm FAILs condition 5", (t) => {
  const repo = releaseRepo(t);
  const bin = fakes(t, { gh: ghFor("v0.1.0-beta.2"), cargo: CLEAN });
  const r = spawnSync(
    process.execPath,
    [script, "0.1.0", "--now", after(7 * DAY)],
    {
      cwd: repo.dir,
      env: {
        ...GIT_ENV,
        PROMOTION_CHECK_GH: bin.gh,
        PROMOTION_CHECK_CARGO_AUDIT: bin.cargo,
        PROMOTION_CHECK_PNPM_AUDIT: join(repo.dir, "no-such-pnpm"),
      },
      encoding: "utf8",
    },
  );
  assert.equal(r.status, 1);
  assert.equal(row(r.stdout, 5).result, "FAIL");
  assert.match(row(r.stdout, 5).evidence, /pnpm is not installed/);
});

test("the audits run in a checkout of the release branch, not in the working tree", (t) => {
  const repo = releaseRepo(t);
  const r = run(repo, t, {
    gh: ghFor("v0.1.0-beta.2"),
    cargo:
      'if [ -f release-notes/0.1.0.md ]; then exit 0; fi\necho "wrong tree: $PWD"\nexit 1',
  });
  assert.equal(row(r.stdout, 5).result, "PASS", r.stdout);
});

test("a breaking fragment none of whose tickets is in the notes FAILs condition 6, naming it", (t) => {
  const repo = releaseRepo(t, { notes: NOTES_WITHOUT });
  const r = run(repo, t, { gh: ghFor("v0.1.0-beta.2") });
  assert.equal(r.status, 1);
  assert.equal(row(r.stdout, 6).result, "FAIL");
  assert.match(
    row(r.stdout, 6).evidence,
    /changes\/open-url\.md \(breaking\): none of mt08\.brk1, mt08\.brk2 is in release-notes\/0\.1\.0\.md/,
  );
  assert.match(r.stderr, /failed: 6\n/);
});

test("a ticket id that is only a prefix of an id in the notes does not satisfy condition 6", (t) => {
  const repo = releaseRepo(t, {
    fragments: {
      "changes/open-url.md": BREAKING.replace(
        "[mt08.brk1, mt08.brk2]",
        "[mt08.brk]",
      ),
    },
  });
  const r = run(repo, t, { gh: ghFor("v0.1.0-beta.2") });
  assert.equal(r.status, 1);
  assert.equal(row(r.stdout, 6).result, "FAIL", r.stdout);
  assert.match(row(r.stdout, 6).evidence, /none of mt08\.brk is in/);
});

test("a ticket id at the end of a sentence in the notes satisfies condition 6", (t) => {
  const repo = releaseRepo(t, {
    notes: "# 0.1.0\n\n### Open takes a URL\n\nSee mt08.brk2.\n",
  });
  const r = run(repo, t, { gh: ghFor("v0.1.0-beta.2") });
  assert.equal(row(r.stdout, 6).result, "MANUAL", r.stdout);
});

test("a Linear issue id in the notes satisfies condition 6", (t) => {
  const repo = releaseRepo(t, {
    fragments: {
      "changes/open-url.md": BREAKING.replace(
        "[mt08.brk1, mt08.brk2]",
        "[NOC-12]",
      ),
    },
    notes: "# 0.1.0\n\n### Open takes a URL\n\nSee NOC-12.\n",
  });
  const r = run(repo, t, { gh: ghFor("v0.1.0-beta.2") });
  assert.equal(row(r.stdout, 6).result, "MANUAL", r.stdout);
});

test("a Linear issue id that is only a prefix of an id in the notes does not satisfy condition 6", (t) => {
  const repo = releaseRepo(t, {
    fragments: {
      "changes/open-url.md": BREAKING.replace(
        "[mt08.brk1, mt08.brk2]",
        "[NOC-12]",
      ),
    },
    notes: "# 0.1.0\n\n### Open takes a URL\n\nSee NOC-123.\n",
  });
  const r = run(repo, t, { gh: ghFor("v0.1.0-beta.2") });
  assert.equal(r.status, 1);
  assert.equal(row(r.stdout, 6).result, "FAIL", r.stdout);
  assert.match(row(r.stdout, 6).evidence, /none of NOC-12 is in/);
});

test("with the fragments already removed, condition 6 is MANUAL", (t) => {
  const repo = releaseRepo(t, { fragments: {} });
  const r = run(repo, t, { gh: ghFor("v0.1.0-beta.2") });
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.equal(row(r.stdout, 6).result, "MANUAL");
  assert.match(
    row(r.stdout, 6).evidence,
    /fragments already removed, checked by the PM/,
  );
});

test("without the release branch on the remote every condition that reads it FAILs", (t) => {
  const repo = makeRepo();
  const origin = mkdtempSync(join(tmpdir(), "promotion-origin-"));
  t.after(() => {
    repo.dispose();
    rmSync(origin, { recursive: true, force: true });
  });
  repo.commit("start");
  spawnSync("git", ["init", "-q", "--bare", origin], { env: GIT_ENV });
  repo.run("remote", "add", "origin", origin);
  repo.run("push", "-q", "origin", "main");
  const r = run(repo, t, { gh: ghFor("v0.1.0-beta.1") });
  assert.equal(r.status, 1);
  for (const n of [4, 5, 6]) {
    assert.equal(row(r.stdout, n).result, "FAIL");
    assert.match(
      row(r.stdout, n).evidence,
      /origin\/release\/0\.1 does not exist/,
    );
  }
});

test("a version that is not X.Y.Z, or a --now that is not a date, is a usage error", (t) => {
  const repo = releaseRepo(t);
  for (const args of [
    ["0.1.0-beta.1"],
    [],
    ["0.1.0", "--now", "yesterday"],
    ["0.1.0", "--bogus"],
  ]) {
    const r = spawnSync(process.execPath, [script, ...args], {
      cwd: repo.dir,
      env: GIT_ENV,
      encoding: "utf8",
    });
    assert.equal(r.status, 2, args.join(" "));
    assert.match(r.stderr, /^usage: promotion-check/);
  }
});

test("rows whose command a seam overrides say so in their evidence, and the header names the seams", (t) => {
  const repo = releaseRepo(t);
  const r = run(repo, t, { gh: ghFor("v0.1.0-beta.2") });
  assert.match(
    row(r.stdout, 1).evidence,
    /^gh \(override\): v0\.1\.0-beta\.2 published/,
  );
  assert.match(
    row(r.stdout, 5).evidence,
    /^cargo audit \(override\): clean; pnpm audit \(override\): clean/,
  );
  assert.match(
    r.stdout,
    /^overrides in effect: PROMOTION_CHECK_GH, PROMOTION_CHECK_CARGO_AUDIT, PROMOTION_CHECK_PNPM_AUDIT, --now; this table is not evidence for a release$/m,
  );
});
