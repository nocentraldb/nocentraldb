import { expect, test } from "vitest";
import { name } from "../src/index.ts";

test("reports its name", () => {
  expect(name("")).toBe("@nocentraldb/sdk");
});
