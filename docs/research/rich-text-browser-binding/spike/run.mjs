// Runs the NOC-455 spike in Chromium: the echo, the three measures with IME,
// paste and undo, and the latency over the TypeScript SDK and the local
// relay. Prints output.txt and writes results.json.
//
// Needs the SDK and the relay built first, from the repository's root:
//   node scripts/sdk-browser/build.mjs
import { writeFileSync } from "node:fs";
import { launch } from "./lib/browser.mjs";
import { runEcho } from "./lib/echo.mjs";
import { runLatency } from "./lib/latency.mjs";
import { runMeasures } from "./lib/measures.mjs";
import { startServer } from "./lib/server.mjs";

const server = await startServer();
const browser = await launch();
const results = {
  engine: `chromium ${browser.version()}`,
  echo: await runEcho(browser, server.url),
  measures: await runMeasures(browser, server.url),
  latency: await runLatency(browser, server.url),
};
await browser.close();
await server.close();

writeFileSync("results.json", `${JSON.stringify(results, null, 2)}\n`);

const lines = [`engine: ${results.engine}`];
for (const part of ["echo", "measures", "latency"]) {
  lines.push("", `== ${part}`);
  for (const [name, value] of Object.entries(results[part])) {
    lines.push(`-- ${name}`);
    for (const [k, v] of Object.entries(value))
      lines.push(`   ${k}: ${JSON.stringify(v)}`);
  }
}
const text = `${lines.join("\n")}\n`;
writeFileSync("output.txt", text);
process.stdout.write(text);
