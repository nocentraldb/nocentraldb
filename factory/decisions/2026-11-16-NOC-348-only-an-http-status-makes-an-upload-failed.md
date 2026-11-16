# 2026-11-16 · NOC-348 · Only an HTTP status makes an upload `upload_failed`; its retry waits as the reconnect does

- Decided by: coding agent, NOC-348, within the owner's option A on NOC-349
- Why: an HTTP status means the URL's server answered and refused, while the relay connection is fine, so the session keeps it and retries that space's push. A lost network, a timeout or an unreadable answer may as well mean the member is offline, so these still end the connection, as before. The retry waits 100 ms, doubling to 120 s, the reconnect's own bounds (`run::FIRST`, `run::LONGEST`), so "the usual backoff" is one set of numbers. A local write does not cut the wait short, as it does not cut the reconnect's.
- Instead of: treating every failure of the upload as `upload_failed`, which would keep a connection that may be gone; a backoff of its own with other numbers.
- Topics: sync, sdk, large-changes
