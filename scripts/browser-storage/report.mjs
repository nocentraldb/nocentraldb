// Turns the results of a probe run (measure.mjs) into the rows of the table
// in docs/browser-storage.md. Pure functions, so the gate can hold the
// committed table to the committed results without starting a browser.

/** The table rows a run measured, in the order the doc lists them. */
export const measuredRows = (_run) => {
  throw new Error("not yet implemented");
};

/** Parses the rows of the first Markdown table under a heading. */
export const tableRows = (_markdown, _heading) => {
  throw new Error("not yet implemented");
};
