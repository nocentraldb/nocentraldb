import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { GIT_ENV, makeRepo } from "./lib/test-repo.mjs";

const script = join(dirname(fileURLToPath(import.meta.url)), "verify-tag.mjs");

const cargo = (version) =>
  `[workspace]\nmembers = []\n\n[workspace.package]\nversion = "${version}"\n`;
const FEATURE =
  "---\ntype: feature\ntickets: [mt08.aaaa]\ncomponents: [core]\n---\n## Problem\na\n## Use\nb\n";
const NOTES = "# NoCentralDB 0.1.0\n\nThe first release.\n";

/**
 * A repository with a bare remote `origin`. `push(...refs)` pushes them and
 * fetches, so origin/<branch> is a real remote-tracking ref.
 */
const repoWithRemote = (t) => {
  const repo = makeRepo();
  const bare = mkdtempSync(join(tmpdir(), "release-remote-"));
  t.after(() => {
    repo.dispose();
    rmSync(bare, { recursive: true, force: true });
  });
  execFileSync("git", ["init", "-q", "--bare", bare], { env: GIT_ENV });
  repo.run("remote", "add", "origin", bare);
  return {
    ...repo,
    push(...refs) {
      repo.run("push", "-q", "origin", ...refs);
      repo.run("fetch", "-q", "origin");
    },
  };
};

/**
 * main with Cargo.toml at `version`, one fragment and the curated notes,
 * release/0.1 cut from it and pushed. Returns the repository and the cut.
 */
const cutRepo = (t, { version = "0.1.0", notes = true } = {}) => {
  const repo = repoWithRemote(t);
  repo.commit("start", {
    "Cargo.toml": cargo(version),
    "changes/README.md": "# guide\n",
    "changes/add-x.md": FEATURE,
    "src/lib.rs": "// one\n",
  });
  const cut = repo.commit(
    "notes",
    notes ? { "release-notes/0.1.0.md": NOTES } : {},
  );
  repo.run("branch", "release/0.1");
  repo.push("main", "release/0.1");
  return { repo, cut };
};

/** cutRepo, beta.1 on the cut, then the final notes commit on the branch, pushed. */
const finalNotesRepo = (t) => {
  const { repo, cut } = cutRepo(t);
  repo.tag("v0.1.0-beta.1", cut);
  repo.run("switch", "-q", "release/0.1");
  repo.remove("changes/add-x.md");
  const head = repo.commit("final notes", {
    "release-notes/0.1.0.md": `${NOTES}\nFinal.\n`,
  });
  repo.push("release/0.1");
  return { repo, cut, head };
};

const run = (repo, tag, commit, ...rest) =>
  spawnSync(
    process.execPath,
    [script, "--tag", tag, "--commit", commit, ...rest],
    { cwd: repo.dir, env: GIT_ENV, encoding: "utf8" },
  );

/** Asserts the run failed with exactly one line: verify-tag: <message>. */
const refused = (r, message) => {
  assert.equal(r.status, 1, r.stdout);
  assert.equal(r.stdout, "");
  assert.equal(r.stderr, `verify-tag: ${message}\n`);
};

test("a beta on the head of release/X.Y with matching manifests and its notes passes", (t) => {
  const { repo, cut } = cutRepo(t);
  repo.tag("v0.1.0-beta.1", cut);
  const r = run(repo, "v0.1.0-beta.1", cut, "--json");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stderr, "");
  assert.deepEqual(JSON.parse(r.stdout), {
    stage: "beta",
    version: "0.1.0",
    lastBeta: null,
  });
});

test("without --json a passing tag prints one line", (t) => {
  const { repo, cut } = cutRepo(t);
  const r = run(repo, "v0.1.0-beta.1", cut);
  assert.equal(r.status, 0, r.stderr);
  assert.equal(
    r.stdout,
    "verify-tag: v0.1.0-beta.1 may be published (beta of 0.1.0)\n",
  );
});

test("a later beta reports the previous beta of the same version as lastBeta", (t) => {
  const { repo, cut } = cutRepo(t);
  repo.tag("v0.1.0-beta.1", cut);
  repo.tag("v0.0.9-beta.7", cut);
  repo.run("switch", "-q", "release/0.1");
  const head = repo.commit("fix", { "src/lib.rs": "// two\n" });
  repo.push("release/0.1");
  repo.tag("v0.1.0-beta.2", head);
  const r = run(repo, "v0.1.0-beta.2", head, "--json");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(JSON.parse(r.stdout).lastBeta, "v0.1.0-beta.1");
});

