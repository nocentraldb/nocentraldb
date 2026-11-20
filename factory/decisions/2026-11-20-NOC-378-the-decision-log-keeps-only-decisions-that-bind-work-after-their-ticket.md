# 2026-11-20 · NOC-378 · The decision log keeps only decisions that bind work after their ticket is done, each titled by what it decides

- Decided by: Carsten, 2026-11-20, on NOC-378
- Why: agents read the log's index first, so only titles count there, and entries that matter only for one ticket bury the ones that bind. The log keeps design and product decisions, factory rules, repository-wide conventions and owner decisions a text relies on; what matters for one ticket goes on the ticket as a comment. A title states the decision itself, never who decided or that a recommendation was followed; a batch becomes one file per decision; a superseded entry is deleted. FACTORY.md, "The decision log", holds the rule, and the gate refuses a superseded file left in the folder and a title that names a review's outcome or who decided.
- Instead of: a log that records every call a reviewer could question, under titles such as "The owner's answers to Q1–Q6", and keeps superseded entries untouched.
- Topics: factory, decision-log
