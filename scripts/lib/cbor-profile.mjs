// A strict reader of RFC 0002's CBOR profile, for the tooling tests: it
// decodes bytes, refuses everything the profile forbids as it reads, and
// encodes the value again so the caller can compare the bytes. It is written
// independently of the Rust core and of Python's cbor2, so that a test that
// agrees with both checks three implementations against each other.
//
// The profile (docs/rfc/0002-change-and-log-format.md, "The CBOR profile"):
// definite and shortest lengths and integers; maps with unsigned or text
// keys, sorted by their encoded bytes, each key once; no tags; only the
// simple values false, true and null; floats in their shortest exact form,
// every NaN as f97e00; valid UTF-8; integers within 64 bits; at most
// MAX_DEPTH levels counted from the frame.

export const MAX_DEPTH = 32;

export class ProfileError extends Error {}

const fail = (at, reason) => {
  throw new ProfileError(`at byte ${at}: ${reason}`);
};

const utf8 = new TextDecoder("utf-8", { fatal: true });

/** Compares two byte arrays as unsigned bytes, shorter first on a tie. */
export const compareBytes = (a, b) => {
  for (let i = 0; i < Math.min(a.length, b.length); i += 1) {
    if (a[i] !== b[i]) return a[i] - b[i];
  }
  return a.length - b.length;
};

/**
 * Decodes `bytes` as one value at `depth` (1 for a frame; 5 for an
 * operation map inside a payload). Values: bigint for integers, Uint8Array
 * for byte strings, string for text, arrays, `{ map: [[key, value], ...] }`
 * for maps in their encoded order, booleans, null, and `{ float: number }`.
 */
export const decode = (bytes, depth = 1) => {
  let pos = 0;

  const take = (n) => {
    if (pos + n > bytes.length) fail(pos, "truncated");
    const slice = bytes.subarray(pos, pos + n);
    pos += n;
    return slice;
  };

  const argument = (info, start) => {
    if (info < 24) return BigInt(info);
    if (info === 31) fail(start, "indefinite length");
    if (info > 27) fail(start, `reserved additional information ${info}`);
    const size = 1 << (info - 24);
    let value = 0n;
    for (const byte of take(size)) value = (value << 8n) | BigInt(byte);
    const smallest = [24n, 0x100n, 0x10000n, 0x100000000n][info - 24];
    if (value < smallest)
      fail(start, "integer or length not in its shortest form");
    return value;
  };

  const length = (info, start) => {
    const n = argument(info, start);
    if (n > BigInt(bytes.length)) fail(start, "length beyond the input");
    return Number(n);
  };

  const read = (level) => {
    if (level > MAX_DEPTH) fail(pos, `nested deeper than ${MAX_DEPTH}`);
    const start = pos;
    const [initial] = take(1);
    const major = initial >> 5;
    const info = initial & 0x1f;
    switch (major) {
      case 0:
        return argument(info, start);
      case 1:
        return -1n - argument(info, start);
      case 2:
        return Uint8Array.from(take(length(info, start)));
      case 3:
        try {
          return utf8.decode(take(length(info, start)));
        } catch {
          return fail(start, "invalid UTF-8");
        }
      case 4: {
        const n = length(info, start);
        return Array.from({ length: n }, () => read(level + 1));
      }
      case 5: {
        const n = length(info, start);
        const entries = [];
        let previous = null;
        for (let i = 0; i < n; i += 1) {
          const keyStart = pos;
          const key = read(level + 1);
          if (typeof key !== "bigint" && typeof key !== "string") {
            fail(keyStart, "map key is neither an unsigned integer nor text");
          }
          if (typeof key === "bigint" && key < 0n) {
            fail(keyStart, "map key is a negative integer");
          }
          const keyBytes = bytes.subarray(keyStart, pos);
          if (previous && compareBytes(previous, keyBytes) >= 0) {
            fail(keyStart, "map keys not sorted by their bytes, or repeated");
          }
          previous = keyBytes;
          entries.push([key, read(level + 1)]);
        }
        return { map: entries };
      }
      case 6:
        return fail(start, "tag");
      default: {
        if (info === 20) return false;
        if (info === 21) return true;
        if (info === 22) return null;
        if (info < 25 || info > 27) return fail(start, `simple value ${info}`);
        const raw = take(1 << (info - 24));
        const view = new DataView(raw.buffer, raw.byteOffset, raw.length);
        const float =
          info === 25
            ? view.getFloat16(0)
            : info === 26
              ? view.getFloat32(0)
              : view.getFloat64(0);
        return { float };
      }
    }
  };

  const value = read(depth);
  if (pos !== bytes.length) fail(pos, "bytes after the value");
  return value;
};

const head = (major, n) => {
  const value = BigInt(n);
  if (value < 24n) return [(major << 5) | Number(value)];
  const size =
    value < 0x100n ? 1 : value < 0x10000n ? 2 : value < 0x100000000n ? 4 : 8;
  const out = [(major << 5) | (24 + Math.log2(size))];
  for (let i = size - 1; i >= 0; i -= 1) {
    out.push(Number((value >> BigInt(8 * i)) & 0xffn));
  }
  return out;
};

const encodeFloat = (x) => {
  if (Number.isNaN(x)) return [0xf9, 0x7e, 0x00];
  for (const [info, size, fits, set] of [
    [25, 2, Math.f16round(x), "setFloat16"],
    [26, 4, Math.fround(x), "setFloat32"],
    [27, 8, x, "setFloat64"],
  ]) {
    if (Object.is(fits, x)) {
      const view = new DataView(new ArrayBuffer(size));
      view[set](0, x);
      return [0xe0 | info, ...new Uint8Array(view.buffer)];
    }
  }
  throw new Error("unreachable");
};

/** Encodes a value from `decode` in RFC 0002's canonical form. */
export const encode = (value) => {
  const out = [];
  const write = (v) => {
    if (typeof v === "bigint") {
      out.push(...(v >= 0n ? head(0, v) : head(1, -1n - v)));
    } else if (v instanceof Uint8Array) {
      out.push(...head(2, v.length), ...v);
    } else if (typeof v === "string") {
      const text = new TextEncoder().encode(v);
      out.push(...head(3, text.length), ...text);
    } else if (Array.isArray(v)) {
      out.push(...head(4, v.length));
      for (const item of v) write(item);
    } else if (v === false || v === true || v === null) {
      out.push(v === false ? 0xf4 : v === true ? 0xf5 : 0xf6);
    } else if ("float" in v) {
      out.push(...encodeFloat(v.float));
    } else {
      const entries = v.map
        .map(([key, item]) => [encode(key), item])
        .sort(([a], [b]) => compareBytes(a, b));
      out.push(...head(5, entries.length));
      for (const [key, item] of entries) {
        out.push(...key);
        write(item);
      }
    }
  };
  write(value);
  return Uint8Array.from(out);
};

/** Bytes from hex, ignoring whitespace. */
export const fromHex = (hex) => {
  const clean = hex.replace(/\s+/g, "");
  if (!/^(?:[0-9a-f]{2})*$/.test(clean)) throw new Error("not hex");
  return Uint8Array.from(clean.match(/../g) ?? [], (pair) =>
    Number.parseInt(pair, 16),
  );
};

export const toHex = (bytes) =>
  Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
