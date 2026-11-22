// Serves the page, its bundle and the SDK's build output (packages/sdk/dist,
// under /sdk/) on localhost, a secure context, so OPFS is there.
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { dirname, extname, join, sep } from "node:path";
import { fileURLToPath } from "node:url";

const SPIKE = join(dirname(fileURLToPath(import.meta.url)), "..");
export const ROOT = join(SPIKE, "..", "..", "..", "..");
const ROOTS = [
  { prefix: "/sdk/", dir: join(ROOT, "packages", "sdk", "dist") },
  { prefix: "/spike.js", dir: join(SPIKE, "dist"), file: "spike.js" },
  { prefix: "/", dir: join(SPIKE, "page"), file: "index.html" },
];
const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".wasm": "application/wasm",
};

const resolve = (pathname) => {
  for (const { prefix, dir, file } of ROOTS) {
    if (!pathname.startsWith(prefix)) continue;
    const rest = pathname.slice(prefix.length);
    const path = join(dir, rest && !file ? rest : (file ?? rest));
    return path.startsWith(dir + sep) ? path : null;
  }
  return null;
};

export const startServer = () =>
  new Promise((done) => {
    const server = createServer(async (req, res) => {
      const file = resolve(new URL(req.url, "http://localhost").pathname);
      const type = file && TYPES[extname(file)];
      if (!type) return res.writeHead(404).end();
      try {
        const body = await readFile(file);
        res.writeHead(200, {
          "content-type": type,
          "cache-control": "no-store",
        });
        res.end(body);
      } catch {
        res.writeHead(404).end();
      }
    });
    server.listen(0, "127.0.0.1", () =>
      done({
        url: `http://localhost:${server.address().port}`,
        close: () =>
          new Promise((c) => {
            server.closeAllConnections();
            server.close(() => c());
          }),
      }),
    );
  });
