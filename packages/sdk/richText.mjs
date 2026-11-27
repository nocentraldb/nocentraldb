// The marker a core built with the compiler cfg `ncdb_rich_text` exports
// (NOC-461): crates/nocentraldb-wasm holds the rich-text bridge only with
// that cfg, which only the project's own tasks set, until the fold's guard
// is complete (NOC-468). The package's build (build.mjs) refuses a module
// that holds the marker. Not in the package: its `files` hold only dist/.

/** The marker, as the core's `RICH_TEXT` holds it. */
export const RICH_TEXT = "nocentraldb:rich-text";

/** Throws when `bytes`, the module at `path`, holds the marker: a core built with rich text, which the package must not ship yet. */
export const refuseRichText = (bytes, path) => {
  if (Buffer.from(bytes).includes(RICH_TEXT)) {
    throw new Error(
      `${path} holds rich text, which the package must not ship yet: build the package's core without the cfg ncdb_rich_text`,
    );
  }
};
