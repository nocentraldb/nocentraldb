# 2026-11-10 · NOC-94 · A child process writes the fake node, instead of a rename into place

- Decided by: coding agent
- Why: the ETXTBSY comes from the write descriptor that every child forked meanwhile by another test thread inherits until it execs; a rename keeps the same file, so the leaked descriptor still makes it busy, and a 16-thread stress test failed with the rename as often as without it, and never in 100 runs once a child `sh` wrote the script
- Instead of: writing to a temporary name and renaming it into place, as the ticket suggested
- Topics: testing, cli
