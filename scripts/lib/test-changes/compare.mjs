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
 * An assertion whose text is gone at head is either a changed expected value,
 * when an assertion new at head has its skeleton, or removed or rewritten.
 */
function assertionFindings(base, head) {
  const pick = (s) => s.assertions;
  const text = (a) => a.text;
  const gone = lost(tally(base, pick, text), tally(head, pick, text));
  const added = lost(tally(head, pick, text), tally(base, pick, text));
  return gone.map((a) => {
    const i = added.findIndex((b) => b.skeleton === a.skeleton);
    if (i === -1) {
      return { ...a, text: `removes or rewrites the assertion \`${a.text}\`` };
    }
    const [now] = added.splice(i, 1);
    return {
      ...a,
      text: `changes an expected value: \`${a.text}\` became \`${now.text}\``,
    };
  });
}

/**
 * The findings for a pull request: `base` and `head` are lists of
 * `{ path, scan }` for the changed files as they stand at each side.
 */
export function compare(base, head) {
  const findings = [];
  const counts = (pick, key) => [
    tally(base, pick, key),
    tally(head, pick, key),
  ];
  const name = (t) => t.name;
  const [testsBefore, testsAfter] = counts((s) => s.tests, name);
  for (const t of lost(testsBefore, testsAfter)) {
    findings.push({ ...t, text: `removes the test \`${t.name}\`` });
  }
  const [ignoredBefore, ignoredAfter] = counts((s) => s.ignored, name);
  for (const t of lost(ignoredAfter, ignoredBefore)) {
    findings.push({ ...t, text: `ignores the test \`${t.name}\`` });
  }
  const [markersBefore, markersAfter] = counts(
    (s) => s.markers,
    (m) => m.key,
  );
  for (const m of lost(markersAfter, markersBefore)) {
    findings.push({ ...m, text: `adds \`${m.key}\`` });
  }
  findings.push(...assertionFindings(base, head));
  return findings;
}
