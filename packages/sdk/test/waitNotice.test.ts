// When the worker reports that it waits for another tab's storage
// (NOC-355): only once the wait has lasted 1 s, so a normal handover,
// where the old tab's worker is still stopping for a few milliseconds
// (Firefox), reports nothing (Carsten, 2026-11-16, NOC-355).
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { waitNotice } from "../src/waitNotice.ts";

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

test("a wait shorter than 1 s reports nothing", () => {
  const reported: boolean[] = [];
  const notice = waitNotice((waiting) => reported.push(waiting));
  notice.started();
  vi.advanceTimersByTime(999);
  notice.ended();
  vi.advanceTimersByTime(5000);
  expect(reported).toEqual([]);
});

test("a wait of 1 s reports true then, and false when it ends", () => {
  const reported: boolean[] = [];
  const notice = waitNotice((waiting) => reported.push(waiting));
  notice.started();
  vi.advanceTimersByTime(999);
  expect(reported).toEqual([]);
  vi.advanceTimersByTime(1);
  expect(reported).toEqual([true]);
  notice.ended();
  expect(reported).toEqual([true, false]);
});
