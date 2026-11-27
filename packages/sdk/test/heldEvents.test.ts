// The core's asks reach `connect` also when they come before it listens
// (NOC-571): the first `relay` ask arrives while the database opens.
// Chromium runs it with the real core (scripts/sdk-browser/relayFunction.e2e.test.mjs).
import { expect, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { heldEvents } from "../src/heldEvents.ts";

const relayAsk = (request: number) => encode({ event: "relay", request });
const sync = (connection: string) =>
  encode({ event: "sync", status: { connection } });

/** A listener that notes each event's name and request number. */
const hearing = () => {
  const heard: string[] = [];
  const listen = (bytes: Uint8Array) => {
    const { event, request } = decode(bytes) as {
      event: string;
      request?: number;
    };
    heard.push(request === undefined ? event : `${event} ${request}`);
  };
  return { heard, listen };
};

test("asks before listen reach the listener first, in order, then every later event", () => {
  const events = heldEvents();
  events.emit(relayAsk(1));
  events.emit(encode({ event: "admission", request: 2, identity: "ab" }));
  const { heard, listen } = hearing();
  events.listen(listen);
  events.emit(sync("connecting"));
  events.emit(relayAsk(3));
  expect(heard).toEqual(["relay 1", "admission 2", "sync", "relay 3"]);
});

test("a state event before listen is dropped, since the answer to open carries a newer state", () => {
  const events = heldEvents();
  events.emit(sync("connecting"));
  events.emit(relayAsk(1));
  events.emit(encode({ event: "waitingForTab", waiting: true }));
  const { heard, listen } = hearing();
  events.listen(listen);
  expect(heard).toEqual(["relay 1"]);
});
