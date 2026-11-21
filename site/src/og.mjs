// The site's Open Graph images (NOC-410): one for the landing page, one that
// every docs page shares. Each is rendered from HTML by
// scripts/og-images.mjs into public/og at 1200x630; Landing.astro and the
// Starlight head in astro.config.mjs name them. The texts are the site's own:
// the hero's kicker and headline, and the two guides the docs start with.
export const SITE = "https://nocentraldb.io";

export const OG_WIDTH = 1200;
export const OG_HEIGHT = 630;

export const OG_IMAGES = {
  landing: {
    file: "og/landing.png",
    kicker: "A local-first database for people and agents",
    title: "Reads like local files, syncs like shared docs",
    footer: "nocentraldb.io",
    alt: "NoCentralDB: Reads like local files, syncs like shared docs. A local-first database for people and agents.",
  },
  docs: {
    file: "og/docs.png",
    kicker: "Docs",
    title: "Build a first app in Rust or in the browser",
    footer: "nocentraldb.io/docs",
    alt: "NoCentralDB docs: build a first app in Rust or in the browser.",
  },
};

/** The absolute URL of an image of OG_IMAGES. */
export const ogUrl = (image) => `${SITE}/${image.file}`;

/** The meta tags that name `image`, as [attribute, key, content]. */
export const ogTags = (image) => [
  ["property", "og:image", ogUrl(image)],
  ["property", "og:image:width", String(OG_WIDTH)],
  ["property", "og:image:height", String(OG_HEIGHT)],
  ["property", "og:image:alt", image.alt],
  ["name", "twitter:card", "summary_large_image"],
];
