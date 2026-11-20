# 2026-11-12 · NOC-232 · A plugin field's config is part of its merge rule, so the build refuses a change of it

- Decided by: Carsten, 2026-11-12, on NOC-232 ("Ich stimme den Vorschlägen zu"), RFC 0011 decision 5
- Why: Two members that fold one field with different options could reach different states, so `ncdb model build` and the check on the device refuse a changed `config` as they refuse a changed type; a new field with the new `config` can be added and the old one deprecated.
- Instead of: no other option was put to the owner
- Topics: rfc-0011, rfc-0005, plugins, model-lock
