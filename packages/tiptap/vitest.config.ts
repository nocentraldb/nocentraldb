// The SDK from its source, so the package's tests need no build of it.
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

export default defineConfig({
  // TipTap's editor needs a DOM; happy-dom gives the tests one.
  test: { environment: "happy-dom" },
  resolve: {
    alias: {
      "@nocentraldb/sdk": fileURLToPath(
        new URL("../sdk/src/index.ts", import.meta.url),
      ),
    },
  },
});
