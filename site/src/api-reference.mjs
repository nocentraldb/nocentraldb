// The TypeScript API reference (NOC-361, pm's option A of 2026-11-20): the
// docs page rendered from the SDK's committed API report,
// packages/sdk/etc/api-report.d.ts. The `api-report` gate keeps that file
// equal to the SDK's public declarations, so the page is never older than the
// SDK. The report carries no doc comments (scripts/release/api-report.mjs
// removes them), so the page shows declarations; /docs/api says what the main
// calls do.

export const API_REFERENCE = {
  file: "packages/sdk/etc/api-report.d.ts",
  slug: "docs/api/reference",
  label: "API reference",
};

const MARKER = /^\/\/ file: (.*)\.d\.ts$/;

/** The page for the API report `report`: one section per module. */
export const apiReferenceEntry = (report) => {
  const sections = [];
  for (const line of report.replace(/\n$/, "").split("\n")) {
    const marker = line.match(MARKER);
    if (marker) sections.push({ module: marker[1], lines: [] });
    else if (sections.length === 0)
      throw new Error(`${API_REFERENCE.file} must start with a module marker`);
    else sections.at(-1).lines.push(line);
  }
  if (sections.length === 0)
    throw new Error(`${API_REFERENCE.file} names no module`);
  const intro = [
    "The public declarations of the TypeScript SDK, `@nocentraldb/sdk`, one section per module of the package. The page is built from the SDK's API report, which the repository's checks keep equal to the SDK's code.",
    "",
    "The declarations carry no comments. [API](/docs/api/) says what the main calls do, and the [browser guide](/docs/guides/browser/) uses them in a first app.",
  ].join("\n");
  const body = [
    intro,
    ...sections.map(
      ({ module, lines }) =>
        `## ${module}\n\n\`\`\`ts\n${lines.join("\n")}\n\`\`\``,
    ),
  ].join("\n\n");
  return {
    title: "TypeScript API reference",
    description:
      "The public declarations of the TypeScript SDK, one section per module.",
    body: `${body}\n`,
  };
};
