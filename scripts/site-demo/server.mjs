// Serves the built site (site/dist) on localhost, which browsers treat as
// a secure context, so OPFS and Web Locks are available to the demo
// (NOC-319). It serves each file with the content type GitHub Pages gives
// it, `application/wasm` for the core included, and a missing file as 404.
//
//   node scripts/site-demo/server.mjs [--port 8337]
import { readFile, stat } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

export const DIST = fileURLToPath(new URL("../../site/dist", import.meta.url));

export const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".json": "application/json",
  ".wasm": "application/wasm",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".ico": "image/x-icon",
  ".woff2": "font/woff2",
  ".txt": "text/plain; charset=utf-8",
  ".md": "text/markdown; charset=utf-8",
  ".xml": "application/xml",
  ".webmanifest": "application/manifest+json",
};

/** The file a URL path names in `root`, or null when it is outside it. */
const resolve = async (root, pathname) => {
  let decoded;
  try {
    decoded = decodeURIComponent(pathname);
  } catch {
    return null;
  }
  if (decoded.includes("\0")) return null;
  let file = join(root, decoded);
  if (file !== root && !file.startsWith(root + sep)) return null;
  try {
    if ((await stat(file)).isDirectory()) file = join(file, "index.html");
  } catch {
    return null;
  }
  return file;
};

/** Starts the server on `port` (0: any free port) for `root`. */
export const startServer = ({ port = 0, root = DIST } = {}) =>
  new Promise((done, failed) => {
    const server = createServer(async (request, response) => {
      const file = await resolve(
        root,
        new URL(request.url, "http://localhost").pathname,
      );
      try {
        if (file === null) throw new Error("not found");
        const body = await readFile(file);
        response.writeHead(200, {
          "content-type": TYPES[extname(file)] ?? "application/octet-stream",
          "cache-control": "no-store",
        });
        response.end(body);
      } catch {
        response.writeHead(404).end();
      }
    });
    server.on("error", failed);
    server.listen(port, "127.0.0.1", () => {
      const { port: bound } = server.address();
      done({
        url: `http://localhost:${bound}`,
        close: () => new Promise((closed) => server.close(closed)),
      });
    });
  });

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const { values } = parseArgs({ options: { port: { type: "string" } } });
  const server = await startServer({ port: Number(values.port ?? 8337) });
  console.log(`the site at ${server.url}/`);
}
