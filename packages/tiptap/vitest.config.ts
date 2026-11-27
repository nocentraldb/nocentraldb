// The SDK from its source, so the package's tests need no build of it.
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

export default defineConfig({
  resolve: {
    alias: {
      "@nocentraldb/sdk": fileURLToPath(
        new URL("../sdk/src/index.ts", import.meta.url),
      ),
    },
  },
});
