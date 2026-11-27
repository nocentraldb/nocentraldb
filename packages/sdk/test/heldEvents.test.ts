// The core's events reach `connect` also when they come before it listens
// (NOC-571): the first `relay` ask arrives while the database opens.
// Chromium runs it with the real core (scripts/sdk-browser/relayFunction.e2e.test.mjs).
import { expect, test } from "vitest";
import { heldEvents } from "../src/heldEvents.ts";

const bytes = (n: number) => new Uint8Array([n]);

test("events before listen reach the listener first, in order, then the later ones", () => {
  const events = heldEvents();
  events.emit(bytes(1));
  events.emit(bytes(2));
  const heard: number[] = [];
  events.listen((event) => heard.push(event[0] ?? -1));
  events.emit(bytes(3));
  expect(heard).toEqual([1, 2, 3]);
});

test("a second listener gets only what comes after it", () => {
  const events = heldEvents();
  events.emit(bytes(1));
  const first: number[] = [];
  events.listen((event) => first.push(event[0] ?? -1));
  const second: number[] = [];
  events.listen((event) => second.push(event[0] ?? -1));
  events.emit(bytes(2));
  expect(first).toEqual([1]);
  expect(second).toEqual([2]);
});
