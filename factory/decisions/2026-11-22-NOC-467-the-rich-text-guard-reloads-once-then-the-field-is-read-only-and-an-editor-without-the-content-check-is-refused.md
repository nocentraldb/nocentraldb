# 2026-11-22 · NOC-467 · The rich-text guard reloads once, then makes the field read-only; an editor without enableContentCheck is refused with an error

- Decided by: Carsten, 2026-11-22, on NOC-467, through pm
- Why: the browser spike (NOC-455) showed the guard alone reloading at every pause, the rebuilt editor deleting the unknown parts again, and the person's typing refused with each write. A second refusal now stops the loop, and the missing content check, which causes it, fails when the editor is created.
- Instead of: reloading after every refused write.
- Topics: rich-text, rfc-0013, sdk-ts, unknown-nodes, errors