test("a release on the branch head after a beta, with only notes and fragments changed, passes", (t) => {
  const { repo, head } = finalNotesRepo(t);
  const r = run(repo, "v0.1.0", head, "--json");
  assert.equal(r.status, 0, r.stderr);
  assert.deepEqual(JSON.parse(r.stdout), {
    stage: "release",
    version: "0.1.0",
    lastBeta: "v0.1.0-beta.1",
  });
});

test("V1: an unknown tag shape is refused", (t) => {
  const { repo, cut } = cutRepo(t);
  refused(
    run(repo, "v0.1.0-rc1", cut),
    "unknown tag shape v0.1.0-rc1; expected vX.Y.Z or vX.Y.Z-beta.N",
  );
});

test("V1: an alpha tag is refused", (t) => {
  const { repo, cut } = cutRepo(t);
  refused(
    run(repo, "v0.1.0-alpha.1", cut),
    "v0.1.0-alpha.1: alpha tags are not published by this job",
  );
});

test("V2: a tag whose X.Y.Z differs from the manifests at the commit is refused", (t) => {
  const { repo, cut } = cutRepo(t, { version: "0.2.0" });
  refused(
    run(repo, "v0.1.0-beta.1", cut),
    `v0.1.0-beta.1: version 0.1.0 is not the workspace version 0.2.0 in Cargo.toml at ${cut}`,
  );
});

test("V2: a commit without Cargo.toml is refused", (t) => {
  const { repo } = cutRepo(t);
  repo.run("rm", "-q", "Cargo.toml");
  const bare = repo.commit("no manifest");
  refused(
    run(repo, "v0.1.0-beta.1", bare),
    `v0.1.0-beta.1: Cargo.toml does not exist at ${bare}`,
  );
});

test("V3: a beta tag that is not reachable from origin/release/X.Y is refused", (t) => {
  const { repo } = cutRepo(t);
  const onMain = repo.commit("later on main", { "src/lib.rs": "// main\n" });
  refused(
    run(repo, "v0.1.0-beta.1", onMain),
    "v0.1.0-beta.1 is not on release/0.1",
  );
});

test("V3: a tag whose release branch does not exist on the remote is refused", (t) => {
  const { repo, cut } = cutRepo(t);
  repo.run("push", "-q", "origin", "--delete", "release/0.1");
  repo.run("fetch", "-q", "--prune", "origin");
  refused(
    run(repo, "v0.1.0-beta.1", cut),
    "v0.1.0-beta.1 is not on release/0.1: origin/release/0.1 does not exist",
  );
});

test("V3: --remote names the remote the branch is read from", (t) => {
  const { repo, cut } = cutRepo(t);
  const r = run(repo, "v0.1.0-beta.1", cut, "--remote", "upstream");
  refused(
    r,
    "v0.1.0-beta.1 is not on release/0.1: upstream/release/0.1 does not exist",
  );
});

test("V4: a release tag without any beta of its version is refused", (t) => {
  const { repo, cut } = cutRepo(t);
  repo.tag("v0.1.1-beta.1", cut);
  repo.tag("v0.1.0-alpha.3", cut);
  refused(
    run(repo, "v0.1.0", cut),
    "no beta of 0.1.0 exists; every release passes through beta",
  );
});

test("V5: a release tag that is not the head of release/X.Y is refused", (t) => {
  const { repo, head } = finalNotesRepo(t);
  const later = repo.commit("later notes", {
    "release-notes/0.1.0.md": `${NOTES}\nLater.\n`,
  });
  repo.push("release/0.1");
  refused(
    run(repo, "v0.1.0", head),
    `a release is tagged on the head of release/0.1, which is ${later}`,
  );
});

test("V6: a release with product code changed since the last beta is refused, naming the paths", (t) => {
  const { repo } = finalNotesRepo(t);
  const head = repo.commit("sneaky", {
    "src/lib.rs": "// changed\n",
    "src/new file.rs": "// new\n",
  });
  repo.push("release/0.1");
  refused(
    run(repo, "v0.1.0", head),
    "since v0.1.0-beta.1 the following paths changed outside release-notes/ and changes/: src/lib.rs, src/new file.rs",
  );
});

