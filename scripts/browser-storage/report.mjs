// Turns the results of a probe run (measure.mjs, run.mjs) into the rows of
// the table in docs/browser-storage.md. Pure functions, so the gate can hold
// the committed table to the committed results without starting a browser.

const ENGINES = {
  chromium: { name: "Chromium", private: "off-the-record context" },
  firefox: { name: "Firefox", private: "private browsing mode" },
  webkit: { name: "WebKit", private: "ephemeral context" },
};

const UNITS = ["B", "KiB", "MiB", "GiB", "TiB"];

const bytes = (value) => {
  if (typeof value !== "number") return "not reported";
  let size = value;
  let unit = 0;
  while (size >= 1024 && unit < UNITS.length - 1) {
    size /= 1024;
    unit += 1;
  }
  return `${Number(size.toFixed(1))} ${UNITS[unit]}`;
};

const both = ({ sqlite, idb }) => `SQLite ${sqlite}, IndexedDB ${idb}`;

const persistFinding = ({ answered, granted }) => {
  if (!answered) return "no answer within 5 s: the browser waits for the user";
  return `${granted ? "granted" : "denied"}, without asking the user`;
};

const privateFinding = ({ opfs, idb }) => {
  const sqlite =
    opfs === "ok" ? "SQLite on OPFS works" : `no SQLite on OPFS (${opfs})`;
  const control = idb === "ok" ? "IndexedDB works" : `IndexedDB fails (${idb})`;
  return `${sqlite}; ${control}`;
};

const afterCloseFinding = ({ opfs, afterClose }) =>
  opfs === "ok"
    ? both(afterClose)
    : `SQLite: no OPFS, IndexedDB ${afterClose.idb}`;

/** The table rows a run measured, in the order the doc lists them. */
export const measuredRows = (run) =>
  Object.entries(run.engines).flatMap(([id, result]) => {
    const engine = ENGINES[id];
    if (!engine) throw new Error(`unknown engine ${id}`);
    const browser = `${engine.name} (Playwright)`;
    const source = `measured: Playwright ${run.playwright}, ${engine.name} ${result.version}, ${run.platform}, ${run.date}`;
    return [
      ["`persist()`", persistFinding(result.persist)],
      ["`persisted()` after `persist()`", String(result.persisted)],
      [
        "`estimate()`",
        `usage ${bytes(result.estimate.usage)}, quota ${bytes(result.estimate.quota)}`,
      ],
      [
        "SQLite on OPFS (`opfs-sahpool`)",
        result.opfs === "ok" ? "works" : `fails (${result.opfs})`,
      ],
      ["Survives a reload", both(result.reload)],
      ["Survives a browser restart", both(result.restart)],
      ['Survives `Clear-Site-Data: "cache"`', both(result.clearCache)],
      [
        `Private window: storage (${engine.private})`,
        privateFinding(result.private),
      ],
      [
        `Private window: after close (${engine.private})`,
        afterCloseFinding(result.private),
      ],
    ].map(([behaviour, finding]) => ({ browser, behaviour, finding, source }));
  });

const cells = (line) =>
  line
    .trim()
    .replace(/^\|/, "")
    .replace(/\|$/, "")
    .split(/(?<!\\)\|/)
    .map((cell) => cell.trim().replaceAll("\\|", "|"));

/** Parses the rows of the first Markdown table under a heading. */
export const tableRows = (markdown, heading) => {
  const lines = markdown.split("\n");
  const start = lines.findIndex((line) =>
    new RegExp(`^#{1,6} ${heading}\\s*$`).test(line),
  );
  if (start < 0) throw new Error(`no heading "${heading}"`);
  let index = start + 1;
  while (index < lines.length && !lines[index].trim().startsWith("|")) {
    if (/^#{1,6} /.test(lines[index])) {
      throw new Error(`no table under "${heading}"`);
    }
    index += 1;
  }
  const header = cells(lines[index] ?? "").map((cell) => cell.toLowerCase());
  const rows = [];
  for (index += 2; lines[index]?.trim().startsWith("|"); index += 1) {
    const values = cells(lines[index]);
    rows.push(Object.fromEntries(header.map((key, i) => [key, values[i]])));
  }
  return rows;
};
