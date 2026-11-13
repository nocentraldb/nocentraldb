// Serves the page of the SDK's browser test (page/) and the package's build
// output (packages/sdk/dist/, under /sdk/) on localhost, which browsers
// treat as a secure context, so OPFS is available.
//
//   node scripts/sdk-browser/server.mjs [--port 8335]
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { dirname, extname, join, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { DIST } from "./build.mjs";

const PAGE_DIR = join(dirname(fileURLToPath(import.meta.url)), "page");

const ROOTS = [
  { prefix: "/sdk/", dir: DIST },
  { prefix: "/", dir: PAGE_DIR },
];

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
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
    const file = join(dir, decoded.slice(prefix.length) || "index.html");
    return file.startsWith(dir + sep) ? file : null;
  }
  return null;
};

const handle = async (request, response) => {
  const file = resolve(new URL(request.url, "http://localhost").pathname);
  const type = file && TYPES[extname(file)];
  if (!type) {
    response.writeHead(404).end();
    return;
  }
  try {
    const body = await readFile(file);
    response.writeHead(200, {
      "content-type": type,
      "cache-control": "no-store",
    });
    response.end(body);
  } catch {
    response.writeHead(404).end();
  }
};

/** Starts the server; resolves to { url, close }. Port 0 picks a free one. */
export const startServer = ({ port = 8335, host = "127.0.0.1" } = {}) =>
  new Promise((done, reject) => {
    const server = createServer((request, response) => {
      handle(request, response).catch(() => {
        if (!response.headersSent) response.writeHead(500);
        response.end();
      });
    });
    server.on("error", reject);
    server.listen(port, host, () => {
      done({
        url: `http://localhost:${server.address().port}`,
        close: () =>
          new Promise((closed) => {
            server.closeAllConnections();
            server.close(() => closed());
          }),
      });
    });
  });

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const { values } = parseArgs({
    options: { port: { type: "string", default: "8335" } },
  });
  const { url } = await startServer({ port: Number(values.port) });
  console.log(`SDK browser page: ${url}/`);
}
