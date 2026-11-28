---
type: fix
tickets: [NOC-580]
components: [sdk-ts]
introduced: cycle
---
## Symptom
A person who opened a rich-text field and, as the first edit, emptied it (for example, select all and delete) saw an empty editor, but the deletion was never written: other members and the next open still showed the text.

## When
Only when the first edit after the editor opened left the document empty. Any other edit first, then emptying it, was written.

## Cause
TipTap's collaboration plugin (y-tiptap 3.3.1) writes a change only once the document has differed from an empty one, and its own first render does not count, since it happens before the plugin's update hook exists.

## Fix
`ncEditor` hands a document an edit left empty to the plugin's binding as the plugin itself would, so it is written and can be undone. Opening or rebuilding an editor still writes nothing.
