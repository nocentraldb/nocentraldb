# 2026-11-27 · NOC-575 · The rich-text API names its codes vocabularyMismatch and contentCheckMissing, and the binding's composing, onReload, vocabulary and known

- Decided by: Carsten, 2026-11-27, on NOC-575
- Why: each name says what it does in the words RFC 0013 and RFC 0009 already use: the two error codes name the mistake an app fixes, and the binding's members name the IME composition, the reload, the field's vocabulary and the names an editor knows beyond it (`known({ nodes, marks, attributes })`).
- Instead of: other names for any of the six, chosen before the API is published.
- Topics: rich-text, rfc-0013, rfc-0009, sdk-ts, errors, naming
