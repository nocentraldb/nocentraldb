# 2026-11-22 · NOC-447 · A moved object keeps its id, and the old space keeps a trace that names the new space

- Decided by: Carsten, 2026-11-22, on RFC 0016's questions, through pm (NOC-449)
- Why: the app's URLs and tables keep working, links between objects moved together stay valid, moving back revives the trace, and a reference finds the object from the new space id alone.
- Instead of: a new id in the new space, which every URL, table and link among moved objects would have to follow.
- Topics: move, object-model, rfc-0016
