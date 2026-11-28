// An import map for the browser tests, as an app's bundler would resolve
// the modules (NOC-461): Yjs, an optional peer of the SDK, and the lib0
// modules it imports, mapped onto the repository's node_modules and served
// by Playwright's routing, so the page and the SDK share one Yjs.
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { ROOT } from "./build.mjs";

const require = createRequire(join(ROOT, "packages", "sdk", "package.json"));

/** The import map's entries for `yjs`, and the file behind each served URL. */
export const yjsModules = (() => {
  const served = new Map();
  const entries = {};
  const packageDir = (name, from) =>
    dirname(
      createRequire(from).resolve(`${name}/package.json`, {
        paths: [dirname(from)],
      }),
    );
  /** Serves the file at `path` under `url`, and each module it imports. */
  const file = (path, url) => {
    if (served.has(url)) return;
    served.set(url, path);
    for (const [, specifier] of readFileSync(path, "utf8").matchAll(
      /(?:\bfrom|\bimport)\s*['"]([^'"]+)['"]/g,
    )) {
      if (specifier.startsWith(".")) {
        file(
          join(dirname(path), specifier),
          new URL(specifier, `http://x${url}`).pathname,
        );
      } else {
        bare(specifier, path);
      }
    }
  };
  /** Maps the bare `specifier`, as `from` imports it, onto its package's file. */
  const bare = (specifier, from) => {
    if (entries[specifier]) return;
    const [scope, ...rest] = specifier.split("/");
    const dir = packageDir(scope, from);
    const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
    const key = rest.length ? `./${rest.join("/")}` : ".";
    // Conditions as a browser's bundler picks them.
    const pick = (entry) =>
      typeof entry === "string" || entry === undefined
        ? entry
        : pick(entry.browser ?? entry.import ?? entry.module ?? entry.default);
    const target = pick(pkg.exports?.[key]) ?? pkg.module;
    const url = `/modules/${scope}/${target.replace(/^\.\//, "")}`;
    entries[specifier] = url;
    file(join(dir, target), url);
  };
  bare("yjs", require.resolve("yjs"));
  return { served, entries };
})();

/** Serves the modules of the map under `${url}/modules/` in `tab`. */
export const routeModules = async (tab, url) => {
  await tab.route(`${url}/modules/**`, (route) => {
    const path = yjsModules.served.get(new URL(route.request().url()).pathname);
    if (!path) return route.fulfill({ status: 404 });
    return route.fulfill({
      body: readFileSync(path),
      contentType: "text/javascript; charset=utf-8",
    });
  });
};
