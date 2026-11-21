// Renders the site's Open Graph images (NOC-410) from the HTML below into
// site/public/og, at 1200x630, in the brand's dark palette and type
// (tokens.css): the near-black ground, the yellow mark beside the wordmark,
// a kicker with the yellow dot, the headline in the heading face and a muted
// footer line. The texts live in src/og.mjs. Each PNG carries the SHA-256 of
// the HTML it was rendered from in a tEXt chunk named "source", so
// scripts/og-images.test.mjs fails when a text, the template or the logo
// changes and the PNG was not rendered again.
//
//   node site/scripts/og-images.mjs
//
// It needs Playwright's Chromium and the network for the brand's Google
// fonts, and runs by hand, like scripts/diagrams.mjs: the PNGs are in git.
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { crc32 } from "node:zlib";
import { OG_HEIGHT, OG_IMAGES, OG_WIDTH } from "../src/og.mjs";

const site = join(dirname(fileURLToPath(import.meta.url)), "..");

const escape = (text) =>
  text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

/** The HTML page an image is rendered from. */
export const ogHtml = (image) => {
  const wordmark = readFileSync(join(site, "src/assets/brand/ncdb-wordmark-dark.svg"), "utf8")
    .replace(/<rect[^>]*\/>/, "")
    .replace("<svg ", '<svg class="wordmark" ');
  const mark = readFileSync(join(site, "public/brand/ncdb-mark-accent-512.png")).toString("base64");
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Encode+Sans+Semi+Expanded:wght@700&family=Commissioner:wght@350..700&family=Syne+Mono&display=block">
<style>
  html, body { margin: 0; }
  body {
    width: ${OG_WIDTH}px; height: ${OG_HEIGHT}px; box-sizing: border-box;
    padding: 72px 80px 64px; display: flex; flex-direction: column;
    background: #111110; color: #ecebe4;
    font-family: "Commissioner", system-ui, sans-serif; font-weight: 350;
  }
  .brand { display: flex; align-items: center; gap: 20px; }
  .mark { width: 64px; height: 64px; }
  .wordmark { height: 52px; width: auto; }
  .main { margin-top: auto; }
  .kicker {
    display: flex; align-items: center; gap: 14px; margin: 0 0 24px;
    font-size: 30px; font-weight: 500; color: #ecebe4;
  }
  .dot { width: 14px; height: 14px; border-radius: 50%; background: #f2dc4b; }
  h1 {
    margin: 0; max-width: 1000px;
    font-family: "Encode Sans Semi Expanded", system-ui, sans-serif; font-weight: 700;
    font-size: 72px; line-height: 1.08; letter-spacing: -0.01em;
  }
  .footer {
    margin-top: 48px; padding-top: 24px; border-top: 2px solid #26261f;
    font-family: "Syne Mono", ui-monospace, monospace; font-size: 26px; color: #a3a299;
  }
</style>
</head>
<body>
  <div class="brand"><img class="mark" alt="" src="data:image/png;base64,${mark}">${wordmark}</div>
  <div class="main">
    <p class="kicker"><span class="dot"></span>${escape(image.kicker)}</p>
    <h1>${escape(image.title)}</h1>
  </div>
  <div class="footer">${escape(image.footer)}</div>
</body>
</html>
`;
};

/** The SHA-256 of an image's HTML, as hex. */
export const sourceHash = (html) => createHash("sha256").update(html).digest("hex");

/** A PNG chunk of `type` holding `data`. */
const chunk = (type, data) => {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "latin1"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([length, body, crc]);
};

/** `png` with a tEXt chunk `keyword` = `text` right after its IHDR chunk. */
export const withText = (png, keyword, text) => {
  const ihdrEnd = 8 + 4 + 4 + 13 + 4;
  const data = Buffer.from(`${keyword}\0${text}`, "latin1");
  return Buffer.concat([png.subarray(0, ihdrEnd), chunk("tEXt", data), png.subarray(ihdrEnd)]);
};

/** A PNG's width, height and tEXt entries. */
export const pngInfo = (png) => {
  const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  if (!png.subarray(0, 8).equals(signature)) throw new Error("not a PNG");
  const text = {};
  let width = 0;
  let height = 0;
  for (let at = 8; at < png.length; ) {
    const length = png.readUInt32BE(at);
    const type = png.toString("latin1", at + 4, at + 8);
    const data = png.subarray(at + 8, at + 8 + length);
    if (type === "IHDR") {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
    }
    if (type === "tEXt") {
      const zero = data.indexOf(0);
      text[data.toString("latin1", 0, zero)] = data.toString("latin1", zero + 1);
    }
    at += 12 + length;
  }
  return { width, height, text };
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const { chromium } = await import("playwright");
  const browser = await chromium.launch();
  const page = await browser.newPage({
    viewport: { width: OG_WIDTH, height: OG_HEIGHT },
    deviceScaleFactor: 1,
  });
  for (const image of Object.values(OG_IMAGES)) {
    const html = ogHtml(image);
    await page.setContent(html, { waitUntil: "networkidle" });
    await page.evaluate(() => document.fonts.ready);
    const missing = await page.evaluate(() =>
      ['700 72px "Encode Sans Semi Expanded"', '500 30px "Commissioner"', '26px "Syne Mono"'].filter(
        (font) => !document.fonts.check(font),
      ),
    );
    if (missing.length > 0) throw new Error(`fonts did not load: ${missing.join(", ")}`);
    const png = await page.screenshot({ type: "png" });
    const out = join(site, "public", image.file);
    writeFileSync(out, withText(png, "source", sourceHash(html)));
    console.log(out);
  }
  await browser.close();
}
