// Screenshots of the landing page: desktop 1440 px and mobile 390 px, full
// page, and the first screen of a short desktop window, 1440x670;
// deviceScaleFactor 2.
// Usage: pnpm build && pnpm preview, then pnpm shots [base-url] in another shell.
// drafts/ is git-ignored: the screenshots go to the pull request, never into git.
import { mkdir } from "node:fs/promises";
import { chromium } from "playwright";

const base = process.argv[2] ?? "http://localhost:4321";
const shots = [
  { name: "landing-desktop", width: 1440, height: 900, fullPage: true },
  { name: "landing-mobile", width: 390, height: 900, fullPage: true },
  { name: "landing-desktop-670", width: 1440, height: 670, fullPage: false },
];
const out = new URL("../drafts/", import.meta.url);
await mkdir(out, { recursive: true });

const browser = await chromium.launch();
for (const { name, width, height, fullPage } of shots) {
  const page = await browser.newPage({
    viewport: { width, height },
    deviceScaleFactor: 2,
  });
  await page.goto(`${base}/?shot`, {
    waitUntil: "networkidle",
  });
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(900);
  const file = new URL(`${name}.png`, out);
  await page.screenshot({ path: file.pathname, fullPage });
  console.log(file.pathname);
  await page.close();
}
await browser.close();
