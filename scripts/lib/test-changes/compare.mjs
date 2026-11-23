// Compares the tests of a pull request's changed files at its merge base and
// at its head (NOC-508). Every comparison runs over all changed files at
// once, so a test moved from one file to another is neither removed nor
// added.

/** Counts of `key(item)` over the items of every file. */
function tally(files, pick, key) {
  const counts = new Map();
  for (const { path, scan } of files) {
    for (const item of pick(scan)) {
      const k = key(item);
      const entry = counts.get(k) ?? { count: 0, at: [] };
      entry.count += 1;
      entry.at.push({ path, ...item });
      counts.set(k, entry);
    }
  }
  return counts;
}

/** The items of `base` that `head` holds fewer of, each with where it was. */
function lost(base, head) {
  const out = [];
  for (const [k, { count, at }] of base) {
    const left = head.get(k)?.count ?? 0;
    out.push(...at.slice(left, count).map((item) => ({ key: k, ...item })));
  }
  return out;
}

/**
 * The findings for a pull request: `base` and `head` are lists of
 * `{ path, scan }` for the changed files as they stand at each side.
 */
export function compare(base, head) {
  const findings = [];
  const tests = (s) => s.tests;
  const name = (t) => t.name;
  for (const t of lost(tally(base, tests, name), tally(head, tests, name))) {
    findings.push({
      path: t.path,
      line: t.line,
      text: `removes the test \`${t.name}\``,
    });
  }
  return findings;
}
