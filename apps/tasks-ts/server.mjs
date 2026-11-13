// Serves the example app on localhost, which browsers treat as a secure
// context, so OPFS is available: the page (index.html), the app's build
// output (dist/), its lock, and the SDK's build output under /sdk/.
//
//   node apps/tasks-ts/server.mjs [--port 8336]
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { APP, SDK } from "./build.mjs";

const FILES = {
  "/": join(APP, "index.html"),
  "/model.lock.json": join(APP, "model.lock.json"),
};

const ROOTS = [
  { prefix: "/sdk/", dir: SDK },
  { prefix: "/dist/", dir: join(APP, "dist") },
];

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json",
  ".wasm": "application/wasm",
};

/** The file a URL path names, or null when it names none the app serves. */
const resolve = (pathname) => {
  let decoded;
  try {
    decoded = decodeURIComponent(pathname);
  } catch {
    return null;
  }
  if (decoded.includes("\0")) return null;
  if (FILES[decoded]) return FILES[decoded];
  for (const { prefix, dir } of ROOTS) {
    if (!decoded.startsWith(prefix)) continue;
    const file = join(dir, decoded.slice(prefix.length));
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
export const startServer = ({ port = 8336, host = "127.0.0.1" } = {}) =>
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
    options: { port: { type: "string", default: "8336" } },
  });
  const { url } = await startServer({ port: Number(values.port) });
  console.log(`tasks: ${url}/`);
}
