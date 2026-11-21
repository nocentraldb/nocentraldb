# 2026-11-21 · NOC-412 · TypeScript reads and writes a day as a "YYYY-MM-DD" string and a point in time as a Date

- Decided by: Carsten, 2026-11-21, on NOC-434 (answer 4a)
- Why: a day has no time zone, so it must not be a Date, which is an instant; the text sorts, compares and survives JSON. A Date holds exactly a millisecond in UTC, the point in time's value, and nc.Infer tells it from an int.
- Instead of: a day as a Date at midnight UTC, which shows the day before west of UTC; a point in time as a number of milliseconds; Temporal types, which need Temporal in every engine or a polyfill.
- Topics: rfc-0005, sdk, typescript, dates, public-names
