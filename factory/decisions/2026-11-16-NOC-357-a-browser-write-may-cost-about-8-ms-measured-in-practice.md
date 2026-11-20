# 2026-11-16 · NOC-357 · A browser write may cost about 8 ms instead of 1.5 ms in WebKit, and the cost is measured in practice

- Decided by: Carsten, 2026-11-16, on RFC 0012's questions 2 to 5, through pm (question 4)
- Why: taking the database per transaction resumes and pauses `opfs-sahpool` around each write; in the spike, through our Rust VFS, a small write took about 8 ms instead of 1.5 ms in WebKit, 13 ms instead of 1.5 ms in Chromium and 31 ms instead of 5 ms in Firefox, before any work on a faster resume. The owner accepted it: "we'll see, it is worth the risk". The browser tests of every engine measure it once the design is built.
- Instead of: holding option B back until a faster resume.
- Topics: browser, tabs, rfc-0012, sdk-ts, cost
