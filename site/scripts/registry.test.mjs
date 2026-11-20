// Whether the registries serve the newest indexed version (NOC-373): npm
// stages a beta or release until the owner approves it, while commit-back
// indexes it at once, so the site asks npm for the version under its
// dist-tag, and crates.io for the version itself, before it shows a line that
// names them. A registry that cannot be reached, or answers anything but the
// version, counts as not serving it; the build never fails for it.
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { cachedProbe, servedVersions } from "../src/registry.mjs";

const NPM_TAGS =
  "https://registry.npmjs.org/-/package/@nocentraldb%2fsdk/dist-tags";
const CRATE = "https://crates.io/api/v1/crates/nocentraldb/0.1.0-beta.1";

/** A fetch that answers `routes` (url → [status, body]) and fails otherwise. */
const fixture = (routes) => {
  const asked = [];
  const fetchImpl = async (url, init) => {
    asked.push({ url, init });
    const route = routes[url];
    if (!route) throw new TypeError("fetch failed");
    const [status, body] = route;
    return new Response(typeof body === "string" ? body : JSON.stringify(body), {
      status,
    });
  };
  return { fetchImpl, asked };
};

const query = {
  npm: { distTag: "beta", version: "0.1.0-beta.1" },
  crate: "0.1.0-beta.1",
};

test("npm serves the version under its dist-tag, and crates.io serves it unyanked", async () => {
  const { fetchImpl, asked } = fixture({
    [NPM_TAGS]: [200, { latest: "0.0.0", beta: "0.1.0-beta.1" }],
    [CRATE]: [200, { version: { num: "0.1.0-beta.1", yanked: false } }],
  });
  assert.deepEqual(await servedVersions(query, fetchImpl), {
    npm: true,
    crate: true,
  });
  // crates.io refuses requests without a User-Agent.
  const crates = asked.find((a) => a.url === CRATE);
  assert.match(crates.init.headers["User-Agent"], /nocentraldb/);
});

test("npm still stages the version: its dist-tag names an older one, or none", async () => {
  for (const tags of [
    { latest: "0.0.0", beta: "0.1.0-alpha.73" },
    { latest: "0.0.0", alpha: "0.1.0-alpha.73" },
  ]) {
    const { fetchImpl } = fixture({
      [NPM_TAGS]: [200, tags],
      [CRATE]: [200, { version: { num: "0.1.0-beta.1", yanked: false } }],
    });
    assert.deepEqual(await servedVersions(query, fetchImpl), {
      npm: false,
      crate: true,
    });
  }
});

test("a release is served under npm's dist-tag latest", async () => {
  const { fetchImpl } = fixture({
    [NPM_TAGS]: [200, { latest: "0.1.0", beta: "0.1.0-beta.3" }],
  });
  assert.deepEqual(
    await servedVersions(
      { npm: { distTag: "latest", version: "0.1.0" }, crate: null },
      fetchImpl,
    ),
    { npm: true, crate: false },
  );
});

test("crates.io does not serve a version it does not know, or one that is yanked", async () => {
  for (const route of [
    [404, { errors: [{ detail: "not found" }] }],
    [200, { version: { num: "0.1.0-beta.1", yanked: true } }],
  ]) {
    const { fetchImpl } = fixture({
      [NPM_TAGS]: [200, { beta: "0.1.0-beta.1" }],
      [CRATE]: route,
    });
    assert.deepEqual(await servedVersions(query, fetchImpl), {
      npm: true,
      crate: false,
    });
  }
});

test("a registry that cannot be reached, or answers no JSON, serves nothing", async () => {
  assert.deepEqual(await servedVersions(query, fixture({}).fetchImpl), {
    npm: false,
    crate: false,
  });
  const { fetchImpl } = fixture({
    [NPM_TAGS]: [503, "<html>unavailable</html>"],
    [CRATE]: [200, "not json"],
  });
  assert.deepEqual(await servedVersions(query, fetchImpl), {
    npm: false,
    crate: false,
  });
});

test("nothing to ask: no request", async () => {
  const { fetchImpl, asked } = fixture({});
  assert.deepEqual(
    await servedVersions({ npm: null, crate: null }, fetchImpl),
    { npm: false, crate: false },
  );
  assert.equal(asked.length, 0);
});

// One build reads the lines several times (the landing page, the docs, their
// Markdown copies) and its output check once more: each reads the same
// answer, kept for an hour in a file, so a registry that changes during the
// build cannot give two pages two lines.
test("the probe keeps an answer per query for an hour, and serves nothing when it fails", () => {
  const file = join(mkdtempSync(join(tmpdir(), "ncdb-registry-")), "c.json");
  let now = 0;
  const runs = [];
  const run = (q) => {
    runs.push(q);
    return { npm: true, crate: runs.length > 1 };
  };
  const probe = cachedProbe({ file, run, now: () => now });
  assert.deepEqual(probe(query), { npm: true, crate: false });
  now = 59 * 60_000;
  assert.deepEqual(probe(query), { npm: true, crate: false });
  assert.equal(runs.length, 1);
  assert.deepEqual(probe({ ...query, crate: null }), {
    npm: true,
    crate: true,
  });
  assert.equal(runs.length, 2);
  now = 61 * 60_000;
  probe(query);
  assert.equal(runs.length, 3);

  const failing = cachedProbe({
    file: join(mkdtempSync(join(tmpdir(), "ncdb-registry-")), "c.json"),
    run: () => {
      throw new Error("timed out");
    },
  });
  assert.deepEqual(failing(query), { npm: false, crate: false });

  writeFileSync(file, "garbage");
  assert.deepEqual(cachedProbe({ file, run })(query), {
    npm: true,
    crate: true,
  });
  assert.ok(JSON.parse(readFileSync(file, "utf8")));
});
