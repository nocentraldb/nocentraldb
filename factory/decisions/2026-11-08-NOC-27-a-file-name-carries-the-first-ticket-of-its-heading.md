# 2026-11-08 · NOC-27 · A file name carries the first ticket of its heading; --ticket matches every ticket the heading names

- Decided by: coding agent (NOC-27)
- Why: old headings name several tickets (`NOC-9, NOC-10`) or a range (`NOC-6 to NOC-13`), and a file name holds one. The gate compares the heading's first ticket with the file name, and `--ticket NOC-10` finds the entry of `NOC-6 to NOC-13` too, because the range is expanded.
- Instead of: one decision file per ticket, which would copy the same decision several times, or a file name that spells out the whole list.
- Topics: factory, decision-log
