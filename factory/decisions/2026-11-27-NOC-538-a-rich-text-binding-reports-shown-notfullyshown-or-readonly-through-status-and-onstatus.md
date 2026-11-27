# 2026-11-27 · NOC-538 · A rich-text binding reports shown, notFullyShown with a reason, or readOnly, through body.status and body.onStatus

- Decided by: Carsten, 2026-11-27, on NOC-538 (Q2), through pm
- Why: an app must see when its editor does not show the whole field or may no longer change it, and why, so it can ask the person to update the app; one field read the same way for every cause keeps that simple.
- Instead of: a status per cause, or only an event without a value to read now.
- Topics: rich-text, rfc-0013, sdk-ts
