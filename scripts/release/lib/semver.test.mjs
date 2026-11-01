import assert from "node:assert/strict";
import { test } from "node:test";
import {
  bump,
  bumpFor,
  classifyTag,
  compare,
  PLAIN,
  parseVersion,
  plain,
} from "./semver.mjs";

test("parseVersion reads a plain, an alpha and a beta version", () => {
  assert.deepEqual(parseVersion("0.1.0"), {
    major: 0,
    minor: 1,
    patch: 0,
    stage: "release",
    n: null,
  });
  assert.deepEqual(parseVersion("0.1.0-alpha.3"), {
    major: 0,
    minor: 1,
    patch: 0,
    stage: "alpha",
    n: 3,
  });
  assert.deepEqual(parseVersion("1.2.3-beta.10"), {
    major: 1,
    minor: 2,
    patch: 3,
    stage: "beta",
    n: 10,
  });
});

test("parseVersion rejects every other shape, naming it", () => {
  for (const bad of ["v0.1.0", "0.1", "0.1.0-rc.1", "0.1.0-alpha", ""])
    assert.throws(
      () => parseVersion(bad),
      new RegExp(`not a version: "${bad.replace(/\./g, "\\.")}"`),
    );
});

test("plain drops the stage and PLAIN matches only X.Y.Z", () => {
  assert.equal(plain(parseVersion("1.2.3-beta.4")), "1.2.3");
  assert.ok(PLAIN.test("1.2.3"));
  assert.ok(!PLAIN.test("1.2.3-beta.4"));
  assert.ok(!PLAIN.test("1.2.3+build"));
});

test("compare orders numerically, then alpha < beta < release, then n", () => {
  const ordered = [
    "0.1.0-alpha.2",
    "0.1.0-alpha.10",
    "0.1.0-beta.1",
    "0.1.0",
    "0.1.1-alpha.1",
  ];
  const shuffled = [ordered[3], ordered[1], ordered[4], ordered[0], ordered[2]];
  const sorted = shuffled
    .map(parseVersion)
    .sort(compare)
    .map((v) =>
      v.stage === "release" ? plain(v) : `${plain(v)}-${v.stage}.${v.n}`,
    );
  assert.deepEqual(sorted, ordered);
  assert.equal(compare(parseVersion("0.10.0"), parseVersion("0.9.0")), 1);
  assert.equal(compare(parseVersion("1.0.0"), parseVersion("1.0.0")), 0);
  assert.equal(compare(parseVersion("1.0.0"), parseVersion("2.0.0")), -1);
});

test("classifyTag reads a version tag and returns null for any other shape", () => {
  assert.deepEqual(classifyTag("v0.1.0-beta.2"), {
    version: "0.1.0-beta.2",
    major: 0,
    minor: 1,
    patch: 0,
    stage: "beta",
    n: 2,
  });
  assert.equal(classifyTag("v0.1.0").stage, "release");
  assert.equal(classifyTag("0.1.0"), null);
  assert.equal(classifyTag("v0.1.0-rc1"), null);
  assert.equal(classifyTag("v0.1.0-alpha"), null);
  assert.equal(classifyTag("release/0.1"), null);
});

test("bump raises one part and resets the lower ones", () => {
  const v = parseVersion("1.2.3-beta.1");
  assert.deepEqual(bump(v, "major"), {
    major: 2,
    minor: 0,
    patch: 0,
    stage: "release",
    n: null,
  });
  assert.equal(plain(bump(v, "minor")), "1.3.0");
  assert.equal(plain(bump(v, "patch")), "1.2.4");
  assert.throws(() => bump(v, "huge"), /unknown part/);
});

test("bumpFor: breaking is minor before 1.0 and major from 1.0, feature minor, else patch", () => {
  assert.equal(bumpFor("0.3.0", ["breaking"]), "0.4.0");
  assert.equal(bumpFor("1.3.0", ["breaking"]), "2.0.0");
  assert.equal(bumpFor("1.3.0", ["feature", "fix"]), "1.4.0");
  assert.equal(bumpFor("1.3.0", ["fix", "internal"]), "1.3.1");
  assert.equal(bumpFor("1.3.0", []), "1.3.1");
});
