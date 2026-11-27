# 2026-11-26 · NOC-459 · Every rich-text change rejected as not a member leaves placeholders, and one rejected at step 3 or 5 is left out

- Decided by: pm, NOC-459
- Why: RFC 0013 un-folds a rejected change to placeholders and requires the result to be a function of the set of changes, so a change rejected before it was ever folded must leave the same placeholders as one un-folded later. A change rejected as not a member (step 4) opens alike on every member that holds it, under the key the control state gives its generation, so each of its rich-text operations is placed in the field, after the field's other operations in stamp order, whether it was un-folded, rejected on arrival, or held before the model knew the field. A change rejected at step 3 (unknown author, bad signature) or step 5 (no key, does not decrypt, a replaced key) cannot be opened alike everywhere, so it leaves none and its update is left out, as for other field types.
- Instead of: placeholders only for a change the member had folded before its rejection, which ends in a state that depends on the order of arrival (the harness's broken fixture `ONLY_WHAT_IT_FOLDED` fails C1); placeholders for every rejected change, which members that cannot open it would not have.
- Topics: rich-text, rfc-0013, un-folding, convergence

A change rejected at step 5 that some member had folded loses what F5 describes: text typed right next to its characters waits for them for good.
