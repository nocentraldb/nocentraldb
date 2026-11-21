// NOC-437 spike. Run with `npm ci && npm run spike` in this folder.
// Prints what each editor binding does with parts of a document its schema
// cannot interpret, and writes the same as results.json.
import { writeFileSync } from "node:fs";
import {
  shown,
  tiptap,
  typeAt,
  yProsemirror,
  yTiptap,
} from "./lib/editors.mjs";
import { runRemedies } from "./lib/remedies.mjs";
import { runScenario } from "./lib/scenario.mjs";
import { oldSchema } from "./lib/setup.mjs";

const wrap = (make) => (doc) => {
  const ed = make(doc);
  return {
    shown: () => shown(ed.view),
    type: (prefix, text) => typeAt(ed.view, prefix, text),
    errors: () => ed.errors ?? [],
    destroy: ed.destroy,
  };
};

const configs = {
  "y-prosemirror 1.3.7": wrap((doc) => yProsemirror(doc, oldSchema)),
  "y-tiptap 3.3.1, no onInvalidContent": wrap((doc) => yTiptap(doc, oldSchema)),
  "y-tiptap 3.3.1, onInvalidContent": wrap((doc) => {
    const errors = [];
    const ed = yTiptap(doc, oldSchema, {
      onInvalidContent: (e) => errors.push(e.message),
    });
    return { ...ed, errors };
  }),
  "TipTap 3.35.0, enableContentCheck false": wrap((doc) =>
    tiptap(doc, { enableContentCheck: false }),
  ),
  "TipTap 3.35.0, enableContentCheck true": wrap((doc) =>
    tiptap(doc, { enableContentCheck: true }),
  ),
};

const results = { scenario: {}, remedies: null };
for (const [name, open] of Object.entries(configs)) {
  results.scenario[name] = runScenario(open);
}
results.remedies = runRemedies();

const lost = (s) =>
  Object.entries(s)
    .filter(([, kept]) => !kept)
    .map(([k]) => k)
    .join(", ") || "nothing";

for (const [name, r] of Object.entries(results.scenario)) {
  console.log(`\n## ${name}`);
  console.log(`  Yjs before any editor lost: ${lost(r.beforeEditor)}`);
  console.log(`  after opening, the reader's Yjs lost: ${lost(r.afterOpen)}`);
  console.log(`  editor shows: ${JSON.stringify(r.shownAfterOpen)}`);
  console.log(
    `  typing in a known paragraph: ${JSON.stringify(r.typeKnown)}, in Yjs: ${r.typedTextInYjs}`,
  );
  console.log(`  after that, lost: ${lost(r.afterTypeKnown)}`);
  console.log(
    `  typing in the aligned paragraph: ${JSON.stringify(r.typeAttr)}`,
  );
  console.log(`  after that, lost: ${lost(r.afterTypeAttr)}`);
  console.log(`  the newer app, after sync, lost: ${lost(r.writerAfterSync)}`);
  console.log(`  errors reported: ${r.errors.length}`);
  for (const d of r.deletes) {
    console.log(`  deleted on "${d.step}": ${JSON.stringify(d.deleted)}`);
  }
}

console.log("\n## Remedies");
for (const [name, r] of Object.entries(results.remedies)) {
  console.log(`\n### ${name}`);
  for (const [k, v] of Object.entries(r)) {
    console.log(`  ${k}: ${typeof v === "object" ? JSON.stringify(v) : v}`);
  }
}

writeFileSync(
  new URL("./results.json", import.meta.url),
  `${JSON.stringify(results, null, 2)}\n`,
);
