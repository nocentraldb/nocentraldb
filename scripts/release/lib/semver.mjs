// SemVer as the release tooling uses it (docs/release-management.md,
// "Versioning"): X.Y.Z, optionally with a stage suffix -alpha.N or -beta.N.
// No other prerelease name and no build metadata. Tags are the version with a
// leading v.

/** A full version: X.Y.Z or X.Y.Z-alpha.N or X.Y.Z-beta.N. */
const VERSION = /^(\d+)\.(\d+)\.(\d+)(?:-(alpha|beta)\.(\d+))?$/;
/** A plain version, as committed manifests hold it. */
export const PLAIN = /^\d+\.\d+\.\d+$/;
/** A version tag. */
const TAG = /^v(\d+\.\d+\.\d+(?:-(?:alpha|beta)\.\d+)?)$/;

const STAGE = { alpha: 0, beta: 1, release: 2 };

/** Parses a full version; throws on anything VERSION does not match. */
export function parseVersion(s) {
  const m = VERSION.exec(s);
  if (!m)
    throw new Error(`not a version: "${s}" (expected X.Y.Z[-alpha.N|-beta.N])`);
  return {
    major: Number(m[1]),
    minor: Number(m[2]),
    patch: Number(m[3]),
    stage: m[4] ?? "release",
    n: m[5] === undefined ? null : Number(m[5]),
  };
}

export const plain = (v) => `${v.major}.${v.minor}.${v.patch}`;

/** Orders two parsed versions: major.minor.patch, then stage, then n. */
export function compare(a, b) {
  const keys = [
    [a.major, b.major],
    [a.minor, b.minor],
    [a.patch, b.patch],
    [STAGE[a.stage], STAGE[b.stage]],
    [a.n ?? 0, b.n ?? 0],
  ];
  for (const [x, y] of keys) if (x !== y) return x < y ? -1 : 1;
  return 0;
}

/** The parsed version of a version tag, or null for any other tag. */
export function classifyTag(tag) {
  const m = TAG.exec(tag);
  if (!m) return null;
  return { version: m[1], ...parseVersion(m[1]) };
}

/** Raises one part of a version; the result is a release version. */
export function bump(v, part) {
  const { major, minor, patch } = v;
  const release = { stage: "release", n: null };
  if (part === "major")
    return { major: major + 1, minor: 0, patch: 0, ...release };
  if (part === "minor")
    return { major, minor: minor + 1, patch: 0, ...release };
  if (part === "patch") return { major, minor, patch: patch + 1, ...release };
  throw new Error(`unknown part: "${part}" (expected major, minor or patch)`);
}

/**
 * The version after `version` for the given fragment types: breaking raises
 * the major from 1.0 on and the minor before it, feature the minor, anything
 * else (or nothing) the patch.
 */
export function bumpFor(version, types) {
  const v = parseVersion(version);
  let part = "patch";
  if (types.includes("breaking")) part = v.major >= 1 ? "major" : "minor";
  else if (types.includes("feature")) part = "minor";
  return plain(bump(v, part));
}
