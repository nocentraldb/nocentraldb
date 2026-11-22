// Bundles the page (page/main.js) into dist/spike.js with esbuild.
import { build } from "esbuild";

await build({
  entryPoints: ["page/main.js"],
  bundle: true,
  format: "esm",
  outfile: "dist/spike.js",
  logLevel: "warning",
});
