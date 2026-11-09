// Runs the probe in every engine Playwright has, writes the results as JSON
// and prints them as rows of the findings table in docs/browser-storage.md.
//
//   node scripts/browser-storage/run.mjs [--out <file>] [--engines chromium,firefox,webkit]
//
// The committed results (docs/browser-storage.results.json) are the run the
// doc's "measured: Playwright" rows come from; scripts/browser-storage.test.mjs
// holds the two together. After a new run, paste the printed rows into the doc.
import { writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { arch, platform } from "node:os";
import { parseArgs } from "node:util";
import { measure } from "./measure.mjs";
import { measuredRows } from "./report.mjs";

const require = createRequire(import.meta.url);
const { values } = parseArgs({
  options: {
    out: { type: "string" },
    engines: { type: "string", default: "chromium,firefox,webkit" },
  },
});

const engines = {};
for (const engine of values.engines.split(",")) {
  console.error(`measuring ${engine}…`);
  engines[engine] = await measure(engine);
}

const run = {
  date: new Date().toISOString().slice(0, 10),
  platform: `${platform()} ${arch()}`,
  playwright: require("playwright/package.json").version,
  engines,
};

if (values.out)
  await writeFile(values.out, `${JSON.stringify(run, null, 2)}\n`);

const cell = (text) => text.replaceAll("|", "\\|");
console.log("| Browser | Behaviour | Finding | Source |");
console.log("| --- | --- | --- | --- |");
for (const row of measuredRows(run)) {
  console.log(
    `| ${[row.browser, row.behaviour, row.finding, row.source].map(cell).join(" | ")} |`,
  );
}
