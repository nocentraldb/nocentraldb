// The canonical CBOR of RFC 0002 ("The canonical encoding";
// crates/ncdb-plugin/src/cbor.rs), as the worker bridge speaks it: definite
// and minimal lengths and integers, map keys sorted by the bytes of their
// encoding, floats in their shortest exact form with every NaN as 0xf97e00,
// and no tags or simple values but false, true and null. The core decodes
// only canonical bytes, so the SDK writes nothing else. Internal: not part
// of the package's public names.

/** A value the bridge carries. */
export type Cbor =
  | null
  | boolean
  | number
  | string
  | Uint8Array
  | Cbor[]
  | { [key: string]: Cbor | undefined };

const text = new TextEncoder();
const utf8 = new TextDecoder("utf-8", { fatal: true });

/** The canonical bytes of `value`. A key whose value is `undefined` is left out. */
export const encode = (value: unknown): Uint8Array => {
  const out: number[] = [];
  write(out, value);
  return Uint8Array.from(out);
};

const head = (out: number[], major: number, n: number | bigint) => {
  const m = major << 5;
  const v = BigInt(n);
  if (v < 24n) out.push(m | Number(v));
  else if (v < 0x100n) out.push(m | 24, Number(v));
  else if (v < 0x10000n) out.push(m | 25, Number(v >> 8n), Number(v & 0xffn));
  else if (v < 0x100000000n) {
    out.push(m | 26);
    for (let shift = 24n; shift >= 0n; shift -= 8n)
      out.push(Number((v >> shift) & 0xffn));
  } else {
    out.push(m | 27);
    for (let shift = 56n; shift >= 0n; shift -= 8n)
      out.push(Number((v >> shift) & 0xffn));
  }
};

const write = (out: number[], value: unknown): void => {
  if (value === null) {
    out.push(0xf6);
  } else if (value === true || value === false) {
    out.push(value ? 0xf5 : 0xf4);
  } else if (typeof value === "number") {
    number(out, value);
  } else if (typeof value === "string") {
    const bytes = text.encode(value);
    head(out, 3, bytes.length);
    out.push(...bytes);
  } else if (value instanceof Uint8Array) {
    head(out, 2, value.length);
    out.push(...value);
  } else if (Array.isArray(value)) {
    head(out, 4, value.length);
    for (const item of value) write(out, item);
  } else if (
    typeof value === "object" &&
    Object.getPrototypeOf(value) === Object.prototype
  ) {
    const entries = Object.entries(value as Record<string, unknown>)
      .filter(([, v]) => v !== undefined)
      .map(([k, v]) => [encode(k), v] as const)
      .sort(([a], [b]) => compare(a, b));
    head(out, 5, entries.length);
    for (const [key, v] of entries) {
      out.push(...key);
      write(out, v);
    }
  } else {
    throw new TypeError(`the bridge cannot carry a ${typeof value}`);
  }
};

const compare = (a: Uint8Array, b: Uint8Array): number => {
  for (let i = 0; i < Math.min(a.length, b.length); i++) {
    const x = a[i] as number;
    const y = b[i] as number;
    if (x !== y) return x - y;
  }
  return a.length - b.length;
};

const number = (out: number[], n: number) => {
  if (Number.isSafeInteger(n) && !Object.is(n, -0)) {
    if (n >= 0) head(out, 0, n);
    else head(out, 1, -1n - BigInt(n));
    return;
  }
  if (Number.isNaN(n)) {
    out.push(0xf9, 0x7e, 0x00);
    return;
  }
  const half = toHalf(n);
  if (half !== undefined) {
    out.push(0xf9, half >> 8, half & 0xff);
    return;
  }
  const view = new DataView(new ArrayBuffer(8));
  view.setFloat32(0, n);
  if (view.getFloat32(0) === n) {
    out.push(0xfa, ...new Uint8Array(view.buffer, 0, 4));
    return;
  }
  view.setFloat64(0, n);
  out.push(0xfb, ...new Uint8Array(view.buffer));
};

/** The 16-bit form of `n`, when it keeps `n` exactly. */
const toHalf = (n: number): number | undefined => {
  const sign = n < 0 || Object.is(n, -0) ? 0x8000 : 0;
  const a = Math.abs(n);
  if (a === Number.POSITIVE_INFINITY) return sign | 0x7c00;
  if (a === 0) return sign;
  const exponent = Math.floor(Math.log2(a));
  if (exponent > 15 || exponent < -24) return undefined;
  if (exponent >= -14) {
    const mantissa = a / 2 ** exponent - 1;
    const scaled = mantissa * 1024;
    if (
      !Number.isInteger(scaled) ||
      fromHalf(sign | ((exponent + 15) << 10) | scaled) !== n
    )
      return undefined;
    return sign | ((exponent + 15) << 10) | scaled;
  }
  const scaled = a / 2 ** -24;
  if (!Number.isInteger(scaled) || scaled >= 1024) return undefined;
  return sign | scaled;
};

const fromHalf = (bits: number): number => {
  const sign = bits & 0x8000 ? -1 : 1;
  const exponent = (bits >> 10) & 0x1f;
  const mantissa = bits & 0x3ff;
  if (exponent === 0) return sign * mantissa * 2 ** -24;
  if (exponent === 0x1f)
    return mantissa ? Number.NaN : sign * Number.POSITIVE_INFINITY;
  return sign * (1 + mantissa / 1024) * 2 ** (exponent - 15);
};

/** The value of `bytes`, which the core wrote. A map with integer keys reads as an object with those keys as text. */
export const decode = (bytes: Uint8Array): Cbor => {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let at = 0;
  const take = (n: number) => {
    if (at + n > bytes.length) throw new RangeError("the message is truncated");
    const start = at;
    at += n;
    return start;
  };
  const length = (info: number): number => {
    if (info < 24) return info;
    if (info === 24) return view.getUint8(take(1));
    if (info === 25) return view.getUint16(take(2));
    if (info === 26) return view.getUint32(take(4));
    // The core writes no integer beyond 2^53 - 1 to the SDK; one would
    // read rounded, as JavaScript's numbers do.
    if (info === 27) return Number(view.getBigUint64(take(8)));
    throw new RangeError("an indefinite length is not canonical");
  };
  const item = (): Cbor => {
    const initial = view.getUint8(take(1));
    const major = initial >> 5;
    const info = initial & 0x1f;
    switch (major) {
      case 0:
        return length(info);
      case 1:
        return -1 - length(info);
      case 2: {
        const n = length(info);
        return bytes.slice(take(n), at);
      }
      case 3: {
        const n = length(info);
        return utf8.decode(bytes.subarray(take(n), at));
      }
      case 4: {
        const n = length(info);
        return Array.from({ length: n }, item);
      }
      case 5: {
        const n = length(info);
        const map: { [key: string]: Cbor } = {};
        for (let i = 0; i < n; i++) {
          const key = item();
          map[String(key)] = item();
        }
        return map;
      }
      case 7:
        if (info === 20) return false;
        if (info === 21) return true;
        if (info === 22) return null;
        if (info === 25) return fromHalf(view.getUint16(take(2)));
        if (info === 26) return view.getFloat32(take(4));
        if (info === 27) return view.getFloat64(take(8));
        throw new RangeError("a simple value the bridge does not carry");
      default:
        throw new RangeError("a tag the bridge does not carry");
    }
  };
  const value = item();
  if (at !== bytes.length) throw new RangeError("bytes after the message");
  return value;
};
