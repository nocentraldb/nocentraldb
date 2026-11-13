// The app's logic that holds no DOM: what a page of the index gives a list,
// and what a person's input writes.
import { expect, test } from "vitest";
import { pageSize, rows, titleOf, toggled } from "../src/tasks.ts";

const SPACE = "0123456789abcdef0123456789abcdef";
const task = (id: string, title: string, deleted = false) => ({
  title,
  status: "open" as const,
  $id: id.repeat(32),
  $space: SPACE,
  $deleted: deleted,
});

test("a list shows each task of the page in its order, with its id and space", () => {
  expect(rows([task("a", "buy bread"), task("b", "call Ana")])).toEqual([
    { id: "a".repeat(32), space: SPACE, title: "buy bread", done: false },
    { id: "b".repeat(32), space: SPACE, title: "call Ana", done: false },
  ]);
});

test("a list leaves out a deleted task", () => {
  expect(rows([task("a", "gone", true), task("b", "kept")])).toEqual([
    { id: "b".repeat(32), space: SPACE, title: "kept", done: false },
  ]);
});

test("a task without a title shows as untitled", () => {
  const { title: _, ...untitled } = task("a", "");
  expect(rows([untitled])[0]?.title).toBe("(untitled)");
});

test("ticking a task marks it done, and unticking opens it again", () => {
  expect(toggled(true)).toBe("done");
  expect(toggled(false)).toBe("open");
});

test("an added title is trimmed, and a blank one adds nothing", () => {
  expect(titleOf("  buy bread ")).toBe("buy bread");
  expect(titleOf("   ")).toBeUndefined();
});

test("the page size comes from ?page=, 50 by default, at most 1000", () => {
  expect(pageSize("")).toBe(50);
  expect(pageSize("?page=2")).toBe(2);
  expect(pageSize("?page=0")).toBe(50);
  expect(pageSize("?page=abc")).toBe(50);
  expect(pageSize("?page=5000")).toBe(1000);
});
