# 2026-11-12 · NOC-232 · A plugin type that is not registered still opens: it parks as UnknownType, reads unset and refuses writes

- Decided by: Carsten, 2026-11-12, on NOC-232 ("Ich stimme den Vorschlägen zu"), RFC 0011 decision 5
- Why: The fold parks the field's operations as `ParkReason::UnknownType`, the field reads as unset, a write is refused with `unknownType` naming the type id and rule, and the next open with the plugin folds what was parked. It keeps what a member does not understand (principle 3) and keeps every tool without the plugin working. A different rule version counts as not registered.
- Instead of: refusing to open with `modelRefused`, which would stop every app and tool without the plugin; refusing only in debug builds, so one lock behaves differently by build.
- Topics: rfc-0011, plugins, parking, errors
