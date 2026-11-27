# 2026-11-27 · NOC-560 · A move back of a written rich-text document is refused until a move back writes only the difference

- Decided by: Carsten, 2026-11-27, NOC-560
- Why: a move into a space whose log already holds the object's rich-text document, such as a move back, would re-create the document next to the one that log holds, and the text would show twice. Refusing it with `notAllowed` (`error.moveRichTextBackLater`) loses nothing and changes no merge rule. The lasting answer is a move back that writes only the difference between that space's document and the current one, planned for Q2 2027 (NOC-581); the refusal stays until then.
- Instead of: writing only the difference now, which needs a tree diff in the core; clearing the old document and re-creating it, which rewrites more than it changes; a fresh start of the document at each create of the object, which changes rich text's merge rule in the core, needs a causal window, a report of the updates it leaves out and a rebuild in the editor binding.
- Topics: rich-text, move, rfc-0013, rfc-0016
