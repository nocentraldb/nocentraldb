// Copies the RFC diagrams that the docs page "How NoCentralDB works" shows
// (NOC-360) into site/public/diagrams, in the brand's dark palette. The RFCs'
// files carry GitHub's light palette and a dark one under
// prefers-color-scheme; the site is dark only (BRAND.md), so the copy swaps
// the style block for one in the brand's colours and keeps everything else,
// labels included. The RFCs' own files never change here.
//
//   node site/scripts/diagrams.mjs
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/** The diagrams the page shows, by their file name in docs/rfc. */
export const DIAGRAMS = [
  "0001-one-member.svg",
  "0001-two-spaces.svg",
  "0001-one-change.svg",
];

// The RFC style's classes, in the brand's dark palette (tokens.css): the
// near-black ground, the ink and the muted for text, the safety yellow as the
// one accent, and surfaces for boxes.
const STYLE = `<style>
    .bg { fill: #111110; }
    .t { fill: #ecebe4; }
    .m { fill: #a3a299; }
    .ac { fill: #f2dc4b; }
    .box { fill: #1b1b19; stroke: #3d3d36; }
    .plain { fill: #111110; stroke: #3d3d36; }
    .focal { fill: #2b2816; stroke: #f2dc4b; }
    .zone { fill: none; stroke: #3d3d36; }
    .zonea { fill: #f2dc4b; fill-opacity: 0.06; stroke: #f2dc4b; }
    .frame { fill: #1b1b19; stroke: #3d3d36; }
    .life { stroke: #3d3d36; }
    .ln { stroke: #a3a299; fill: none; }
    .lna { stroke: #f2dc4b; fill: none; }
    .mk { fill: #a3a299; }
    .mka { fill: #f2dc4b; }
    .mask { fill: #111110; }
    .num { fill: #111110; }
  </style>`;

/** The site copy of an RFC diagram's SVG text: its style in the brand's palette. */
export const siteCopy = (svg) => {
  const start = svg.indexOf("<style>");
  const end = svg.indexOf("</style>");
  if (start < 0 || end < 0) throw new Error("the diagram has no style block");
  return svg.slice(0, start) + STYLE + svg.slice(end + "</style>".length);
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const site = join(dirname(fileURLToPath(import.meta.url)), "..");
  for (const name of DIAGRAMS) {
    const original = readFileSync(join(site, "..", "docs", "rfc", name), "utf8");
    writeFileSync(join(site, "public", "diagrams", name), siteCopy(original));
  }
}
