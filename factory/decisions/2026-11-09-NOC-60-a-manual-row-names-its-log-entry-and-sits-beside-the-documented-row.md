# 2026-11-09 · NOC-60 · A manual row names its log entry, and a measurement sits beside the documented row

- Decided by: coding agent (NOC-60)
- Why: the source `measured: manual check <entry heading>` carries the device and the browser version the ticket asks for and lets the gate find the exact log entry, outside a code fence. A behaviour a vendor documents (Safari's and Firefox's `persist()`) keeps its documented row with its link, and the measurement gets a row of its own below it, so the vendor's explanation is not lost.
- Instead of: the ticket's wording `measured: manual, 2026-11-09` (the doc and gate already used `measured: manual check <date>`, and a date alone matched the log's template too), and overwriting documented rows with what one device showed.
- Topics: browser-storage, testing
