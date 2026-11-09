// Serves the probe page (page/) and the SQLite WebAssembly build
// (@sqlite.org/sqlite-wasm's dist/, under /sqlite/) on localhost, which
// browsers treat as a secure context, so OPFS is available. /clear-cache
// answers with `Clear-Site-Data: "cache"`, which asks the browser to drop
// its HTTP cache for the origin and keep site data.
//
//   node scripts/browser-storage/server.mjs [--port 8333] [--host 127.0.0.1]
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { dirname, extname, join, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const here = dirname(fileURLToPath(import.meta.url));
const require = createRequire(import.meta.url);

export const PAGE_DIR = join(here, "page");
export const SQLITE_DIR = dirname(
  require.resolve("@sqlite.org/sqlite-wasm/package.json"),
);

const ROOTS = [
  { prefix: "/sqlite/", dir: join(SQLITE_DIR, "dist") },
  { prefix: "/", dir: PAGE_DIR },
];

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".wasm": "application/wasm",
};

/** The file a URL path names, or null when it lies outside every root. */
const resolve = (pathname) => {
  let decoded;
  try {
    decoded = decodeURIComponent(pathname);
  } catch {
    return null;
  }
  if (decoded.includes("\0")) return null;
  for (const { prefix, dir } of ROOTS) {
    if (!decoded.startsWith(prefix)) continue;
    const rest = decoded.slice(prefix.length) || "index.html";
    const file = join(dir, rest);
    return file.startsWith(dir + sep) ? file : null;
  }
  return null;
};

const handle = async (request, response) => {
  const { pathname } = new URL(request.url, "http://localhost");
  if (pathname === "/clear-cache") {
    response.writeHead(200, {
      "content-type": "text/html; charset=utf-8",
      "clear-site-data": '"cache"',
      "cache-control": "no-store",
    });
    response.end("<!doctype html><title>cache cleared</title>cache cleared");
    return;
  }
  const file = resolve(pathname);
  const type = file && TYPES[extname(file)];
  if (!type) {
    response.writeHead(404).end();
    return;
  }
  try {
    const body = await readFile(file);
    response.writeHead(200, { "content-type": type });
    response.end(body);
  } catch {
    response.writeHead(404).end();
  }
};

/** Starts the server; resolves to { url, close }. Port 0 picks a free one. */
export const startServer = ({ port = 8333, host = "127.0.0.1" } = {}) =>
  new Promise((resolvePromise, reject) => {
    const server = createServer((request, response) => {
      handle(request, response).catch(() => {
        if (!response.headersSent) response.writeHead(500);
        response.end();
      });
    });
    server.on("error", reject);
    server.listen(port, host, () => {
      const address = server.address();
      resolvePromise({
        url: `http://localhost:${address.port}`,
        close: () =>
          new Promise((done) => {
            server.closeAllConnections();
            server.close(() => done());
          }),
      });
    });
  });

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const { values } = parseArgs({
    options: {
      port: { type: "string", default: "8333" },
      host: { type: "string", default: "127.0.0.1" },
    },
  });
  const { url } = await startServer({
    port: Number(values.port),
    host: values.host,
  });
  console.log(`probe page: ${url}/`);
}
