# 2026-11-08 · NOC-19 · Simulated members learn arbitrary earlier operations, not causally closed sets

- Decided by: coding agent (NOC-19)
- Why: RFC 0004 says no field type may rely on causal delivery, so before each operation a member learns a random subset of earlier operations, without what their authors had seen. C4 checks "replaced" against exactly that set.
- Instead of: learning an operation together with everything its author had seen, which would hide merge rules that assume causal delivery.
- Topics: convergence, testing
