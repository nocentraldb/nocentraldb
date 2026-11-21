// Days and points in time between the app and the core (RFC 0005, "Days
// and points in time"; Carsten, 2026-11-21, NOC-434). The app writes and
// reads a day as its "YYYY-MM-DD" string and a point in time as a Date; the
// bridge takes a day as that text and a point in time as its milliseconds
// since 1970-01-01T00:00:00.000Z (crates/nocentraldb/src/bridge/values.rs).
// Both range over the years 1 to 9999. Internal: the package's entry does
// not reach it.

import { NcError } from "./error.ts";
import * as texts from "./texts.ts";

const DATE = "ncdb/date";
const TIMESTAMP = "ncdb/timestamp";

/** The first and the last millisecond of the years 1 to 9999. */
const FIRST = -62_135_596_800_000;
const LAST = 253_402_300_799_999;

/** Whether `text` is exactly "YYYY-MM-DD" and names a day of the years 1 to 9999. */
const isDay = (text: string): boolean => {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(text);
  if (match === null) return false;
  const [year, month, day] = match.slice(1).map(Number) as [
    number,
    number,
    number,
  ];
  if (year < 1) return false;
  // Date.UTC maps years 0 to 99 onto 1900 to 1999; setUTCFullYear does not.
  const at = new Date(0);
  at.setUTCFullYear(year, month - 1, day);
  return (
    at.getUTCFullYear() === year &&
    at.getUTCMonth() === month - 1 &&
    at.getUTCDate() === day
  );
};

/** Whether `value` is a valid Date of the years 1 to 9999. */
const isPoint = (value: unknown): value is Date =>
  value instanceof Date &&
  !Number.isNaN(value.getTime()) &&
  value.getTime() >= FIRST &&
  value.getTime() <= LAST;

/**
 * The form the bridge takes for `written` in a field of `type`: a day as
 * its text, a point in time as its milliseconds; `invalidValue` for
 * anything else. A value of another type passes as it is.
 */
export const toCore = (
  type: string | undefined,
  collection: string,
  field: string,
  written: unknown,
): unknown => {
  if (type === DATE) {
    if (typeof written === "string" && isDay(written)) return written;
    throw new NcError("invalidValue", texts.notADay(collection, field));
  }
  if (type === TIMESTAMP) {
    if (isPoint(written)) return written.getTime();
    throw new NcError("invalidValue", texts.notATimestamp(collection, field));
  }
  return written;
};

/** A query's equality value as the bridge takes it: a Date as its milliseconds. */
export const equalToCore = (value: unknown): unknown =>
  value instanceof Date ? value.getTime() : value;

/**
 * `object`, read from `collection`, with each point in time as a Date. The
 * bridge gives a day as its text already.
 */
export const fromCore = <T extends Record<string, unknown>>(
  fields: Record<string, { type: string }> | undefined,
  object: T,
): T => {
  for (const [name, field] of Object.entries(fields ?? {})) {
    const value = object[name];
    if (field.type === TIMESTAMP && typeof value === "number") {
      (object as Record<string, unknown>)[name] = new Date(value);
    }
  }
  return object;
};
