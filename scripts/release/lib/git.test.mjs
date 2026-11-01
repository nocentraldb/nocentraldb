import assert from "node:assert/strict";
import { test } from "node:test";
import * as lib from "./git.mjs";
import { GIT_ENV, makeRepo } from "./test-repo.mjs";

const repoFor = (t) => {
  const repo = makeRepo();
  t.after(() => repo.dispose());
  return { repo, opts: { cwd: repo.dir, env: GIT_ENV } };
};

test("zList splits -z output and drops the trailing empty entry", () => {
  assert.deepEqual(lib.zList("a b\0c\nd\0"), ["a b", "c\nd"]);
  assert.deepEqual(lib.zList(""), []);
});

test("git throws with the command and git's stderr in the message", (t) => {
  const { opts } = repoFor(t);
  assert.throws(
    () => lib.git(["rev-parse", "--verify", "nope"], opts),
    /^Error: git rev-parse --verify nope failed: fatal: /,
  );
});

test("tags lists the tags that match the pattern", (t) => {
  const { repo, opts } = repoFor(t);
  repo.commit("one", { a: "1\n" });
  repo.tag("v0.1.0-alpha.1");
  repo.tag("v0.1.0-alpha.2");
  repo.tag("other");
  assert.deepEqual(lib.tags("v*-alpha.*", opts), [
    "v0.1.0-alpha.1",
    "v0.1.0-alpha.2",
  ]);
  assert.deepEqual(lib.tags("v9*", opts), []);
});

test("describeNearest names the nearest reachable matching tag", (t) => {
  const { repo, opts } = repoFor(t);
  const first = repo.commit("one", { a: "1\n" });
  repo.tag("v0.1.0-alpha.1");
  repo.commit("two", { a: "2\n" });
  repo.tag("v0.1.0-alpha.2");
  repo.tag("x-other");
  assert.equal(lib.describeNearest("v*", "HEAD", opts), "v0.1.0-alpha.2");
  assert.equal(lib.describeNearest("v*", first, opts), "v0.1.0-alpha.1");
});

test("describeNearest is null when no tag matches, and when there is none", (t) => {
  const { repo, opts } = repoFor(t);
  repo.commit("one", { a: "1\n" });
  assert.equal(lib.describeNearest("v*", "HEAD", opts), null);
  repo.tag("x-other");
  assert.equal(lib.describeNearest("v*", "HEAD", opts), null);
});

test("describeNearest still throws on a bad ref", (t) => {
  const { repo, opts } = repoFor(t);
  repo.commit("one", { a: "1\n" });
  assert.throws(() => lib.describeNearest("v*", "nope", opts), /failed/);
});

test("fileAt reads a file as it stands at the ref, and is null where it does not exist", (t) => {
  const { repo, opts } = repoFor(t);
  const first = repo.commit("one", { "d/a.md": "1\n" });
  repo.commit("two", { "d/a.md": "2\n", "d/b.md": "b\n" });
  assert.equal(lib.fileAt("HEAD", "d/a.md", opts), "2\n");
  assert.equal(lib.fileAt(first, "d/a.md", opts), "1\n");
  assert.equal(lib.fileAt(first, "d/b.md", opts), null);
  assert.equal(lib.fileAt(first, "nope.md", opts), null);
  assert.throws(() => lib.fileAt("nope", "d/a.md", opts), /failed/);
});

test("filesAt lists the entries directly in a directory at the ref, -z safe", (t) => {
  const { repo, opts } = repoFor(t);
  const first = repo.commit("one", { "d/a b.md": "1\n", "d/sub/c.md": "c\n" });
  repo.commit("two", { "d/z.md": "z\n" });
  assert.deepEqual(lib.filesAt(first, "d", opts), ["d/a b.md", "d/sub"]);
  assert.deepEqual(lib.filesAt("HEAD", "d", opts), [
    "d/a b.md",
    "d/sub",
    "d/z.md",
  ]);
  assert.deepEqual(lib.filesAt("HEAD", "none", opts), []);
});
