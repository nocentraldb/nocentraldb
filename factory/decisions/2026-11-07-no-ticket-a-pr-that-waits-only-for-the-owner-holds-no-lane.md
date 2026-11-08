# 2026-11-07 · no ticket · A PR that waits only for the owner holds no lane

- Decided by: Carsten
- Why: four reviewed PRs waited for the owner and blocked all four lanes, so no ready ticket could start; the lanes now count only work in progress, and a waiting session stays open to answer review.
- Instead of: a lane held until merge, which keeps the owner's queue short but stops the factory while the owner reviews.
- Topics: factory, lanes
