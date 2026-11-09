# 2026-11-09 · NOC-34 · RFC 0001 and RFC 0006 say that an evicted browser copy takes its member key with it

- Decided by: coding agent (NOC-34), for pm's and the owner's review
- Why: a browser evicts all of an origin's data at once (documented, MDN), and RFC 0003 keeps the browser's member key in IndexedDB of the same origin. RFC 0001 said the member then fetches its log again, which it cannot do without its key: the person pairs the browser again as a new device. The sentence claimed more than the design delivers (principle 4), so RFC 0001 gets Amendment 3 and RFC 0006's storage limits are rewritten under its Amendment 2, in the style of the earlier amendments, adding no decision.
- Instead of: leaving RFC 0001's sentence as it was and stating the key loss only on the new page, so that two documents disagree.
- Topics: browser, durability, rfc-0001, rfc-0006, keys
