# 2026-11-09 · NOC-35 · GENESIS.md links no RFC

- Decided by: Carsten
- Why: the owner wants GENESIS.md as clean and clear as possible (commit 33da4ae on #76): it states what we build and its limits, and the RFCs hold the details. The owner removed every RFC link from it, and with them the sentence on groups as lists the SDK expands, which RFC 0008 keeps. The owner kept the proposed sentence that keeping names out of free text is the app's job, without its link. The tests of NOC-20, NOC-44 and NOC-47 that required those links in GENESIS.md now check the claims only, and a new test keeps GENESIS.md free of RFC links.
- Instead of: GENESIS.md citing the RFC section behind each limit, as NOC-20, NOC-44, NOC-47 and this ticket had it.
- Topics: genesis, rfc, docs
