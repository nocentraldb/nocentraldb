// While a worker holds a database open it touches OPFS every minute
// (NOC-444): Chromium drops an off-the-record origin's OPFS after about ten
// minutes without a file operation, with every access handle and file in
// it. worker.ts starts the keep-alive when an open begins and stops it
// when the open fails or the worker stops.
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { KEEP_ALIVE_EVERY, keepAlive } from "../src/keepAlive.ts";

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

test("touches every minute, well inside Chromium's ten minutes", () => {
  expect(KEEP_ALIVE_EVERY).toBe(60_000);
});

test("a started keep-alive touches once each interval, and a stopped one no more", () => {
  const touch = vi.fn(async () => {});
  const alive = keepAlive(touch);
  alive.start();
  vi.advanceTimersByTime(KEEP_ALIVE_EVERY - 1);
  expect(touch).toHaveBeenCalledTimes(0);
  vi.advanceTimersByTime(1);
  expect(touch).toHaveBeenCalledTimes(1);
  vi.advanceTimersByTime(9 * KEEP_ALIVE_EVERY);
  expect(touch).toHaveBeenCalledTimes(10);
  alive.stop();
  vi.advanceTimersByTime(10 * KEEP_ALIVE_EVERY);
  expect(touch).toHaveBeenCalledTimes(10);
});

test("starting twice keeps one timer", () => {
  const touch = vi.fn(async () => {});
  const alive = keepAlive(touch);
  alive.start();
  alive.start();
  vi.advanceTimersByTime(KEEP_ALIVE_EVERY);
  expect(touch).toHaveBeenCalledTimes(1);
  alive.stop();
});

test("a touch that fails is swallowed, and the next one still runs", async () => {
  const touch = vi
    .fn<() => Promise<unknown>>()
    .mockRejectedValueOnce(new Error("NotFoundError"))
    .mockResolvedValue(undefined);
  // An unhandled rejection would fail the run.
  const alive = keepAlive(touch);
  alive.start();
  await vi.advanceTimersByTimeAsync(2 * KEEP_ALIVE_EVERY);
  expect(touch).toHaveBeenCalledTimes(2);
  alive.stop();
});
