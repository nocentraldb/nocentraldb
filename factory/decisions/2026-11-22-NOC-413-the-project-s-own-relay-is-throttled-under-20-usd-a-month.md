# 2026-11-22 · NOC-413 · The throttling of the project's own relay holds it under 20 USD a month

- Decided by: Carsten, 2026-11-22, on RFC 0015's questions, through pm (NOC-429)
- Why: open signup lets every user of an app write, and today nothing limits writes; a ceiling of API Gateway stage throttling and reserved concurrency caps the worst case at today's budget, at the price of unavailable for everybody once it is reached.
- Instead of: no ceiling, with the budget only sending a mail; a higher ceiling.
- Topics: relay, aws, cost, admission, rfc-0015
