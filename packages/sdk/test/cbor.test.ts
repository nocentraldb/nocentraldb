// The worker bridge's messages are canonical CBOR (RFC 0002, "The canonical
// encoding"; crates/ncdb-plugin/src/cbor.rs): the core decodes only
// canonical bytes, so the SDK's encoder writes shortest integers and
// floats, definite lengths, and map keys sorted by their encoded bytes.
import { expect, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";

const hex = (bytes: Uint8Array) =>
  [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");

test("encodes integers in their shortest form", () => {
  expect(hex(encode(0))).toBe("00");
  expect(hex(encode(23))).toBe("17");
  expect(hex(encode(24))).toBe("1818");
  expect(hex(encode(500))).toBe("1901f4");
  expect(hex(encode(70000))).toBe("1a00011170");
  expect(hex(encode(2 ** 40))).toBe("1b0000010000000000");
  expect(hex(encode(-1))).toBe("20");
  expect(hex(encode(-500))).toBe("3901f3");
});

test("encodes floats in their shortest exact form", () => {
  expect(hex(encode(1.5))).toBe("f93e00");
  expect(hex(encode(0.1))).toBe("fb3fb999999999999a");
  expect(hex(encode(100000.5))).toBe("fa47c35040");
  expect(hex(encode(Number.NaN))).toBe("f97e00");
  expect(hex(encode(Number.POSITIVE_INFINITY))).toBe("f97c00");
  expect(hex(encode(-0))).toBe("f98000");
});

test("encodes text, bytes, booleans and null", () => {
  expect(hex(encode("a"))).toBe("6161");
  expect(hex(encode("ü"))).toBe("62c3bc");
  expect(hex(encode(new Uint8Array([1, 2])))).toBe("420102");
  expect(hex(encode(true))).toBe("f5");
  expect(hex(encode(false))).toBe("f4");
  expect(hex(encode(null))).toBe("f6");
});

test("sorts map keys by their encoded bytes and drops undefined", () => {
  // "id" (0x62...) sorts before "call" (0x64...): shorter text first.
  expect(hex(encode({ call: 1, id: 2, gone: undefined }))).toBe(
    "a2626964026463616c6c01",
  );
});

test("round-trips nested values", () => {
  const value = {
    id: 7,
    call: "write",
    args: {
      space: "00112233445566778899aabbccddeeff",
      ops: [{ op: "create", values: { title: "Ship", done: false, n: -3 } }],
      tags: { $add: ["a", "b"] },
      none: null,
      half: 2.5,
      bytes: new Uint8Array([255]),
    },
  };
  expect(decode(encode(value))).toEqual(value);
});

test("decodes what the core writes, maps with integer keys as objects", () => {
  // {1: "a"} as the core writes a map with an unsigned key.
  expect(decode(new Uint8Array([0xa1, 0x01, 0x61, 0x61]))).toEqual({ 1: "a" });
});

test("encodes a number beyond the safe integers as a float", () => {
  expect(hex(encode(2 ** 64))).toBe("fa5f800000");
});

test("refuses a value it cannot encode", () => {
  expect(() => encode(Symbol("x") as unknown)).toThrow();
  expect(() => encode(() => 1)).toThrow();
  expect(() => encode(10n as unknown)).toThrow();
});