test("V6: the last beta is the highest beta.N of the same version, numerically", (t) => {
  const { repo, cut } = cutRepo(t);
  repo.tag("v0.1.0-beta.2", cut);
  repo.run("switch", "-q", "release/0.1");
  const fixed = repo.commit("fix", { "src/lib.rs": "// fixed\n" });
  repo.tag("v0.1.0-beta.10", fixed);
  repo.tag("v0.1.1-beta.99", cut);
  repo.tag("v0.2.0-beta.1", cut);
  repo.remove("changes/add-x.md");
  const head = repo.commit("final notes", {
    "release-notes/0.1.0.md": `${NOTES}\nFinal.\n`,
  });
  repo.push("release/0.1");
  const r = run(repo, "v0.1.0", head, "--json");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(JSON.parse(r.stdout).lastBeta, "v0.1.0-beta.10");
});

test("V7: a release while changes/ still holds a fragment is refused, naming them", (t) => {
  const { repo, cut } = cutRepo(t);
  repo.tag("v0.1.0-beta.1", cut);
  repo.run("switch", "-q", "release/0.1");
  const head = repo.commit("notes only", {
    "release-notes/0.1.0.md": `${NOTES}\nFinal.\n`,
    "changes/cherry.md": FEATURE,
  });
  repo.push("release/0.1");
  refused(
    run(repo, "v0.1.0", head),
    "2 fragments are still in changes/ at v0.1.0; the final notes PR removes them: changes/add-x.md, changes/cherry.md",
  );
});

test("V8: a beta without release-notes/X.Y.Z.md at the commit is refused", (t) => {
  const { repo, cut } = cutRepo(t, { notes: false });
  refused(
    run(repo, "v0.1.0-beta.1", cut),
    "release-notes/0.1.0.md does not exist at v0.1.0-beta.1",
  );
});

test("V8: a release without release-notes/X.Y.Z.md at the commit is refused", (t) => {
  const { repo } = finalNotesRepo(t);
  repo.run("rm", "-q", "release-notes/0.1.0.md");
  const gone = repo.commit("notes gone");
  repo.push("release/0.1");
  refused(
    run(repo, "v0.1.0", gone),
    "release-notes/0.1.0.md does not exist at v0.1.0",
  );
});

test("the checks run in order and stop at the first failure", (t) => {
  // Wrong manifests (V2), not on the branch (V3), no beta (V4), no notes (V8):
  // only V2 is reported.
  const { repo } = cutRepo(t, { version: "0.2.0", notes: false });
  const onMain = repo.commit("later on main");
  const r = run(repo, "v0.1.0", onMain);
  refused(
    r,
    `v0.1.0: version 0.1.0 is not the workspace version 0.2.0 in Cargo.toml at ${onMain}`,
  );
});

test("a commit that does not exist is refused, naming it", (t) => {
  const { repo } = cutRepo(t);
  const r = run(repo, "v0.1.0-beta.1", "deadbeef");
  assert.equal(r.status, 1);
  assert.equal(r.stdout, "");
  assert.match(r.stderr, /^verify-tag: .*deadbeef/);
});

test("--help prints the usage; missing arguments are usage errors", (t) => {
  const { repo, cut } = cutRepo(t);
  const help = spawnSync(process.execPath, [script, "--help"], {
    cwd: repo.dir,
    env: GIT_ENV,
    encoding: "utf8",
  });
  assert.equal(help.status, 0);
  assert.match(help.stdout, /^usage: verify-tag/);
  for (const args of [
    ["--tag", "v0.1.0-beta.1"],
    ["--commit", cut],
    ["--tag", "v0.1.0-beta.1", "--commit", cut, "--nope"],
  ]) {
    const r = spawnSync(process.execPath, [script, ...args], {
      cwd: repo.dir,
      env: GIT_ENV,
      encoding: "utf8",
    });
    assert.equal(r.status, 2, args.join(" "));
    assert.match(r.stderr, /^usage: verify-tag/, args.join(" "));
  }
});
